use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail};
use axum::extract::{Json, Path, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use bip39::Mnemonic;
use cdk::amount::Amount;
use cdk::error::ErrorResponse;
use cdk::mint::{LocalStore, Mint, RedbLocalStore};
use cdk::nuts::nut02::Id;
use cdk::nuts::{
    CheckStateRequest, CheckStateResponse, MeltBolt11Request, MeltBolt11Response,
    MintBolt11Request, MintBolt11Response, SwapRequest, SwapResponse, *,
};
use cdk::types::MintQuote;
use clap::Parser;
use error::{into_response, Error};
use futures::StreamExt;
use ln_rs::{Bolt11Invoice, Ln};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::warn;
use utils::unix_time;

use crate::config::LnBackend;

pub const CARGO_PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

use crate::cli::CLIArgs;

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

    let settings = config::Settings::new(&Some(config_file_arg));

    let db_path = match args.db {
        Some(path) => PathBuf::from_str(&path)?,
        None => settings.info.clone().db_path,
    };

    let db_path = db_path
        .to_str()
        .ok_or(Error::Custom("Invalid db_path".to_string()))?;

    let localstore = RedbLocalStore::new(db_path)?;
    let mint_info = MintInfo::default();
    //settings.mint_info.clone();
    localstore.set_mint_info(&mint_info).await?;

    let mnemonic = Mnemonic::from_str(&settings.info.mnemonic)?;

    let mint = Mint::new(
        Arc::new(localstore),
        mnemonic.clone(),
        HashSet::new(),
        Amount::ZERO,
        0.0,
    )
    .await?;

    let last_pay_path = PathBuf::from_str(&settings.info.last_pay_path.clone())?;

    match fs::metadata(&last_pay_path) {
        Ok(_) => (),
        Err(_e) => {
            // Create the parent directory if it doesn't exist
            fs::create_dir_all(
                last_pay_path
                    .parent()
                    .ok_or(Error::Custom("Invalid last_pay_path".to_string()))?,
            )?;

            // Attempt to create the file
            let mut fs = File::create(&last_pay_path)?;
            fs.write_all(&0_u64.to_be_bytes())?;
        }
    }

    let last_pay = fs::read(&last_pay_path)?;

    let last_pay_index =
        u64::from_be_bytes(last_pay.try_into().unwrap_or([0, 0, 0, 0, 0, 0, 0, 0]));

    let ln: Ln = match settings.ln.ln_backend {
        LnBackend::Greenlight => {
            let seed_path = settings
                .ln
                .greenlight_seed_path
                .ok_or(anyhow!("Greenlight seed not defined"))?;
            let greenlight_mnemonic = match fs::metadata(&seed_path) {
                Ok(_) => {
                    let contents = fs::read_to_string(seed_path)?;
                    Mnemonic::from_str(&contents)?
                }
                Err(_e) => bail!("Seed undefined"),
            };

            let mut greenlight = if let Ok(greenlight) = ln_rs::Greenlight::recover(
                greenlight_mnemonic.clone(),
                settings
                    .ln
                    .greenlight_cert_path
                    .as_ref()
                    .ok_or(anyhow!("cert path not set"))?,
                settings
                    .ln
                    .greenlight_key_path
                    .as_ref()
                    .ok_or(anyhow!("cert path not set"))?,
                &settings.ln.network.ok_or(anyhow!("network not set"))?,
                Some(last_pay_index),
            )
            .await
            {
                greenlight
            } else {
                let greenlight = ln_rs::Greenlight::new(
                    greenlight_mnemonic,
                    &settings
                        .ln
                        .greenlight_cert_path
                        .ok_or(anyhow!("cert path not set"))?,
                    &settings
                        .ln
                        .greenlight_key_path
                        .ok_or(anyhow!("cert path not set"))?,
                    &settings.ln.network.ok_or(anyhow!("network not set"))?,
                )
                .await;

                greenlight?
            };

            // Start the greenlight hsmd
            // This does not actually need to be run within the mint
            // However, it MUST be running somewhere
            // TODO: Should add an option to not start it here
            greenlight.start_signer()?;

            Ln {
                ln_processor: Arc::new(greenlight),
            }
        }
        LnBackend::Cln => {
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

            let cln = ln_rs::Cln::new(cln_socket, Some(last_pay_index)).await?;

            Ln {
                ln_processor: Arc::new(cln.clone()),
            }
        }
        LnBackend::Ldk => {
            todo!()
        }
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

    let invoice = if let Ok(invoice) = invoice {
        invoice
    } else {
        warn!("Could not get ln invoice for mint quote");
        let response = ErrorResponse {
            code: 99,
            error: Some("Could not fetch ln invoice".to_string()),
            detail: None,
        };
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response());
    };

    let quote = state
        .mint
        .lock()
        .await
        .new_mint_quote(
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
    let amount = payload
        .request
        .amount_milli_satoshis()
        .ok_or(Error::Custom("Invoice amount not defined".to_string()).into_response())?
        / 1000;
    assert!(amount > 0);
    let quote = state
        .mint
        .lock()
        .await
        .new_melt_quote(
            payload.request.to_string(),
            payload.unit,
            Amount::from(amount),
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
) -> Result<Json<MeltQuoteBolt11Response>, Response> {
    let quote = state
        .mint
        .lock()
        .await
        .check_melt_quote(&quote_id)
        .await
        .map_err(into_response)?;

    Ok(Json(quote))
}

async fn post_melt_bolt11(
    State(state): State<MintState>,
    Json(payload): Json<MeltBolt11Request>,
) -> Result<Json<MeltBolt11Response>, Response> {
    let quote = state
        .mint
        .lock()
        .await
        .verify_melt_request(&payload)
        .await
        .map_err(into_response)?;

    let pre = state
        .ln
        .ln_processor
        .pay_invoice(
            Bolt11Invoice::from_str(&quote.request)
                .map_err(|_| Error::DecodeInvoice.into_response())?,
            None,
        )
        .await
        .map_err(|err| Error::Ln(err).into_response())?;

    let preimage = pre
        .payment_preimage
        .ok_or(Error::DecodeInvoice.into_response())?;
    let res = state
        .mint
        .lock()
        .await
        .process_melt_request(&payload, &preimage, Amount::from(pre.total_spent.to_sat()))
        .await
        .map_err(into_response)?;

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
        state
            .mint
            .lock()
            .await
            .mint_info()
            .await
            .map_err(into_response)?,
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
