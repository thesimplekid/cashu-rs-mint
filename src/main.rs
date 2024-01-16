use std::collections::HashSet;
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
use axum::routing::{get, post};
use axum::Router;
use bip39::Mnemonic;
use cashu_sdk::amount::Amount;
use cashu_sdk::mint::{Mint, RedbLocalStore};
use cashu_sdk::nuts::nut02::Id;
use cashu_sdk::nuts::{
    CheckStateRequest, CheckStateResponse, MeltBolt11Request, MeltBolt11Response,
    MintBolt11Request, MintBolt11Response, SwapRequest, SwapResponse, *,
};
use cashu_sdk::types::MintQuote;
use clap::Parser;
use futures::StreamExt;
use ln_rs::Ln;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::warn;
use utils::unix_time;

pub const CARGO_PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

use crate::cli::CLIArgs;
use crate::error::Error;

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

    let localstore = RedbLocalStore::new(db_path.to_str().unwrap())?;
    let s = "";
    let mnemonic = Mnemonic::from_str("")?;

    let mint = Mint::new(
        Arc::new(localstore),
        mnemonic,
        HashSet::new(),
        Amount::ZERO,
        0.0,
    )
    .await?;

    /*
        let ln = match &settings.ln.ln_backend {
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
                // TODO: get this from db

                let last_pay_index = db.get_last_pay_index().await?;

                let cln = Arc::new(ln_rs::Cln::new(cln_socket, Some(last_pay_index)).await?);

                let node_manager = if settings.node_manager.is_some()
                    && settings.node_manager.as_ref().unwrap().enable_node_manager
                {
                    Some(ln_rs::node_manager::NodeManger(cln.clone()))
                } else {
                    None
                };

                Ln {
                    ln_processor: cln,
                    node_manager,
                }
            }
            LnBackend::Greenlight => {
                // Greenlight::recover().await.unwrap();
                // TODO: get this from db
                let last_pay_index = None;

                let gln = match args.recover {
                    Some(seed) => Arc::new(Greenlight::recover(&seed, last_pay_index).await?),
                    None => {
                        let invite_code = settings.ln.greenlight_invite_code.clone();

                        Arc::new(Greenlight::new(invite_code).await?)
                    }
                };

                let node_manager = if settings.node_manager.is_some()
                    && settings.node_manager.as_ref().unwrap().enable_node_manager
                {
                    Some(ln_rs::node_manager::Nodemanger::Greenlight(gln.clone()))
                } else {
                    None
                };

                Ln {
                    ln_processor: gln,
                    node_manager,
                }
            }
            LnBackend::Ldk => {
                let ldk = Arc::new(ln_rs::Ldk::new().await?);

                let node_manager = if settings.node_manager.is_some()
                    && settings.node_manager.as_ref().unwrap().enable_node_manager
                {
                    Some(ln_rs::node_manager::Nodemanger::Ldk(ldk.clone()))
                } else {
                    None
                };

                Ln {
                    ln_processor: ldk,
                    node_manager,
                }
            }
        };
    */

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
    // TODO: get this from db

    let last_pay_index = 0;

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
            }
        }
    });

    let mint_info = MintInfo::from(settings.mint_info.clone());

    let settings_clone = settings.clone();

    let state = MintState {
        ln,
        mint: Arc::new(Mutex::new(mint)),
        mint_info,
    };

    let mint_service = Router::new()
        .route("/v1/keys", get(get_keys))
        .route("/v1/keysets", get(get_keysets))
        .route("/v1/keys/:keyset_id", get(get_keyset_pubkeys))
        .route("/v1/swap", post(post_swap))
        .route("/v1/mint/quote/bolt11", get(get_mint_bolt11_quote))
        .route(
            "/v1/mint/quote/bolt11/:quote_id",
            get(get_check_mint_bolt11_quote),
        )
        .route("/v1/mint/bolt11", post(post_mint_bolt11))
        .route("/v1/melt/quote/bolt11", get(get_melt_bolt11_quote))
        .route(
            "/v1/melt/quote/bolt11/:quote_id",
            get(get_check_melt_bolt11_quote),
        )
        .route("/v1/melt/bolt11", post(post_melt_bolt11))
        .route("/v1/checkstate", post(post_check))
        .route("/v1/info", get(get_mint_info))
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
    mint_info: MintInfo,
}

async fn get_keys(State(state): State<MintState>) -> Result<Json<KeysResponse>, StatusCode> {
    let pubkeys = state.mint.lock().await.pubkeys().await.unwrap();

    Ok(Json(pubkeys))
}

