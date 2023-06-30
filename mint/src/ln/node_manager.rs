use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use cashu_crab::{lightning_invoice::Invoice, types, Amount, Sha256};
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::bitcoin::util::address::Address;
use ldk_node::{ChannelDetails, NetAddress};
use log::warn;
use node_manager_types::responses::ChannelInfo;
use node_manager_types::{requests, responses, Bolt11};
use std::net::{Ipv4Addr, SocketAddr};
use tower_http::cors::CorsLayer;

pub use super::error::Error;
use super::{cashu_crab_invoice, cln, greenlight, ldk, InvoiceStatus};

use crate::config::Settings;

const SECS_IN_DAY: u32 = 86400;

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
        let node_manager_service = Router::new()
            .route("/fund", get(get_funding_address))
            .route("/open-channel", post(post_new_open_channel))
            .route("/channels", get(get_list_channels))
            .route("/balance", get(get_balance))
            .route("/pay-invoice", post(post_pay_invoice))
            .route("/pay-keysend", post(post_pay_keysend))
            .route("/invoice", get(get_create_invoice))
            .route("/pay-on-chain", post(post_pay_on_chain))
            .route("/close-all", post(post_close_all))
            .layer(CorsLayer::permissive())
            .with_state(manager);

        let ip = Ipv4Addr::from_str(&settings.info.listen_host)?;

        let port = 8086;

        let listen_addr = std::net::SocketAddr::new(std::net::IpAddr::V4(ip), port);

        axum::Server::bind(&listen_addr)
            .serve(node_manager_service.into_make_service())
            .await
            .map_err(|_| Error::Custom("Axum Server".to_string()))?;

        Ok(())
    }

    pub async fn new_onchain_address(&self) -> Result<responses::FundingAddressResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let address = ldk.node.new_onchain_address()?;
                Ok(responses::FundingAddressResponse {
                    address: address.to_string(),
                })
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn connect_open_channel(
        &self,
        open_channel_request: requests::OpenChannelRequest,
    ) -> Result<StatusCode, Error> {
        let requests::OpenChannelRequest {
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
                let node_pubkey =
                    ldk_node::bitcoin::secp256k1::PublicKey::from_slice(&public_key.serialize())
                        .unwrap();

                let push_amount = push_amount.map(|a| a.to_msat());

                let _ = ldk.node.connect_open_channel(
                    node_pubkey,
                    net_address,
                    amount.into(),
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

    pub async fn list_channels(&self) -> Result<Vec<responses::ChannelInfo>, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let channels_details = ldk.node.list_channels();

                let channel_info = channels_details
                    .into_iter()
                    .map(|c| channel_info_from_details(c))
                    .collect();
                Ok(channel_info)
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn get_balance(&self) -> Result<responses::BalanceResponse, Error> {
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

                Ok(responses::BalanceResponse {
                    on_chain_total,
                    on_chain_spendable,
                    ln,
                })
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn pay_invoice(
        &self,
        bolt11: Bolt11,
    ) -> Result<responses::PayInvoiceResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let p = bolt11.bolt11.payment_hash();

                let _res = ldk.node.send_payment(&bolt11.bolt11)?;

                let res = responses::PayInvoiceResponse {
                    paymnet_hash: Sha256::from_str(&p.to_string())?,
                    status: cashu_crab_invoice(InvoiceStatus::InFlight),
                };

                Ok(res)
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn pay_keysend(
        &self,
        keysend_request: requests::KeysendRequest,
    ) -> Result<String, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let node_pubkey = ldk_node::bitcoin::secp256k1::PublicKey::from_slice(
                    keysend_request.pubkey.to_string().as_bytes(),
                )
                .unwrap();
                let res = ldk
                    .node
                    .send_spontaneous_payment(keysend_request.amount, node_pubkey)?;

                Ok(String::from_utf8(res.0.to_vec()).unwrap())
            }
            Nodemanger::Cln(_cln) => todo!(),
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn create_invoice(
        &self,
        create_invoice_request: requests::CreateInvoiceParams,
    ) -> Result<Bolt11, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let requests::CreateInvoiceParams { msat, description } = create_invoice_request;

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
        create_invoice_request: requests::PayOnChainRequest,
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

async fn post_pay_keysend(
    State(state): State<Nodemanger>,
    Json(payload): Json<requests::KeysendRequest>,
) -> Result<Json<String>, Error> {
    let res = state.pay_keysend(payload).await?;

    Ok(Json(res))
}

async fn post_pay_invoice(
    State(state): State<Nodemanger>,
    Json(payload): Json<Bolt11>,
) -> Result<Json<responses::PayInvoiceResponse>, Error> {
    let p = state.pay_invoice(payload).await?;
    Ok(Json(p))
}

async fn get_funding_address(
    State(state): State<Nodemanger>,
) -> Result<Json<responses::FundingAddressResponse>, Error> {
    let on_chain_balance = state.new_onchain_address().await?;

    Ok(Json(on_chain_balance))
}

async fn post_new_open_channel(
    State(state): State<Nodemanger>,
    Json(payload): Json<requests::OpenChannelRequest>,
) -> Result<StatusCode, Error> {
    // TODO: Check if node has sufficient onchain balance

    if let Err(err) = state.connect_open_channel(payload).await {
        warn!("{:?}", err);
    };
    Ok(StatusCode::OK)
}

async fn get_list_channels(
    State(state): State<Nodemanger>,
) -> Result<Json<Vec<responses::ChannelInfo>>, Error> {
    let channel_info = state.list_channels().await?;

    Ok(Json(channel_info))
}

async fn get_balance(
    State(state): State<Nodemanger>,
) -> Result<Json<responses::BalanceResponse>, Error> {
    let balance = state.get_balance().await?;

    Ok(Json(balance))
}

async fn get_create_invoice(
    State(state): State<Nodemanger>,
    Query(params): Query<requests::CreateInvoiceParams>,
) -> Result<Json<Bolt11>, Error> {
    let bolt11 = state.create_invoice(params).await?;
    Ok(Json(bolt11))
}

async fn post_pay_on_chain(
    State(state): State<Nodemanger>,
    Json(payload): Json<requests::PayOnChainRequest>,
) -> Result<Json<String>, Error> {
    let res = state.send_to_onchain_address(payload).await?;

    Ok(Json(res))
}

async fn post_close_all(State(state): State<Nodemanger>) -> Result<StatusCode, Error> {
    state.close_all().await?;

    Ok(StatusCode::OK)
}

pub fn channel_info_from_details(details: ChannelDetails) -> ChannelInfo {
    let peer_pubkey =
        bitcoin::secp256k1::PublicKey::from_str(&details.counterparty_node_id.to_string()).unwrap();
    ChannelInfo {
        peer_pubkey,
        balance: Amount::from_msat(details.balance_msat),
        value: Amount::from_sat(details.channel_value_sats),
        is_usable: details.is_usable,
    }
}