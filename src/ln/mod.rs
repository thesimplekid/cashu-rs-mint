use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use cashu_crab::{lightning_invoice::Invoice, Amount, Sha256};
use cln_rpc::model::responses::ListinvoicesInvoicesStatus;
use gl_client::pb::cln::listinvoices_invoices::ListinvoicesInvoicesStatus as GL_ListInvoiceStatus;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::bitcoin::util::address::Address;
use ldk_node::ChannelDetails;
use ldk_node::NetAddress;
use log::warn;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};

pub use error::Error;

use crate::config::Settings;

pub mod cln;
pub mod error;
pub mod greenlight;
pub mod ldk;

const SECS_IN_DAY: u32 = 86400;

#[derive(Clone)]
pub struct Ln {
    pub ln_processor: Arc<dyn LnProcessor>,
    pub node_manager: Nodemanger,
}

#[derive(Clone)]
pub enum Nodemanger {
    Ldk(Arc<ldk::Ldk>),
    Cln(Arc<cln::Cln>),
    Greenlight(Arc<greenlight::Greenlight>),
}

impl Nodemanger {
    pub async fn start_server(&self, settings: &Settings) -> Result<(), Error> {
        let manager = self.clone();
        // TODO: These should be authed
        let mint_service = Router::new()
            .route("/fund", get(get_funding_address))
            .route("/open-channel", post(post_new_open_channel))
            .route("/list-channels", get(get_list_channels))
            .route("/balance", get(get_balance))
            .route("/pay-invoice", post(post_pay_invoice))
            .route("/pay-keysend", post(post_pay_keysend))
            .route("/invoice", get(get_create_invoice))
            .route("/pay-on-chain", post(post_pay_on_chain))
            .route("/close-all", post(post_close_all))
            .with_state(manager);

        let ip = Ipv4Addr::from_str(&settings.info.listen_host)?;

        let port = 8086;

        let listen_addr = std::net::SocketAddr::new(std::net::IpAddr::V4(ip), port);

        axum::Server::bind(&listen_addr)
            .serve(mint_service.into_make_service())
            .await
            .map_err(|_| Error::Custom("Axum Server".to_string()))?;

        Ok(())
    }

