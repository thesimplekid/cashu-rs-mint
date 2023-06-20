use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use cashu::keyset::{mint, Map};
use cashu::lightning_invoice::Invoice as LnInvoice;
use cashu::mint::{
    CheckFeesResponse, CheckSpendableResponse, Invoice, KeySetsResponse, MeltResponse, Mint,
    MintResponse, Sha256, SplitResponse,
};
use cashu::wallet::{check_fees, check_spendable, melt, split, MintRequest};
use cashu::{keyset, lightning_invoice, secret::Secret, Amount};
use ln::cln::fee_reserve;
use ln::{InvoiceStatus, InvoiceTokenStatus, Ln};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use types::KeysetInfo;
use utils::unix_time;

use crate::database::Db;
use crate::error::Error;
use crate::ln::cln::Cln;

mod config;
mod database;
mod error;
mod ln;
mod types;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let settings = config::Settings::new(&Some("./config.toml".to_string()));

    let db_path = settings.info.clone().db_path;

    let db = Db::new(db_path).await.unwrap();

    let cln_socket = utils::expand_path(settings.ln.path.to_str().unwrap()).unwrap();

    let all_keysets = db.get_all_keyset_info().await?;

    let inactive_keysets: HashMap<keyset::Id, keyset::mint::KeySet> = all_keysets
        .iter()
        .map(|(k, v)| (k.clone(), v.keyset.clone()))
        .collect();

    let paid_invoices_info = db.get_unissued_invoices().await?;

    let paid_invoices: HashMap<Sha256, (Amount, lightning_invoice::Invoice)> = paid_invoices_info
        .iter()
        .map(|(k, v)| (k.clone(), (v.amount, v.invoice.clone())))
        .collect();

    let pending_invoices_info = db.get_pending_invoices().await?;

    let pending_invoices: HashMap<Sha256, (Amount, Option<lightning_invoice::Invoice>)> =
        pending_invoices_info
            .iter()
            .map(|(k, v)| (k.clone(), (v.amount, Some(v.invoice.clone()))))
            .collect();

    let spent_secrets: HashSet<Secret> = db.get_spent_secrets().await?;

    let mint = Mint::new_with_history(
        settings.info.secret_key,
        settings.info.derivation_path,
        8,
        inactive_keysets,
        paid_invoices,
        pending_invoices,
        spent_secrets,
    );
    let keyset = mint.active_keyset();
    db.set_active_keyset(keyset.id).await?;
    let keyset_info = KeysetInfo {
        valid_from: unix_time(),
        valid_to: None,
        keyset: mint::KeySet::from(keyset),
    };
    db.add_keyset(&keyset_info).await?;

    let mint = Arc::new(Mutex::new(mint));

    let ln = Ln {
        ln_processor: Arc::new(Cln::new(cln_socket, db.clone(), mint.clone())),
    };

    let ln_clone = ln.clone();
    tokio::spawn(async move {
        loop {
            if let Err(err) = ln_clone.ln_processor.wait_invoice().await {
                warn!("{}", err);
            }
        }
    });

    let mint_info = MintInfo::from(settings.mint_info);

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
        .route("/melt", post(post_melt))
        .route("/split", post(post_split))
        .route("/check", post(post_check))
        .route("/info", get(get_info))
        .with_state(state);

    let ip = Ipv4Addr::from_str(&settings.info.listen_host)?;

    let port = settings.info.listen_port;

    let listen_addr = SocketAddr::new(std::net::IpAddr::V4(ip), port);

    axum::Server::bind(&listen_addr)
        .serve(mint_service.into_make_service())
        .await?;

    Ok(())
}

#[derive(Clone)]
struct MintState {
    ln: Ln,
    db: Db,
    mint: Arc<Mutex<Mint>>,
    mint_info: MintInfo,
}

async fn get_keys(State(state): State<MintState>) -> Result<Json<Map>, StatusCode> {
    let mint = state.mint.lock().await;

    let keys = mint.active_keyset_pubkeys();

    Ok(Json(keys.keys))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestMintParams {
    amount: u64,
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
    pubkey: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    descrption_long: Option<String>,
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
            pubkey: info.pubkey,
            version: info.version,
            description: info.description,
            descrption_long: info.descrption_long,
            contact: info.contact,
            nuts: info.nuts,
            motd: info.motd,
        }
    }
}

