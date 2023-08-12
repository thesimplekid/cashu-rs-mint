use std::collections::HashMap;

use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use gloo_net::http::Request;
use ln_rs::node_manager_types::responses::{self, ChannelInfo};
use serde_json::Value;
use url::Url;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::components::connect_peer::ConnectPeer;

use super::channel::Channel;
use super::open_channel::OpenChannel;

#[derive(Properties, PartialEq, Clone)]
pub struct Props {
    pub url: Url,
    pub jwt: String,
}

pub enum Msg {
    OpenChannelView,
    ConnectPeerView,
    FetechedChannels(Vec<ChannelInfo>),
    FetechedPeers(Vec<responses::PeerInfo>),
    Back,
}

#[derive(Default)]
enum View {
    #[default]
    Channels,
    OpenChannel,
    ConnectPeer,
}

#[derive(Default)]
pub struct Channels {
    view: View,
    channels: Vec<ChannelInfo>,
    peers: HashMap<PublicKey, responses::PeerInfo>,
}

impl Component for Channels {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let channels_callback = ctx.link().callback(Msg::FetechedChannels);
        let peers_callback = ctx.link().callback(Msg::FetechedPeers);

        let jwt = ctx.props().jwt.clone();
        let url = ctx.props().url.clone();
        spawn_local(async move {
            get_channels(&jwt, &url, channels_callback).await.ok();
            get_peers(&jwt, &url, peers_callback).await.unwrap();
        });

        Self::default()
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::OpenChannelView => {
                self.view = View::OpenChannel;
                true
            }
            Msg::Back => {
                self.view = View::Channels;
                true
            }
            Msg::FetechedChannels(channels) => {
                self.channels = channels;
                true
            }
            Msg::FetechedPeers(peers) => {
                let peers = peers.into_iter().fold(HashMap::new(), |mut acc, x| {
                    acc.insert(x.peer_pubkey, x);
                    acc
                });
                self.peers = peers;
                true
            }
            Msg::ConnectPeerView => {
                self.view = View::ConnectPeer;
                true
            }
        }
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        let open_channel_button = ctx.link().callback(|_| Msg::OpenChannelView);
        let connect_peer_button = ctx.link().callback(|_| Msg::ConnectPeerView);

        html! {
        <>
            <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Channels" } </h5>

                {
                    match &self.view {
                        View::Channels => {
                            html!{
                                <>
                                    {
                                        self.channels.iter().map(|channel| {
                                            let remote_balance = channel.value - channel.balance;
                                            html!{
                                                <Channel jwt={ctx.props().jwt.clone()} channel_id={channel.channel_id.clone()} peer_id={channel.peer_pubkey} local_balance={channel.balance} {remote_balance} status={channel.status} url={ctx.props().url.clone()}/>
                                            }
                                        }).collect::<Html>()
                                    }
                                    <div class="flex space-x-2">
                                        <button onclick={open_channel_button} class="flex-1 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                                            { "Open Channel" }
                                        </button>
                                        <button onclick={connect_peer_button} class="flex-1 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                                            { "Connect Peer" }
                                        </button>
                                    </div>
                                </>
                            }
                        }
                        View::OpenChannel => {
                            let back = ctx.link().callback(|_| Msg::Back);
                            html!{
                                <OpenChannel jwt={ctx.props().jwt.clone()} url={ctx.props().url.clone()} peers={self.peers.clone()} back_callback={back}/>
                            }
                        }
                        View::ConnectPeer => {
                            let open_channel_cb = ctx.link().callback(|_| Msg::OpenChannelView);
                            let back = ctx.link().callback(|_| Msg::Back);
                            html!{
                                <ConnectPeer jwt={ctx.props().jwt.clone()} url={ctx.props().url.clone()} back_callback={back} {open_channel_cb}/>
                            }
                        }
                    }
                }
            </a>
        </>

        }
    }
}

async fn get_channels(
    jwt: &str,
    url: &Url,
    got_channels_cb: Callback<Vec<ChannelInfo>>,
) -> Result<()> {
    let url = url.join("channels")?;

    let fetched_channels: Vec<ChannelInfo> = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    got_channels_cb.emit(fetched_channels);

    Ok(())
}

async fn get_peers(
    jwt: &str,
    url: &Url,
    got_peers_cb: Callback<Vec<responses::PeerInfo>>,
) -> Result<()> {
    let url = url.join("peers")?;
    let fetched_channels: Value = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    let peers = serde_json::from_value(fetched_channels)?;

    got_peers_cb.emit(peers);

    Ok(())
}
