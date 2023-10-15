use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Query, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bitcoin_hashes::sha256;
use bitcoin_hashes::Hash;
use cashu_sdk::amount::Amount;
use cashu_sdk::nuts::nut01::Keys;
use cashu_sdk::nuts::nut02::{mint::KeySet, Id};
use cashu_sdk::nuts::nut03::RequestMintResponse;
use cashu_sdk::nuts::nut04::{MintRequest, PostMintResponse};
use cashu_sdk::nuts::nut05::{CheckFeesRequest, CheckFeesResponse};
use cashu_sdk::nuts::nut06::{SplitRequest, SplitResponse};
use cashu_sdk::nuts::nut07::{CheckSpendableRequest, CheckSpendableResponse};
use cashu_sdk::nuts::nut08::{MeltRequest, MeltResponse};
use cashu_sdk::nuts::nut09::MintVersion;
use cashu_sdk::{mint::Mint, Sha256};
use cashu_sdk::{nuts::*, Bolt11Invoice};
use clap::Parser;
use futures::StreamExt;
use ln_rs::cln::fee_reserve;
use ln_rs::{InvoiceStatus, InvoiceTokenStatus, Ln};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{debug, warn};
use types::KeysetInfo;
use utils::{cashu_crab_amount_to_ln_rs_amount, ln_rs_amount_to_cashu_crab_amount, unix_time};

pub const CARGO_PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

use crate::cli::CLIArgs;
use crate::database::Db;
use crate::error::Error;

mod cli;
mod config;
mod database;
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

    let db = Db::new(db_path).await.unwrap();

    let all_keysets = db.get_all_keyset_info().await?;

    let inactive_keysets: HashMap<Id, nut02::mint::KeySet> = all_keysets
        .iter()
        // FIXME: Handle unwrap
        // TODO: Should check that ID matches
        .map(|(k, v)| {
            (
                Id::try_from_base64(k).unwrap(),
                KeySet::generate(&v.secret, &v.derivation_path, v.max_order),
            )
        })
        .collect();

    let spent_secrets = db.get_spent_secrets().await?;

    let mint = Mint::new(
        &settings.info.secret_key,
        &settings.info.derivation_path,
        inactive_keysets,
        spent_secrets,
        settings.info.max_order,
        settings.info.min_fee_reserve,
        settings.info.min_fee_percent,
    );

    let keyset = mint.active_keyset();
    db.set_active_keyset(&keyset.id).await?;
    let keyset_info = KeysetInfo {
        valid_from: unix_time(),
        valid_to: None,
        id: keyset.id,
        secret: settings.info.secret_key.clone(),
        derivation_path: settings.info.derivation_path.clone(),
        max_order: settings.info.max_order,
    };
    db.add_keyset(&keyset_info).await?;

    let mint = Arc::new(Mutex::new(mint));

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

    let last_pay_index = db.get_last_pay_index().await?;

    let cln = ln_rs::Cln::new(cln_socket, Some(last_pay_index)).await?;

    let ln = Ln {
        ln_processor: Arc::new(cln.clone()),
    };

    let nodemanger = ln_rs::node_manager::NodeManger {
        ln: Arc::new(Box::new(cln)),
        authorized_users: HashSet::new(),
        jwt_secret: "secret".to_string(),
    };

    let ln_clone = ln.clone();
    let db_clone = db.clone();

    tokio::spawn(async move {
        loop {
            let mut stream = ln_clone.ln_processor.wait_invoice().await.unwrap();

            while let Some((invoice, pay_index)) = stream.next().await {
                if let Err(err) = handle_paid_invoice(&db_clone, invoice, pay_index).await {
                    warn!("{:?}", err);
                }
            }
        }
    });

    let mint_info = MintInfo::from(settings.mint_info.clone());

    let settings_clone = settings.clone();

    if settings_clone.node_manager.is_some()
        && settings_clone
            .node_manager
            .as_ref()
            .unwrap()
            .enable_node_manager
            .eq(&true)
    {
        let node_manager_settings = settings_clone.node_manager.unwrap();
        tokio::spawn(async move {
            loop {
                if let Err(err) = nodemanger
                    .clone()
                    .start_server(
                        &node_manager_settings.listen_host,
                        node_manager_settings.listen_port,
                        node_manager_settings.authorized_users.clone(),
                        &node_manager_settings.jwt_secret,
                    )
                    .await
                {
                    warn!("{:?}", err)
                }
            }
        });
    }
    let state = MintState {
        db,
        ln,
        mint,
        mint_info,
    };

    let mint_service = Router::new()
        .route("/keys", get(get_keys))
        .route("/keysets", get(get_keysets))
        .route("/mint", get(get_request_mint))
        .route("/mint", post(post_mint))
        .route("/checkfees", post(post_check_fee))
        .route("/split", post(post_split))
        .route("/check", post(post_check))
        .route("/melt", post(post_melt))
        .route("/info", get(get_info))
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

async fn handle_paid_invoice(
    db: &Db,
    invoice: Bolt11Invoice,
    last_pay_index: Option<u64>,
) -> anyhow::Result<()> {
    if let Some(pay_index) = last_pay_index {
        db.set_last_pay_index(pay_index).await?;
    }

    let payment_hash = Sha256::from_str(&invoice.payment_hash().to_string())?;

    let mut invoice_info = db.get_invoice_info_by_payment_hash(&payment_hash).await?;

    invoice_info.status = InvoiceStatus::Paid;
    invoice_info.confirmed_at = Some(unix_time());

    db.add_invoice(&invoice_info).await?;

    Ok(())
}

#[derive(Clone)]
struct MintState {
    ln: Ln,
    db: Db,
    mint: Arc<Mutex<Mint>>,
    mint_info: MintInfo,
}

async fn get_keys(State(state): State<MintState>) -> Result<Json<Keys>, StatusCode> {
    let mint = state.mint.lock().await;

    let keys = mint.active_keyset_pubkeys();

    Ok(Json(keys.keys))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestMintParams {
    amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MintParams {
    hash: Option<Sha256>,
    payment_hash: Option<Sha256>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MintInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    contact: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    nuts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    motd: Option<String>,
}

impl From<config::MintInfo> for MintInfo {
    fn from(info: config::MintInfo) -> Self {
        Self {
            name: info.name,
            version: info.version,
            description: info.description,
            description_long: info.description_long,
            contact: info.contact,
            nuts: info.nuts,
            motd: info.motd,
        }
    }
}

async fn get_keysets(State(state): State<MintState>) -> Result<Json<nut02::Response>, StatusCode> {
    let mint = state.mint.lock().await;

    Ok(Json(mint.keysets()))
}

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

async fn post_split(
    State(state): State<MintState>,
    Json(payload): Json<SplitRequest>,
) -> Result<Json<SplitResponse>, Error> {
    let mut mint = state.mint.lock().await;

    let proofs = payload.proofs.clone();

    match mint.process_split_request(payload) {
        Ok(split_response) => {
            state.db.add_used_proofs(&proofs).await.map_err(|err| {
                warn!("Could not add used proofs {:?}", err);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(split_response))
        }
        Err(err) => {
            warn!("{}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR.into())
        }
    }
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