    pub async fn new_onchain_address(&self) -> Result<FundingAddressResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let address = ldk.node.new_onchain_address()?;
                Ok(FundingAddressResponse {
                    address: address.to_string(),
                })
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn connect_open_channel(
        &self,
        open_channel_request: OpenChannelRequest,
    ) -> Result<StatusCode, Error> {
        let OpenChannelRequest {
            public_key,
            ip,
            port,
            amount,
            push_amount,
        } = open_channel_request;
        match &self {
            Nodemanger::Ldk(ldk) => {
                let peer_ip = Ipv4Addr::from_str(&ip)?;

                let peer_addr = SocketAddr::new(std::net::IpAddr::V4(peer_ip), port);

                let net_address = NetAddress::from(peer_addr);
                let _ = ldk.node.connect_open_channel(
                    public_key,
                    net_address,
                    amount,
                    push_amount,
                    None,
                    true,
                );
                Ok(StatusCode::OK)
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn list_channels(&self) -> Result<Vec<ChannelInfo>, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let chanels_details = ldk.node.list_channels();

                let channel_info = chanels_details.into_iter().map(|c| c.into()).collect();
                Ok(channel_info)
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn get_balance(&self) -> Result<BalanceResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let on_chain_total =
                    Amount::from_sat(ldk.node.total_onchain_balance_sats().unwrap());
                let on_chain_spendable =
                    Amount::from_sat(ldk.node.spendable_onchain_balance_sats().unwrap());
                let channel_info = ldk.node.list_channels();

                let ln = channel_info.into_iter().fold(Amount::ZERO, |acc, c| {
                    Amount::from_msat(c.balance_msat) + acc
                });

                Ok(BalanceResponse {
                    on_chain_total,
                    on_chain_spendable,
                    ln,
                })
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn pay_invoice(&self, bolt11: Bolt11) -> Result<PayInvoiceResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let p = bolt11.bolt11.payment_hash();

                let _res = ldk.node.send_payment(&bolt11.bolt11)?;

                let res = PayInvoiceResponse {
                    paymnet_hash: Sha256::from_str(&p.to_string())?,
                    status: InvoiceStatus::InFlight,
                };

                Ok(res)
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn pay_keysend(&self, keysend_request: KeysendRequest) -> Result<String, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let res = ldk
                    .node
                    .send_spontaneous_payment(keysend_request.amount, keysend_request.pubkey)?;

                Ok(String::from_utf8(res.0.to_vec()).unwrap())
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn create_invoice(
        &self,
        create_invoice_request: CreateInvoiceParams,
    ) -> Result<Bolt11, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let CreateInvoiceParams { msat, description } = create_invoice_request;

                let description = match description {
                    Some(des) => des,
                    None => {
                        // TODO: Get default from config
                        "Hello World".to_string()
                    }
                };

                let invoice = ldk
                    .node
                    .receive_payment(msat, &description, SECS_IN_DAY)
                    .unwrap();

                Ok(Bolt11 { bolt11: invoice })
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn send_to_onchain_address(
        &self,
        create_invoice_request: PayOnChainRequest,
    ) -> Result<String, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let address = Address::from_str(&create_invoice_request.address).unwrap();
                let res = ldk
                    .node
                    .send_to_onchain_address(&address, create_invoice_request.sat)?;

                Ok(res.to_string())
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn close_all(&self) -> Result<(), Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let channels = ldk.node.list_channels();
                let channels: Vec<(ldk_node::ChannelId, PublicKey)> = channels
                    .into_iter()
                    .map(|c| (c.channel_id, c.counterparty_node_id))
                    .collect();

                for (id, peer) in channels {
                    ldk.node.close_channel(&id, peer).unwrap();
                }

                Ok(())
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceParams {
    msat: u64,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysendRequest {
    amount: u64,
    pubkey: PublicKey,
}

async fn post_pay_keysend(
    State(state): State<Nodemanger>,
    Json(payload): Json<KeysendRequest>,
) -> Result<Json<String>, Error> {
    let res = state.pay_keysend(payload).await?;

    Ok(Json(res))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11 {
    bolt11: Invoice,
}

async fn post_pay_invoice(
    State(state): State<Nodemanger>,
    Json(payload): Json<Bolt11>,
) -> Result<Json<PayInvoiceResponse>, Error> {
    let p = state.pay_invoice(payload).await?;
    Ok(Json(p))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayInvoiceResponse {
    paymnet_hash: Sha256,
    status: InvoiceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingAddressResponse {
    address: String,
}

async fn get_funding_address(
    State(state): State<Nodemanger>,
) -> Result<Json<FundingAddressResponse>, Error> {
    let on_chain_balance = state.new_onchain_address().await?;

    Ok(Json(on_chain_balance))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    public_key: PublicKey,
    ip: String,
    port: u16,
    amount: u64,
    push_amount: Option<u64>,
}

async fn post_new_open_channel(
    State(state): State<Nodemanger>,
    Json(payload): Json<OpenChannelRequest>,
) -> Result<StatusCode, Error> {
    // TODO: Check if node has sufficient onchain balance

    if let Err(err) = state.connect_open_channel(payload).await {
        warn!("{:?}", err);
    };
    Ok(StatusCode::OK)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    peer_pubkey: PublicKey,
    balance: Amount,
    value: Amount,
    is_usable: bool,
}

impl From<ChannelDetails> for ChannelInfo {
    fn from(channel_details: ChannelDetails) -> Self {
        Self {
            peer_pubkey: channel_details.counterparty_node_id,
            balance: Amount::from_msat(channel_details.balance_msat),
            value: Amount::from_sat(channel_details.channel_value_sats),
            is_usable: channel_details.is_usable,
        }
    }
}

async fn get_list_channels(
    State(state): State<Nodemanger>,
) -> Result<Json<Vec<ChannelInfo>>, Error> {
    let channel_info = state.list_channels().await?;

    Ok(Json(channel_info))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    on_chain_spendable: Amount,
    on_chain_total: Amount,
    ln: Amount,
}

async fn get_balance(State(state): State<Nodemanger>) -> Result<Json<BalanceResponse>, Error> {
    let balance = state.get_balance().await?;

    Ok(Json(balance))
}

async fn get_create_invoice(
    State(state): State<Nodemanger>,
    Query(params): Query<CreateInvoiceParams>,
) -> Result<Json<Bolt11>, Error> {
    let bolt11 = state.create_invoice(params).await?;
    Ok(Json(bolt11))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayOnChainRequest {
    sat: u64,
    address: String,
}

async fn post_pay_on_chain(
    State(state): State<Nodemanger>,
    Json(payload): Json<PayOnChainRequest>,
) -> Result<Json<String>, Error> {
    let res = state.send_to_onchain_address(payload).await?;

    Ok(Json(res))
}

async fn post_close_all(State(state): State<Nodemanger>) -> Result<(), Error> {
    state.close_all().await
}

/// Possible states of an invoice
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum InvoiceStatus {
    Unpaid,
    Paid,
    Expired,
    InFlight,
}

impl From<ListinvoicesInvoicesStatus> for InvoiceStatus {
    fn from(status: ListinvoicesInvoicesStatus) -> Self {
        match status {
            ListinvoicesInvoicesStatus::UNPAID => Self::Unpaid,
            ListinvoicesInvoicesStatus::PAID => Self::Paid,
            ListinvoicesInvoicesStatus::EXPIRED => Self::Expired,
        }
    }
}

impl From<GL_ListInvoiceStatus> for InvoiceStatus {
    fn from(status: GL_ListInvoiceStatus) -> Self {
        match status {
            GL_ListInvoiceStatus::Unpaid => Self::Unpaid,
            GL_ListInvoiceStatus::Paid => Self::Paid,
            GL_ListInvoiceStatus::Expired => Self::Expired,
        }
    }
}

impl From<ldk_node::PaymentStatus> for InvoiceStatus {
    fn from(status: ldk_node::PaymentStatus) -> Self {
        match status {
            ldk_node::PaymentStatus::Pending => Self::Unpaid,
            ldk_node::PaymentStatus::Succeeded => Self::Paid,
            ldk_node::PaymentStatus::Failed => Self::Expired,
        }
    }
}

impl ToString for InvoiceStatus {
    fn to_string(&self) -> String {
        match self {
            InvoiceStatus::Paid => "Paid".to_string(),
            InvoiceStatus::Unpaid => "Unpaid".to_string(),
            InvoiceStatus::Expired => "Expired".to_string(),
            InvoiceStatus::InFlight => "InFlight".to_string(),
        }
    }
}

/// Possible states of an invoice
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum InvoiceTokenStatus {
    Issued,
    NotIssued,
}

/// Invoice information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceInfo {
    /// Payment hash of LN Invoice
    pub payment_hash: Sha256,
    /// random hash generated by the mint to internally look up the invoice state
    pub hash: Sha256,
    pub invoice: Invoice,
    pub amount: Amount,
    pub status: InvoiceStatus,
    pub token_status: InvoiceTokenStatus,
    pub memo: String,
    pub confirmed_at: Option<u64>,
}

impl InvoiceInfo {
    pub fn new(
        payment_hash: Sha256,
        hash: Sha256,
        invoice: Invoice,
        amount: Amount,
        status: InvoiceStatus,
        memo: &str,
        confirmed_at: Option<u64>,
    ) -> Self {
        Self {
            payment_hash,
            hash,
            invoice,
            amount,
            status,
            token_status: InvoiceTokenStatus::NotIssued,
            memo: memo.to_string(),
            confirmed_at,
        }
    }

    pub fn as_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(self)?)
    }
}

#[async_trait]
pub trait LnProcessor: Send + Sync {
    async fn get_invoice(
        &self,
        amount: Amount,
        hash: Sha256,
        description: &str,
    ) -> Result<InvoiceInfo, Error>;

    async fn wait_invoice(&self) -> Result<(), Error>;

    async fn pay_invoice(
        &self,
        invoice: Invoice,
        max_fee: Option<Amount>,
    ) -> Result<(String, Amount), Error>;

    async fn check_invoice_status(&self, payment_hash: &Sha256) -> Result<InvoiceStatus, Error>;
}
