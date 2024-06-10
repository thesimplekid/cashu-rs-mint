use std::fs::{self, File};
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Json, Path, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use bip39::Mnemonic;
use cdk::amount::Amount;
use cdk::cdk_database::{self, MintDatabase};
use cdk::mint::Mint;
use cdk::nuts::nut02::Id;
use cdk::nuts::{
    CheckStateRequest, CheckStateResponse, MeltBolt11Request, MeltBolt11Response,
    MintBolt11Request, MintBolt11Response, SwapRequest, SwapResponse, *,
};
use cdk::types::MintQuote;
use cdk_redb::MintRedbDatabase;
use cdk_sqlite::MintSqliteDatabase;
use clap::Parser;
use error::into_response;
use futures::StreamExt;
use ln_rs::{Bolt11Invoice, Ln};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{debug, warn};
use utils::unix_time;

use crate::cli::CLIArgs;
use crate::config::DatabaseEngine;

pub const CARGO_PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

mod cli;
mod config;
mod error;
mod types;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let args = CLIArgs::parse();

    // get config file name from args
    let config_file_arg = match args.config {
        Some(c) => c,
        None => "./config.toml".to_string(),
    };

    debug!("Path: {}", config_file_arg);

    let settings = config::Settings::new(&Some(config_file_arg));

    let mint_url = settings.info.url.clone();

    let db_path = match args.db {
        Some(path) => PathBuf::from_str(&path)?,
        None => settings.info.clone().db_path,
    };

    let localstore: Arc<dyn MintDatabase<Err = cdk_database::Error> + Send + Sync> =
        match settings.database.engine {
            DatabaseEngine::Sqlite => {
                Arc::new(MintSqliteDatabase::new(db_path.to_str().unwrap()).await?)
            }
            DatabaseEngine::Redb => Arc::new(MintRedbDatabase::new(db_path.to_str().unwrap())?),
        };
    let mint_info = MintInfo::default();

    let mnemonic = Mnemonic::from_str(&settings.info.mnemonic)?;

    let mint = Mint::new(
        &mnemonic.to_seed_normalized(""),
        mint_info,
        localstore,
        Amount::ZERO,
        0.0,
    )
    .await?;

    println!("Mint created");

    let cln_socket = utils::expand_path(
        settings
            .ln
            .cln_path
            .clone()
            .ok_or(anyhow!("cln socket not defined"))?
            .to_str()
            .ok_or(anyhow!("cln socket not defined"))?,
    )
    .ok_or(anyhow!("cln socket not defined"))?;

    let last_pay_path = PathBuf::from_str(&settings.info.last_pay_path.clone())?;

    match fs::metadata(&last_pay_path) {
        Ok(_) => (),
        Err(_e) => {
            // Create the parent directory if it doesn't exist
            fs::create_dir_all(last_pay_path.parent().unwrap())?;

            // Attempt to create the file
            let mut fs = File::create(&last_pay_path).unwrap();
            fs.write_all(&0_u64.to_be_bytes()).unwrap();
        }
    }

    let last_pay = fs::read(&last_pay_path).unwrap();

    let last_pay_index =
        u64::from_be_bytes(last_pay.try_into().unwrap_or([0, 0, 0, 0, 0, 0, 0, 0]));

    let cln = ln_rs::Cln::new(cln_socket, Some(last_pay_index)).await?;

    let ln = Ln {
        ln_processor: Arc::new(cln.clone()),
    };

    let ln_clone = ln.clone();
    let mint_clone = Arc::new(mint.clone());

    tokio::spawn(async move {
        loop {
            let mut stream = ln_clone.ln_processor.wait_invoice().await.unwrap();

            while let Some((invoice, pay_index)) = stream.next().await {
                if let Err(err) =
                    handle_paid_invoice(mint_clone.clone(), &invoice.to_string()).await
                {
                    warn!("{:?}", err);
                }
                if let Some(pay_index) = pay_index {
                    if let Err(err) = fs::write(&last_pay_path, pay_index.to_be_bytes()) {
                        warn!("Could not write last pay index {:?}", err);
                    }
                }
            }
        }
    });

    let state = MintState {
        ln,
        mint: Arc::new(Mutex::new(mint)),
        mint_url,
    };

    let mint_service = Router::new()
        .route("/v1/keys", get(get_keys))
        .route("/v1/keysets", get(get_keysets))
        .route("/v1/keys/:keyset_id", get(get_keyset_pubkeys))
        .route("/v1/swap", post(post_swap))
        .route("/v1/mint/quote/bolt11", post(get_mint_bolt11_quote))
        .route(
            "/v1/mint/quote/bolt11/:quote_id",
            get(get_check_mint_bolt11_quote),
        )
        .route("/v1/mint/bolt11", post(post_mint_bolt11))
        .route("/v1/melt/quote/bolt11", post(get_melt_bolt11_quote))
        .route(
            "/v1/melt/quote/bolt11/:quote_id",
            get(get_check_melt_bolt11_quote),
        )
        .route("/v1/melt/bolt11", post(post_melt_bolt11))
        .route("/v1/checkstate", post(post_check))
        .route("/v1/info", get(get_mint_info))
        .route("/v1/restore", post(post_restore))
        .layer(CorsLayer::very_permissive().allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            ACCESS_CONTROL_ALLOW_CREDENTIALS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
        ]))
        .with_state(state);

    let ip = Ipv4Addr::from_str(&settings.info.listen_host)?;

    let port = settings.info.listen_port;

    let listen_addr = SocketAddr::new(std::net::IpAddr::V4(ip), port);
    axum::Server::bind(&listen_addr)
        .serve(mint_service.into_make_service())
        .await?;

    Ok(())
}

