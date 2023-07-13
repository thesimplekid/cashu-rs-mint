use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::middleware;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, SameSite};
use bitcoin::Address;
use cashu_crab::Amount;
use chrono::Duration;
use jwt_compact::{
    alg::{Hs256, Hs256Key},
    prelude::*,
};
use node_manager_types::TokenClaims;
use node_manager_types::{requests, responses, Bolt11};
use nostr::event::Event;
use std::net::Ipv4Addr;
use tower_http::cors::CorsLayer;
use tracing::warn;

pub use super::error::Error;
use super::jwt_auth::auth;
use super::{cln, greenlight, ldk};

use crate::config::Settings;
use crate::database::Db;
use crate::ln::LnNodeManager;

#[derive(Clone)]
pub enum Nodemanger {
    Ldk(Arc<ldk::Ldk>),
    Cln(Arc<cln::Cln>),
    Greenlight(Arc<greenlight::Greenlight>),
}

#[derive(Clone)]
pub struct NodeMangerState {
    pub ln: Nodemanger,
    pub db: Db,
    pub settings: Settings,
}

impl Nodemanger {
    pub async fn start_server(&self, settings: &Settings, db: Db) -> Result<(), Error> {
        let state = NodeMangerState {
            ln: self.clone(),
            db,
            settings: settings.clone(),
        };

        let state_arc = Arc::new(state.clone());
        // TODO: These should be authed
        let node_manager_service = Router::new()
            // Auth Routes
            .route("/nostr-login", post(post_nostr_login))
            .route(
                "/auth",
                post(post_check_auth)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            // Ln Routes
            .route(
                "/fund",
                get(get_funding_address)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/connect-peer",
                post(post_connect_peer)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/peers",
                get(get_peers).route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/open-channel",
                post(post_new_open_channel)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/channels",
                get(get_list_channels)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/balance",
                get(get_balance)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/pay-invoice",
                post(post_pay_invoice)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/pay-keysend",
                post(post_pay_keysend)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/invoice",
                get(get_create_invoice)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/pay-on-chain",
                post(post_pay_on_chain)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .route(
                "/close",
                post(post_close_channel)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            // Mint Routes
            .route(
                "/outstanding",
                get(in_circulation)
                    .route_layer(middleware::from_fn_with_state(state_arc.clone(), auth)),
            )
            .layer(CorsLayer::permissive())
            .with_state(state);

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
                let address = ldk.new_onchain_address().await?;
                Ok(responses::FundingAddressResponse {
                    address: address.to_string(),
                })
            }
            Nodemanger::Cln(cln) => {
                let address = cln.new_onchain_address().await?;
                Ok(responses::FundingAddressResponse {
                    address: address.to_string(),
                })
            }
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn connect_open_channel(
        &self,
        open_channel_request: requests::OpenChannelRequest,
    ) -> Result<StatusCode, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                ldk.open_channel(open_channel_request).await?;
                Ok(StatusCode::OK)
            }
            Nodemanger::Cln(cln) => {
                cln.open_channel(open_channel_request).await?;
                Ok(StatusCode::OK)
            }
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn list_channels(&self) -> Result<Vec<responses::ChannelInfo>, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                let channel_info = ldk.list_channels().await?;
                Ok(channel_info)
            }
            Nodemanger::Cln(cln) => {
                let channels = cln.list_channels().await?;

                Ok(channels)
            }
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn get_balance(&self) -> Result<responses::BalanceResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => ldk.get_balance().await,
            Nodemanger::Cln(cln) => cln.get_balance().await,
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn pay_invoice(
        &self,
        bolt11: Bolt11,
    ) -> Result<responses::PayInvoiceResponse, Error> {
        match &self {
            Nodemanger::Ldk(ldk) => ldk.pay_invoice(bolt11).await,
            Nodemanger::Cln(cln) => cln.pay_invoice(bolt11).await,
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn pay_keysend(
        &self,
        keysend_request: requests::KeysendRequest,
    ) -> Result<String, Error> {
        let amount = Amount::from_sat(keysend_request.amount);

        match &self {
            Nodemanger::Ldk(ldk) => ldk.pay_keysend(keysend_request.pubkey, amount).await,
            Nodemanger::Cln(cln) => cln.pay_keysend(keysend_request.pubkey, amount).await,
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }

    pub async fn create_invoice(
        &self,
        create_invoice_request: requests::CreateInvoiceParams,
    ) -> Result<Bolt11, Error> {
        let requests::CreateInvoiceParams { msat, description } = create_invoice_request;

        let description = match description {
            Some(des) => des,
            None => {
                // TODO: Get default from config
                "Hello World".to_string()
            }
        };

        let amount = Amount::from_msat(msat);

        let invoice = match &self {
            Nodemanger::Ldk(ldk) => ldk.create_invoice(amount, description).await?,
            Nodemanger::Cln(cln) => cln.create_invoice(amount, description).await?,
            Nodemanger::Greenlight(_gln) => todo!(),
        };

        Ok(Bolt11 { bolt11: invoice })
    }

    pub async fn send_to_onchain_address(
        &self,
        create_invoice_request: requests::PayOnChainRequest,
    ) -> Result<String, Error> {
        let amount = Amount::from_sat(create_invoice_request.sat);
        let address = Address::from_str(&create_invoice_request.address)
            .unwrap()
            .assume_checked();
        let txid = match &self {
            Nodemanger::Ldk(ldk) => ldk.pay_on_chain(address, amount).await?,
            Nodemanger::Cln(cln) => cln.pay_on_chain(address, amount).await?,
            Nodemanger::Greenlight(_gln) => todo!(),
        };

        Ok(txid)
    }

    pub async fn connect_peer(
        &self,
        connect_request: requests::ConnectPeerRequest,
    ) -> Result<responses::PeerInfo, Error> {
        let requests::ConnectPeerRequest {
            public_key,
            host,
            port,
        } = connect_request;
        let peer_info = match &self {
            Nodemanger::Ldk(ldk) => ldk.connect_peer(public_key, host, port).await?,
            Nodemanger::Cln(cln) => cln.connect_peer(public_key, host, port).await?,
            Nodemanger::Greenlight(gln) => gln.connect_peer(public_key, host, port).await?,
        };

        Ok(peer_info)
    }

    pub async fn peers(&self) -> Result<Vec<responses::PeerInfo>, Error> {
        let peers = match &self {
            Nodemanger::Ldk(ldk) => ldk.list_peers().await?,
            Nodemanger::Cln(cln) => cln.list_peers().await?,
            Nodemanger::Greenlight(gln) => gln.list_peers().await?,
        };

        Ok(peers)
    }

    pub async fn close(&self, close_channel_request: requests::CloseChannel) -> Result<(), Error> {
        match &self {
            Nodemanger::Ldk(ldk) => {
                ldk.close(
                    close_channel_request.channel_id,
                    close_channel_request.peer_id,
                )
                .await
            }
            Nodemanger::Cln(cln) => {
                cln.close(
                    close_channel_request.channel_id,
                    close_channel_request.peer_id,
                )
                .await
            }
            Nodemanger::Greenlight(_gln) => todo!(),
        }
    }
}

async fn post_nostr_login(
    State(state): State<NodeMangerState>,
    Json(payload): Json<Event>,
) -> Result<Response<String>, StatusCode> {
    let event = payload;

    event.verify().map_err(|_| StatusCode::UNAUTHORIZED)?;

    let authorized_users = state.settings.ln.authorized_users;

    if !authorized_users.contains(&event.pubkey) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let claims = TokenClaims {
        sub: event.pubkey.to_string(),
    };

    let time_options = TimeOptions::default();

    let claims = Claims::new(claims).set_duration_and_issuance(&time_options, Duration::hours(1));

    let key = Hs256Key::new(state.settings.ln.jwt_secret);
    let header: Header = Header::default();
    let token: String = Hs256.token(&header, &claims, &key).unwrap();

    let cookie = Cookie::build("token", token.to_owned())
        .path("/")
        .max_age(time::Duration::hours(1))
        .same_site(SameSite::Lax)
        .http_only(true)
        .finish();

    let mut response = Response::new(
        responses::LoginResponse {
            status: "OK".to_string(),
            token,
        }
        .as_json()
        .unwrap(),
    );
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

async fn post_check_auth() -> Result<StatusCode, StatusCode> {
    Ok(StatusCode::OK)
}

async fn post_connect_peer(
    State(state): State<NodeMangerState>,
    Json(payload): Json<requests::ConnectPeerRequest>,
) -> Result<StatusCode, Error> {
    let _res = state.ln.connect_peer(payload).await;
    Ok(StatusCode::OK)
}

async fn get_peers(
    State(state): State<NodeMangerState>,
) -> Result<Json<Vec<responses::PeerInfo>>, Error> {
    let peer_info = state.ln.peers().await?;

    Ok(Json(peer_info))
}

async fn post_close_channel(
    State(state): State<NodeMangerState>,
    Json(payload): Json<requests::CloseChannel>,
) -> Result<StatusCode, Error> {
    state.ln.close(payload).await?;

    Ok(StatusCode::OK)
}

async fn post_pay_keysend(
    State(state): State<NodeMangerState>,
    Json(payload): Json<requests::KeysendRequest>,
) -> Result<Json<String>, Error> {
    let res = state.ln.pay_keysend(payload).await?;

    Ok(Json(res))
}

async fn post_pay_invoice(
    State(state): State<NodeMangerState>,
    Json(payload): Json<Bolt11>,
) -> Result<Json<responses::PayInvoiceResponse>, Error> {
    let p = state.ln.pay_invoice(payload).await?;
    Ok(Json(p))
}

async fn get_funding_address(
    State(state): State<NodeMangerState>,
) -> Result<Json<responses::FundingAddressResponse>, Error> {
    let on_chain_balance = state.ln.new_onchain_address().await?;

    Ok(Json(on_chain_balance))
}

async fn post_new_open_channel(
    State(state): State<NodeMangerState>,
    Json(payload): Json<requests::OpenChannelRequest>,
) -> Result<StatusCode, Error> {
    // TODO: Check if node has sufficient onchain balance

    if let Err(err) = state.ln.connect_open_channel(payload).await {
        warn!("{:?}", err);
    };
    Ok(StatusCode::OK)
}

async fn get_list_channels(
    State(state): State<NodeMangerState>,
) -> Result<Json<Vec<responses::ChannelInfo>>, Error> {
    let channel_info = state.ln.list_channels().await?;

    Ok(Json(channel_info))
}

async fn get_balance(
    State(state): State<NodeMangerState>,
) -> Result<Json<responses::BalanceResponse>, Error> {
    let balance = state.ln.get_balance().await?;

    Ok(Json(balance))
}

async fn get_create_invoice(
    State(state): State<NodeMangerState>,
    Query(params): Query<requests::CreateInvoiceParams>,
) -> Result<Json<Bolt11>, Error> {
    let bolt11 = state.ln.create_invoice(params).await?;
    Ok(Json(bolt11))
}

async fn post_pay_on_chain(
    State(state): State<NodeMangerState>,
    Json(payload): Json<requests::PayOnChainRequest>,
) -> Result<Json<String>, Error> {
    let res = state.ln.send_to_onchain_address(payload).await?;

    Ok(Json(res))
}

async fn in_circulation(State(state): State<NodeMangerState>) -> Result<Json<Amount>, Error> {
    let amount = state.db.get_in_circulation().await?;

    Ok(Json(amount))
}