async fn get_keysets(State(state): State<MintState>) -> Result<Json<KeySetsResponse>, StatusCode> {
    let mint = state.mint.lock().await;

    Ok(Json(mint.keysets()))
}

async fn get_request_mint(
    State(state): State<MintState>,
    Query(params): Query<RequestMintParams>,
) -> Result<Json<Invoice>, Error> {
    let amount = Amount::from(params.amount);

    let mut mint = state.mint.lock().await;

    let hash = mint.process_invoice_request(amount);

    let invoice = state
        .ln
        .ln_processor
        .get_invoice(amount, hash, "")
        .await
        .map_err(|err| {
            warn!("{}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match mint.set_invoice(hash, invoice.invoice) {
        Some(invoice) => Ok(Json(invoice)),
        None => Err(StatusCode::INTERNAL_SERVER_ERROR.into()),
    }
}

async fn post_mint(
    State(state): State<MintState>,
    Query(params): Query<MintParams>,
    Json(payload): Json<MintRequest>,
) -> Result<Json<MintResponse>, Error> {
    let hash = match params.hash {
        Some(hash) => hash,
        None => match params.payment_hash {
            Some(hash) => hash,
            None => return Err(StatusCode::BAD_REQUEST.into()),
        },
    };

    let db = state.db;
    let invoice = db.get_invoice_info(&hash).await.unwrap();

    debug!("Before lock");
    debug!("got mint lock");

    match invoice.status {
        InvoiceStatus::Paid => {
            // sanity check
            // It really shouldnt be needed but incase wait invoice fails to call mint.pay_invoice
            let mut mint = state.mint.lock().await;
            if invoice.token_status.eq(&InvoiceTokenStatus::NotIssued) {
                debug!("Invoice Paid");
                mint.pay_invoice(hash);
            }
            drop(mint);
        }
        InvoiceStatus::Unpaid => {
            debug!("Checking");
            state
                .ln
                .ln_processor
                .check_invoice_status(&invoice.payment_hash)
                .await
                .unwrap();
            let invoice = db.get_invoice_info(&hash).await.unwrap();

            debug!("Unpaid check: {:?}", invoice.status);
        }
        InvoiceStatus::Expired => {
            return Err(Error::InvoiceExpired);
        }
    }

    let mut mint = state.mint.lock().await;

    let res = match mint.process_mint_request(hash, payload) {
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
    Json(payload): Json<check_fees::Request>,
) -> Result<Json<CheckFeesResponse>, Error> {
    let invoice = LnInvoice::from_str(&payload.pr)?;

    let amount_msat = invoice.amount_milli_satoshis().unwrap();
    let amount_sat = amount_msat / 1000;
    let amount = Amount::from(amount_sat);

    let fee = fee_reserve(amount);

    Ok(Json(CheckFeesResponse::new(fee)))
}

async fn post_melt(
    State(state): State<MintState>,
    Json(payload): Json<melt::Request>,
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
        .pay_invoice(payload.payment_request.clone(), None)
        .await
        .map_err(|err| {
            warn!("{}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state
        .db
        .add_used_proofs(&payload.proofs)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Process mint request
    Ok(Json(
        mint.process_melt_request(payload, &pay_res.0, pay_res.1)
            .map_err(|err| {
                warn!("{}", err);
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
    ))
}

async fn post_split(
    State(state): State<MintState>,
    Json(payload): Json<split::Request>,
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

async fn post_check(
    State(state): State<MintState>,
    Json(payload): Json<check_spendable::Request>,
) -> Result<Json<CheckSpendableResponse>, Error> {
    let mint = state.mint.lock().await;

    Ok(Json(mint.check_spendable(payload).map_err(|err| {
        warn!("{}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?))
}

async fn get_info(State(state): State<MintState>) -> Result<Json<MintInfo>, Error> {
    Ok(Json(state.mint_info))
}
