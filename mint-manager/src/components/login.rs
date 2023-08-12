use std::str::FromStr;

use gloo::storage::{LocalStorage, Storage};
use gloo_net::http::Request;
use ln_rs::node_manager_types::responses::LoginResponse;
use log::{debug, warn};
use nostr::event::builder::EventBuilder;
use nostr::event::kind::Kind;
use nostr::event::unsigned::UnsignedEvent;
use nostr::event::Event;
use nostr::key::Keys;
use nostr::secp256k1::XOnlyPublicKey;
use url::Url;
use yew::prelude::*;

use crate::app::JWT_KEY;
use crate::bindings;

#[derive(Properties, PartialEq, Clone)]
pub struct Props {
    pub node_url: Url,
    pub logged_in_callback: Callback<String>,
}

async fn get_pubkey() -> Option<String> {
    let key = bindings::get_pubkey().await;
    key.as_string()
}

async fn sign_event(event: UnsignedEvent) -> Option<Event> {
    let signed_event = bindings::sign_event(
        event.created_at.as_i64(),
        event.content,
        event.pubkey.to_string(),
    )
    .await
    .as_string();

    if let Some(event) = signed_event {
        let event: Event = serde_json::from_str(&event).unwrap();
        debug!("sig: {:?}", event.as_json());
        return Some(event);
    }

    None
}

pub enum Msg {
    Login,
    GotPubkey(XOnlyPublicKey),
    EventSigned(Event),
    LoggedIn(String),
    Error(String),
}

pub struct Login;

impl Component for Login {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Login => {
                ctx.link().send_future(async {
                    match get_pubkey().await {
                        Some(pubkey) => {
                            let pubkey = XOnlyPublicKey::from_str(&pubkey).unwrap();

                            Msg::GotPubkey(pubkey)
                        }
                        None => Msg::Error("".to_string()),
                    }
                });
                false
            }
            Msg::GotPubkey(pubkey) => {
                let keys = Keys::from_public_key(pubkey);

                let event =
                    EventBuilder::new(Kind::TextNote, "", &[]).to_unsigned_event(keys.public_key());

                ctx.link().send_future(async move {
                    match sign_event(event).await {
                        Some(event) => Msg::EventSigned(event),
                        None => Msg::Error("".to_string()),
                    }
                });

                false
            }
            Msg::EventSigned(signed_event) => {
                let url = ctx.props().node_url.join("nostr-login").unwrap();
                ctx.link().send_future(async move {
                    match Request::post(url.as_str())
                        .json(&signed_event)
                        .unwrap()
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                    {
                        Ok(login_response) => {
                            let login: LoginResponse = login_response;

                            LocalStorage::set(JWT_KEY, login.token.clone()).unwrap();

                            Msg::LoggedIn(login.token)
                        }
                        Err(err) => Msg::Error(err.to_string()),
                    }
                });

                false
            }
            Msg::LoggedIn(jwt) => {
                ctx.props().logged_in_callback.emit(jwt);
                false
            }
            Msg::Error(err) => {
                warn!("{}", err);
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let onclick = ctx.link().callback(|_| Msg::Login);
        html! {
        <button type="button" class="focus:outline-none text-white bg-red-700 hover:bg-red-800 focus:ring-4 focus:ring-red-300 font-medium rounded-lg text-sm px-5 py-2.5 mr-2 mb-2 dark:bg-red-600 dark:hover:bg-red-700 dark:focus:ring-red-900" {onclick}> {"Sign In"} </button>
            }
    }
}
