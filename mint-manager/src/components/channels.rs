use anyhow::Result;
use gloo_net::http::Request;
use node_manager_types::responses::{self, ChannelInfo};
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::components::connect_peer::ConnectPeer;

use super::channel::Channel;
use super::open_channel::OpenChannel;

#[derive(Properties, PartialEq, Default, Clone)]
pub struct Props {
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
    peers: Vec<responses::PeerInfo>,
}

impl Component for Channels {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let channels_callback = ctx.link().callback(Msg::FetechedChannels);
        let peers_callback = ctx.link().callback(Msg::FetechedPeers);

        let jwt = ctx.props().jwt.clone();
        spawn_local(async move {
            get_channels(&jwt, channels_callback).await.ok();
            get_peers(&jwt, peers_callback).await.ok();
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
                                                           <Channel jwt={ctx.props().jwt.clone()} channel_id={channel.channel_id.clone()} peer_id= {channel.peer_pubkey} local_balance={channel.balance} {remote_balance} status={channel.status}/>
                                                               }}).collect::<Html>()
                                                            }
                                                           <button onclick={open_channel_button} class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                                                            { "Open Channel" }

                                                            </button>
                                                           <button onclick={connect_peer_button} class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                                                            { "Connect Peer" }

                                            </button>
                                            </>
                                        }
                                   }
                               View::OpenChannel => {
                        let back = ctx.link().callback(|_| Msg::Back);
                                   html!{ <OpenChannel  jwt={ctx.props().jwt.clone()} peers={self.peers.clone()} back_callback={back}/> }
                               }
                                View::ConnectPeer => {
                        let open_channel_cb = ctx.link().callback(|_| Msg::OpenChannelView);
                        let back = ctx.link().callback(|_| Msg::Back);
                                   html!{ <ConnectPeer  jwt={ctx.props().jwt.clone()} back_callback={back} {open_channel_cb}/> }

                    }
                }
        }

                           </a>
                                       </>
                  }
    }
}

async fn get_channels(jwt: &str, got_channels_cb: Callback<Vec<ChannelInfo>>) -> Result<()> {
    let fetched_channels: Vec<ChannelInfo> = Request::get("http://127.0.0.1:8086/channels")
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    got_channels_cb.emit(fetched_channels);

    Ok(())
}

async fn get_peers(jwt: &str, got_peers_cb: Callback<Vec<responses::PeerInfo>>) -> Result<()> {
    let fetched_channels: Vec<responses::PeerInfo> = Request::get("http://127.0.0.1:8086/peers")
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    got_peers_cb.emit(fetched_channels);

    Ok(())
}
