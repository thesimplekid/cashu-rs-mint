use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bitcoin_hashes::Hash;
use cashu_crab::Amount;
use cashu_crab::Sha256;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::bitcoin::util::address::Address;
use ldk_node::bitcoin::Network;
use ldk_node::io::SqliteStore;
use ldk_node::lightning_invoice::Invoice;
use ldk_node::ChannelDetails;
use ldk_node::{Builder, Config, NetAddress};
use ldk_node::{Event, Node};
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::config::Settings;
use crate::database::Db;
use crate::utils::unix_time;

use super::Error;
use super::InvoiceInfo;
use super::InvoiceStatus;
use super::LnProcessor;

const SECS_IN_DAY: u32 = 86400;

#[derive(Clone)]
pub struct Ldk {
    node: Arc<Node<SqliteStore>>,
    db: Db,
}

#[derive(Clone)]
pub struct LdkState {
    node: Arc<Node<SqliteStore>>,
}

impl Ldk {
    pub async fn new(settings: &Settings, db: Db) -> anyhow::Result<Self> {
        let config = Config {
            log_level: ldk_node::LogLevel::Info,
            ..Default::default()
        };
        let mut builder = Builder::from_config(config);
        builder.set_entropy_seed_path("./myseed".to_string());
        builder.set_network(Network::Testnet);
        builder.set_esplora_server("https://blockstream.info/testnet/api".to_string());
        builder.set_gossip_source_rgs(
            "https://rapidsync.lightningdevkit.org/testnet/snapshot".to_string(),
        );

        let node = Arc::new(builder.build()?);

        node.start()?;

        let state = LdkState { node: node.clone() };

        // TODO: These should be authed
        let mint_service = Router::new()
            .route("/fund", get(get_funding_address))
            .route("/open-channel", post(post_new_open_channel))
            .route("/list-channels", get(get_list_channels))
            .route("/balance", get(get_balances))
            // TODO: Close channel
            // TODO: Pay invoice
            .route("/pay-invoice", post(post_pay_invoice))
            .route("/pay-keysend", post(post_pay_keysend))
            .route("/invoice", get(get_create_invoice))
            .route("/pay-on-chain", post(post_pay_on_chain))
            .with_state(state);

        let ip = Ipv4Addr::from_str(&settings.info.listen_host)?;

        let port = 8086;

        let listen_addr = SocketAddr::new(std::net::IpAddr::V4(ip), port);

        tokio::spawn(async move {
            if let Err(err) = axum::Server::bind(&listen_addr)
                .serve(mint_service.into_make_service())
                .await
            {
                warn!("{:?}", err)
            }
        });

        // let funding_address = node.new_onchain_address()?;

        // info!("Funding Address: {}", funding_address);

        Ok(Self {
            node: node.clone(),
            db: db.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseChannelRequest {
    peer_pubkey: PublicKey,
    channel_id: String,
}

/*
async fn post_close_channel(
    State(state): State<LdkState>,
    // FIXME: Stop using query string
    Query(params): Query<CloseChannelRequest>,
) -> Result<StatusCode, StatusCode> {
    // let channel_id = ChannelId(params.channel_id.clone().as_bytes().to_owned().as_slice());
    Ok(StatusCode::OK)
}

*/

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayOnChainRequest {
    sat: u64,
    address: String,
}

async fn post_pay_on_chain(
    State(state): State<LdkState>,
    // Stop using query string
    Query(params): Query<PayOnChainRequest>,
) -> Result<Json<String>, StatusCode> {
    let address = Address::from_str(&params.address).unwrap();
    let res = state
        .node
        .send_to_onchain_address(&address, params.sat)
        .unwrap();

    Ok(Json(res.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceParams {
    msat: u64,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11 {
    bolt11: Invoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysendRequest {
    amount: u64,
    pubkey: PublicKey,
}

async fn post_pay_keysend(
    State(state): State<LdkState>,
    // Stop using query string
    Query(params): Query<KeysendRequest>,
) -> Result<Json<String>, StatusCode> {
    let res = state
        .node
        .send_spontaneous_payment(params.amount, params.pubkey)
        .unwrap();

    Ok(Json(String::from_utf8(res.0.to_vec()).unwrap()))
}

async fn post_pay_invoice(
    State(state): State<LdkState>,
    // Stop using query string
    Query(params): Query<Bolt11>,
) -> Result<Json<String>, StatusCode> {
    let res = state.node.send_payment(&params.bolt11).unwrap();

    Ok(Json(String::from_utf8(res.0.to_vec()).unwrap()))
}

async fn get_create_invoice(
    State(state): State<LdkState>,
    Query(params): Query<CreateInvoiceParams>,
) -> Result<Json<Bolt11>, StatusCode> {
    let CreateInvoiceParams { msat, description } = params;

    let invoice = state
        .node
        .receive_payment(msat, &description, SECS_IN_DAY)
        .unwrap();

    Ok(Json(Bolt11 { bolt11: invoice }))
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
    State(state): State<LdkState>,
) -> Result<Json<Vec<ChannelInfo>>, StatusCode> {
    let channel_info = state.node.list_channels();

    Ok(Json(channel_info.into_iter().map(|c| c.into()).collect()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenChannelParams {
    public_key: PublicKey,
    ip: String,
    port: u16,
    amount: u64,
    push_amount: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BalanceResponse {
    on_chain_spendable: Amount,
    on_chain_total: Amount,
    ln: Amount,
}

async fn get_balances(State(state): State<LdkState>) -> Result<Json<BalanceResponse>, StatusCode> {
    let on_chain_total = Amount::from_sat(state.node.total_onchain_balance_sats().unwrap());
    let on_chain_spendable = Amount::from_sat(state.node.spendable_onchain_balance_sats().unwrap());
    let channel_info = state.node.list_channels();

    let ln = channel_info.into_iter().fold(Amount::ZERO, |acc, c| {
        Amount::from_msat(c.balance_msat) + acc
    });

    Ok(Json(BalanceResponse {
        on_chain_spendable,
        on_chain_total,
        ln,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FundingAddressResponse {
    address: String,
}

async fn get_funding_address(
    State(state): State<LdkState>,
) -> Result<Json<FundingAddressResponse>, StatusCode> {
    let on_chain_balance = state.node.new_onchain_address().unwrap();

    Ok(Json(FundingAddressResponse {
        address: on_chain_balance.to_string(),
    }))
}

async fn post_new_open_channel(
    State(state): State<LdkState>,
    Query(params): Query<OpenChannelParams>,
) -> Result<StatusCode, StatusCode> {
    let OpenChannelParams {
        public_key,
        ip,
        port,
        amount,
        push_amount,
    } = params;

    // TODO: Check if node has sufficient onchain balance

    let peer_ip = Ipv4Addr::from_str(&ip).unwrap();

    let peer_addr = SocketAddr::new(std::net::IpAddr::V4(peer_ip), port);

    let net_address = NetAddress::from(peer_addr);
    /*
    if let Err(err) = state.node.connect(public_key, net_address, true) {
        warn!("{:?}", err);
    };
    */

    if let Err(err) =
        state
            .node
            .connect_open_channel(public_key, net_address, amount, push_amount, None, true)
    {
        warn!("{:?}", err);
    };
    Ok(StatusCode::OK)
}

#[async_trait]
impl LnProcessor for Ldk {
    async fn get_invoice(
        &self,
        amount: Amount,
        hash: Sha256,
        description: &str,
    ) -> Result<InvoiceInfo, Error> {
        let invoice = self
            .node
            .receive_payment(amount.to_msat(), description, SECS_IN_DAY)?;

        let inoice_info = InvoiceInfo::new(
            Sha256::from_str(&invoice.payment_hash().to_owned().to_string())?,
            hash,
            invoice,
            amount,
            InvoiceStatus::Unpaid,
            "",
            None,
        );
        Ok(inoice_info)
    }

    async fn wait_invoice(&self) -> Result<(), Error> {
        while let Some(event) = self.node.next_event() {
            match event {
                Event::PaymentReceived {
                    payment_hash,
                    amount_msat: _,
                } => {
                    let payment_hash =
                        Sha256::from_str(&String::from_utf8(payment_hash.0.to_vec())?)?;

                    let mut invoice_info = self
                        .db
                        .get_invoice_info_by_payment_hash(&payment_hash)
                        .await?;

                    invoice_info.status = InvoiceStatus::Paid;
                    invoice_info.confirmed_at = Some(unix_time());

                    self.db.add_invoice(&invoice_info).await?;

                    self.node.event_handled();
                }
                _ => {
                    debug!("{:?}", event);
                    // TODO: Do something with this
                    self.node.event_handled();
                }
            }
        }
        Ok(())
    }

    async fn check_invoice_status(&self, payment_hash: &Sha256) -> Result<InvoiceStatus, Error> {
        let payment_hash = ldk_node::lightning::ln::PaymentHash(payment_hash.to_byte_array());

        let payment = self
            .node
            .list_payments_with_filter(|p| p.hash == payment_hash);

        let status = payment[0].status.into();

        Ok(status)
    }

    async fn pay_invoice(
        &self,
        invoice: Invoice,
        _max_fee: Option<Amount>,
    ) -> Result<(String, Amount), Error> {
        let payment_hash = self.node.send_payment(&invoice)?;
        let payment = self
            .node
            .list_payments_with_filter(|p| p.hash == payment_hash);

        Ok((
            String::from_utf8(payment_hash.0.to_vec())?,
            Amount::from_msat(payment[0].amount_msat.unwrap()),
        ))
    }
}