async fn get_keyset_pubkeys(
    State(state): State<MintState>,
    Path(keyset_id): Path<Id>,
) -> Result<Json<KeysResponse>, StatusCode> {
    let pubkeys = state
        .mint
        .lock()
        .await
        .keyset_pubkeys(&keyset_id)
        .await
        .unwrap()
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(pubkeys))
}

async fn get_keysets(State(state): State<MintState>) -> Result<Json<KeysetResponse>, StatusCode> {
    let mint = state.mint.lock().await.keysets().await.unwrap();

    Ok(Json(mint))
}

async fn get_mint_bolt11_quote(
    State(state): State<MintState>,
    Json(payload): Json<MintQuoteBolt11Request>,
) -> Result<Json<MintQuoteBolt11Response>, StatusCode> {
    let invoice = state
        .ln
        .ln_processor
        .create_invoice(
            ln_rs::Amount::from_sat(u64::from(payload.amount)),
            "".to_string(),
        )
        .await
        .unwrap();

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
        .unwrap();

    Ok(Json(quote.into()))
}

async fn get_check_mint_bolt11_quote(
    State(state): State<MintState>,
    Path(quote_id): Path<String>,
) -> Result<Json<MintQuoteBolt11Response>, StatusCode> {
    let quote = state
        .mint
        .lock()
        .await
        .check_mint_quote(&quote_id)
        .await
        .unwrap();

    Ok(Json(quote))
}

async fn post_mint_bolt11(
    State(state): State<MintState>,
    Json(payload): Json<MintBolt11Request>,
) -> Result<Json<MintBolt11Response>, StatusCode> {
    let res = state
        .mint
        .lock()
        .await
        .process_mint_request(payload)
        .await
        .unwrap();

    Ok(Json(res))
}

async fn get_melt_bolt11_quote(
    State(state): State<MintState>,
    Json(payload): Json<MeltQuoteBolt11Request>,
) -> Result<Json<MeltQuoteBolt11Response>, StatusCode> {
    let amount = payload.request.amount_milli_satoshis().unwrap() / 1000;
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
        .unwrap();

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
    let preimage = "";
    let res = state
        .mint
        .lock()
        .await
        .process_melt_request(&payload, preimage, Amount::ZERO)
        .await
        .unwrap();

    Ok(Json(res))
}

async fn post_check(
    State(state): State<MintState>,
    Json(payload): Json<CheckStateRequest>,
) -> Result<Json<CheckStateResponse>, Error> {
    let state = state.mint.lock().await.check_state(&payload).await.unwrap();

    Ok(Json(state))
}

async fn get_mint_info(State(state): State<MintState>) -> Result<Json<MintInfo>, Error> {
    Ok(Json(state.mint.lock().await.mint_info().await.unwrap()))
}

async fn post_swap(
    State(state): State<MintState>,
    Json(payload): Json<SwapRequest>,
) -> Result<Json<SwapResponse>, Error> {
    let swap_response = state
        .mint
        .lock()
        .await
        .process_swap_request(payload)
        .await
        .unwrap();
    Ok(Json(swap_response))
}