async fn handle_paid_invoice(mint: Arc<Mint>, request: &str) -> anyhow::Result<()> {
    let quotes: Vec<MintQuote> = mint.mint_quotes().await?;

    for quote in quotes {
        if quote.request.eq(request) {
            let q = MintQuote {
                id: quote.id,
                mint_url: quote.mint_url,
                amount: quote.amount,
                unit: quote.unit,
                request: quote.request,
                paid: true,
                expiry: quote.expiry,
            };

            mint.update_mint_quote(q).await?;
        }
    }

    Ok(())
}

#[derive(Clone)]
struct MintState {
    ln: Ln,
    mint: Arc<Mutex<Mint>>,
    mint_url: String,
}

async fn get_keys(State(state): State<MintState>) -> Result<Json<KeysResponse>, Response> {
    let pubkeys = state
        .mint
        .lock()
        .await
        .pubkeys()
        .await
        .map_err(into_response)?;

    Ok(Json(pubkeys))
}

async fn get_keyset_pubkeys(
    State(state): State<MintState>,
    Path(keyset_id): Path<Id>,
) -> Result<Json<KeysResponse>, Response> {
    let pubkeys = state
        .mint
        .lock()
        .await
        .keyset_pubkeys(&keyset_id)
        .await
        .map_err(into_response)?;

    Ok(Json(pubkeys))
}

async fn get_keysets(State(state): State<MintState>) -> Result<Json<KeysetResponse>, Response> {
    let mint = state
        .mint
        .lock()
        .await
        .keysets()
        .await
        .map_err(into_response)?;

    Ok(Json(mint))
}

async fn get_mint_bolt11_quote(
    State(state): State<MintState>,
    Json(payload): Json<MintQuoteBolt11Request>,
) -> Result<Json<MintQuoteBolt11Response>, Response> {
    let invoice = state
        .ln
        .ln_processor
        .create_invoice(
            ln_rs::Amount::from_sat(u64::from(payload.amount)),
            "".to_string(),
        )
        .await;

    let invoice = invoice.unwrap();

    let quote = state
        .mint
        .lock()
        .await
        .new_mint_quote(
            state.mint_url.into(),
            invoice.to_string(),
            payload.unit,
            payload.amount,
            unix_time() + 120,
        )
        .await
        .map_err(into_response)?;

    Ok(Json(quote.into()))
}

async fn get_check_mint_bolt11_quote(
    State(state): State<MintState>,
    Path(quote_id): Path<String>,
) -> Result<Json<MintQuoteBolt11Response>, Response> {
    let quote = state
        .mint
        .lock()
        .await
        .check_mint_quote(&quote_id)
        .await
        .map_err(into_response)?;

    Ok(Json(quote))
}

