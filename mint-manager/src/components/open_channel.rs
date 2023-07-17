use std::collections::HashMap;
use std::str::FromStr;

use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use cashu_crab::Amount;
use gloo_net::http::Request;
use log::warn;
use node_manager_types::requests::OpenChannelRequest;
use node_manager_types::responses::{self, ChannelInfo};
use url::Url;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::platform::spawn_local;
use yew::prelude::*;

async fn post_open_channel(
    jwt: &str,
    url: &Url,
    open_channel_request: OpenChannelRequest,
    open_channel_callback: Callback<String>,
) -> Result<()> {
    let url = url.join("open-channel")?;
    let _fetched_channels: ChannelInfo = Request::post(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .json(&open_channel_request)?
        .send()
        .await?
        .json()
        .await?;

    open_channel_callback.emit("".to_string());

    Ok(())
}

async fn get_peers(
    jwt: &str,
    url: &Url,
    peers_callback: Callback<Vec<responses::PeerInfo>>,
) -> Result<()> {
    let url = url.join("peers")?;
    let peers: Vec<responses::PeerInfo> = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    peers_callback.emit(peers);

    Ok(())
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub jwt: String,
    pub url: Url,
    pub peers: HashMap<PublicKey, responses::PeerInfo>,
    pub back_callback: Callback<MouseEvent>,
}

pub enum Msg {
    Submit,
    ChannelOpened(String),
    FetechedPeers(Vec<responses::PeerInfo>),
}

#[derive(Default)]
enum View {
    #[default]
    OpenChannel,
    OpenedChannel,
}

#[derive(Default)]
pub struct OpenChannel {
    view: View,
    amount_input_node_ref: NodeRef,
    push_amount_input_node_ref: NodeRef,
    select_node_ref: NodeRef,
    peers: Vec<responses::PeerInfo>,
}

impl Component for OpenChannel {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let callback = ctx.link().callback(Msg::FetechedPeers);
        let jwt = ctx.props().jwt.clone();
        let url = ctx.props().url.clone();
        spawn_local(async move {
            if let Err(err) = get_peers(&jwt, &url, callback).await {
                warn!("Could not get peers: {:?}", err);
            }
        });
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Submit => {
                log::debug!("{:?}", self.select_node_ref.cast::<HtmlInputElement>());

                let pubkey = self
                    .select_node_ref
                    .cast::<HtmlSelectElement>()
                    .map(|p| PublicKey::from_str(&p.value()));

                let amount = self
                    .amount_input_node_ref
                    .cast::<HtmlInputElement>()
                    .map(|i| {
                        let value = i.value();
                        value.parse::<u64>()
                    });

                let push_amount = self
                    .push_amount_input_node_ref
                    .cast::<HtmlInputElement>()
                    .map(|i| {
                        let value = i.value();
                        value.parse::<u64>()
                    });

                if let (Some(Ok(public_key)), Some(Ok(amount)), Some(Ok(push_amount))) =
                    (pubkey, amount, push_amount)
                {
                    if let Some(peer) = ctx.props().peers.get(&public_key) {
                        let host = peer.host.clone();
                        let port = peer.port;

                        let amount = Amount::from_sat(amount);
                        let push_amount = if push_amount > 0 {
                            Some(Amount::from_sat(push_amount))
                        } else {
                            None
                        };

                        let open_channel = OpenChannelRequest {
                            public_key,
                            host,
                            port,
                            amount,
                            push_amount,
                        };

                        let callback = ctx.link().callback(Msg::ChannelOpened);
                        let jwt = ctx.props().jwt.clone();
                        let url = ctx.props().url.clone();

                        spawn_local(async move {
                            if let Err(err) =
                                post_open_channel(&jwt, &url, open_channel, callback).await
                            {
                                warn!("Failed to open channel: {:?}", err);
                            }
                        });
                    } else {
                        warn!("Peer is missing");
                    }
                } else {
                    warn!("Something is missing");
                }

                false
            }
            Msg::ChannelOpened(_response) => {
                self.view = View::OpenedChannel;
                true
            }
            Msg::FetechedPeers(peers) => {
                self.peers = peers;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_submit = ctx.link().callback(|_| Msg::Submit);

        html! {
            <>
                <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                    <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Open Channel" } </h5>
                    {
                        match self.view {
                            View::OpenChannel => {
                                html! {
                                    <>
                                        <div class="relative z-0 w-full mb-6 group">
                                        </div>
                                        <div class="relative z-0 w-full mb-6 group">
                                            <select
                                                ref={self.select_node_ref.clone()}
                                                class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer"
                                            >
                                                <option value="" disabled=true>
                                                    { "Select Peer" }
                                                </option>
                                                { for self.peers.iter().map(|p| {
                                                    html! {
                                                        <option value={p.peer_pubkey.to_string()}>
                                                            { p.peer_pubkey }
                                                        </option>
                                                    }
                                                })}
                                            </select>
                                        </div>
                                        <div class="relative z-0 w-full mb-6 group">
                                            <input
                                                type="numeric"
                                                name="channel_size"
                                                id="channel_size"
                                                class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer"
                                                ref={self.amount_input_node_ref.clone()}
                                            />
                                            <label for="channel_size" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Channel Size (sat)"}</label>
                                        </div>
                                        <div class="relative z-0 w-full mb-6 group">
                                            <input
                                                type="numeric"
                                                name="push_amount"
                                                id="push_amount"
                                                class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer"
                                                ref={self.push_amount_input_node_ref.clone()}
                                            />
                                            <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Push Amount"}</label>
                                        </div>
                                        <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={on_submit}>{"Open Channel"}</button>
                                        <button class="px-6 py-2 rounded-sm" onclick={ctx.props().back_callback.clone()}>{"Back"}</button>
                                    </>
                                }
                            }
                            View::OpenedChannel => {
                                html! {
                                    <>
                                        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Channel Opened" } </h5>
                                        <button class="px-6 py-2 rounded-sm" onclick={ctx.props().back_callback.clone()}>{"Back"}</button>
                                    </>
                                }
                            }
                        }
                    }
                </a>
            </>
        }
    }
}