/*
async fn get_request_mint(
    State(state): State<MintState>,
    Query(params): Query<RequestMintParams>,
) -> Result<Json<RequestMintResponse>, Error> {
    let amount = params.amount;

    let hash = sha256::Hash::hash(&cashu_sdk::utils::random_hash());

    let invoice = state
        .ln
        .ln_processor
        .get_invoice(cashu_crab_amount_to_ln_rs_amount(amount), hash, "")
        .await
        .map_err(|err| {
            warn!("{}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state.db.add_invoice(&invoice).await.unwrap();
    Ok(Json(RequestMintResponse {
        hash: hash.to_string(),
        pr: invoice.invoice,
    }))
}

async fn post_mint(
    State(state): State<MintState>,
    Query(params): Query<MintParams>,
    Json(payload): Json<MintRequest>,
) -> Result<Json<PostMintResponse>, Error> {
    let hash = match params.hash {
        Some(hash) => hash,
        None => match params.payment_hash {
            Some(hash) => hash,
            None => return Err(StatusCode::BAD_REQUEST.into()),
        },
    };

    let db = state.db;
    let invoice = db
        .get_invoice_info(&hash)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // debug!("{:?}", invoice);

    if invoice.amount != cashu_crab_amount_to_ln_rs_amount(payload.total_amount()) {
        return Err(Error::InvoiceNotPaid);
    }

    match invoice.status {
        InvoiceStatus::Paid => {}
        InvoiceStatus::Unpaid => {
            debug!("Checking");
            state
                .ln
                .ln_processor
                .check_invoice_status(&invoice.payment_hash)
                .await
                .unwrap();
            let invoice = db.get_invoice_info(&hash).await.unwrap();

            match invoice.status {
                InvoiceStatus::Unpaid => return Err(Error::InvoiceNotPaid),
                InvoiceStatus::Expired => return Err(Error::InvoiceExpired),
                _ => (),
            }

            debug!("Unpaid check: {:?}", invoice.status);
        }
        InvoiceStatus::Expired => {
            return Err(Error::InvoiceExpired);
        }
        InvoiceStatus::InFlight => {}
    }

    let mut mint = state.mint.lock().await;

    let res = match mint.process_mint_request(payload) {
        Ok(mint_res) => {
            let mut invoice = db.get_invoice_info(&hash).await.map_err(|err| {
                warn!("{}", err);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            invoice.token_status = InvoiceTokenStatus::Issued;

            db.add_invoice(&invoice).await.map_err(|err| {
                warn!("{}", err);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            let in_circulation = db.get_in_circulation().await.unwrap()
                + ln_rs_amount_to_cashu_crab_amount(invoice.amount);

            db.set_in_circulation(&in_circulation).await.ok();

            mint_res
        }
        Err(err) => match db.get_invoice_info(&hash).await {
            Ok(_) => {
                warn!("{}", err);
                return Err(Error::InvoiceNotPaid);
            }
            Err(err) => {
                warn!("{}", err);
                return Err(StatusCode::NOT_FOUND.into());
            }
        },
    };

    Ok(Json(res))
}

async fn post_check_fee(
    Json(payload): Json<CheckFeesRequest>,
) -> Result<Json<CheckFeesResponse>, Error> {
    // let invoice = LnInvoice::from_str(&payload.pr)?;

    let amount_msat = payload.pr.amount_milli_satoshis().unwrap();
    let amount_sat = amount_msat / 1000;
    let amount = Amount::from(amount_sat);

    let fee =
        ln_rs_amount_to_cashu_crab_amount(fee_reserve(cashu_crab_amount_to_ln_rs_amount(amount)));

    Ok(Json(CheckFeesResponse { fee }))
}


async fn post_melt(
    State(state): State<MintState>,
    Json(payload): Json<MeltRequest>,
) -> Result<Json<MeltResponse>, Error> {
    let mut mint = state.mint.lock().await;
    mint.verify_melt_request(&payload).map_err(|err| {
        warn!("{}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Pay ln
    let pay_res = state
        .ln
        .ln_processor
        .pay_invoice(payload.pr.clone(), None)
        .await
        .map_err(|err| {
            warn!("{}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let melt_response = mint
        .process_melt_request(
            &payload,
            &pay_res.payment_preimage.unwrap_or("".to_string()),
            ln_rs_amount_to_cashu_crab_amount(pay_res.total_spent),
        )
        .map_err(|err| {
            warn!("{}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state
        .db
        .add_used_proofs(&payload.proofs)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let in_circulation = state.db.get_in_circulation().await.unwrap();

    let in_circulation = in_circulation - payload.proofs_amount() + melt_response.change_amount();

    state.db.set_in_circulation(&in_circulation).await.unwrap();

    // Process mint request
    Ok(Json(melt_response))
}

async fn post_check(
    State(state): State<MintState>,
    Json(payload): Json<CheckSpendableRequest>,
) -> Result<Json<CheckSpendableResponse>, Error> {
    let mint = state.mint.lock().await;

    Ok(Json(mint.check_spendable(&payload).map_err(|err| {
        warn!("{}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?))
}

async fn get_info(State(state): State<MintState>) -> Result<Json<nut09::MintInfo>, Error> {
    // TODO:
    let nuts = vec![
        "NUT-07".to_string(),
        "NUT-08".to_string(),
        "NUT-09".to_string(),
    ];

    let mint_version = MintVersion {
        name: "cashu-rs-mint".to_string(),
        version: CARGO_PKG_VERSION
            .map(std::borrow::ToOwned::to_owned)
            .unwrap_or("".to_string()),
    };

    let contact: Vec<Vec<String>> = state
        .mint_info
        .contact
        .iter()
        .map(|inner_map| {
            inner_map
                .iter()
                .flat_map(|(k, v)| vec![k.clone(), v.clone()])
                .collect()
        })
        .collect();

    let mint_info = nut09::MintInfo {
        name: state.mint_info.name,
        // TODO:
        pubkey: None,
        version: Some(mint_version),
        description: state.mint_info.description,
        description_long: state.mint_info.description_long,
        contact: Some(contact),
        nuts,
        motd: state.mint_info.motd,
    };

    Ok(Json(mint_info))
}
*/