async fn post_mint_bolt11(
    State(state): State<MintState>,
    Json(payload): Json<MintBolt11Request>,
) -> Result<Json<MintBolt11Response>, Response> {
    let res = state
        .mint
        .lock()
        .await
        .process_mint_request(payload)
        .await
        .map_err(into_response)?;

    Ok(Json(res))
}

async fn get_melt_bolt11_quote(
    State(state): State<MintState>,
    Json(payload): Json<MeltQuoteBolt11Request>,
) -> Result<Json<MeltQuoteBolt11Response>, Response> {
    let amount = match payload.options {
        Some(mpp) => mpp.amount,
        None => Amount::from(payload.request.amount_milli_satoshis().unwrap() / 1000),
    };

    assert!(amount > Amount::ZERO);
    let quote = state
        .mint
        .lock()
        .await
        .new_melt_quote(
            payload.request.to_string(),
            payload.unit,
            amount,
            Amount::ZERO,
            unix_time() + 1800,
        )
        .await
        .map_err(into_response)?;

    Ok(Json(quote.into()))
}

async fn get_check_melt_bolt11_quote(
    State(state): State<MintState>,
    Path(quote_id): Path<String>,
) -> Result<Json<MeltQuoteBolt11Response>, StatusCode> {
    let quote = state
        .mint
        .lock()
        .await
        .check_melt_quote(&quote_id)
        .await
        .unwrap();

    Ok(Json(quote))
}

async fn post_melt_bolt11(
    State(state): State<MintState>,
    Json(payload): Json<MeltBolt11Request>,
) -> Result<Json<MeltBolt11Response>, StatusCode> {
    let quote = state
        .mint
        .lock()
        .await
        .verify_melt_request(&payload)
        .await
        .map_err(|_| StatusCode::NOT_ACCEPTABLE)?;

    let invoice = Bolt11Invoice::from_str(&quote.request).unwrap();
    let invoice_amount = ln_rs::Amount::from_msat(
        Bolt11Invoice::from_str(&quote.request)
            .unwrap()
            .amount_milli_satoshis()
            .unwrap(),
    );

    let partial_msat = match u64::from(invoice_amount) > u64::from(quote.amount) {
        true => {
            assert!(payload.proofs_amount() >= quote.amount);
            Some(ln_rs::Amount::from_sat(u64::from(quote.amount)))
        }
        false => {
            assert!(u64::from(payload.proofs_amount()) >= u64::from(invoice_amount));
            None
        }
    };

    let pre = state
        .ln
        .ln_processor
        .pay_invoice(invoice, partial_msat, None)
        .await
        .unwrap();

    let preimage = pre.payment_preimage;
    let res = state
        .mint
        .lock()
        .await
        .process_melt_request(
            &payload,
            &preimage.unwrap(),
            Amount::from(pre.total_spent.to_sat()),
        )
        .await
        .unwrap();

    Ok(Json(res))
}

async fn post_check(
    State(state): State<MintState>,
    Json(payload): Json<CheckStateRequest>,
) -> Result<Json<CheckStateResponse>, Response> {
    let state = state
        .mint
        .lock()
        .await
        .check_state(&payload)
        .await
        .map_err(into_response)?;

    Ok(Json(state))
}

async fn get_mint_info(State(state): State<MintState>) -> Result<Json<MintInfo>, Response> {
    Ok(Json(
        state.mint.lock().await.mint_info().map_err(into_response)?,
    ))
}

async fn post_swap(
    State(state): State<MintState>,
    Json(payload): Json<SwapRequest>,
) -> Result<Json<SwapResponse>, Response> {
    let swap_response = state
        .mint
        .lock()
        .await
        .process_swap_request(payload)
        .await
        .map_err(into_response)?;
    Ok(Json(swap_response))
}

async fn post_restore(
    State(state): State<MintState>,
    Json(payload): Json<RestoreRequest>,
) -> Result<Json<RestoreResponse>, Response> {
    let restore_response = state
        .mint
        .lock()
        .await
        .restore(payload)
        .await
        .map_err(into_response)?;

    Ok(Json(restore_response))
}
