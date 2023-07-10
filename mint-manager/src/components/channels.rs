use std::str::FromStr;

use anyhow::Result;
use bitcoin::secp256k1::{PublicKey, XOnlyPublicKey};
use cashu_crab::Amount;
use gloo::storage::{LocalStorage, Storage};
use gloo_net::http::Request;
use log::{debug, warn};
use node_manager_types::requests::{self, OpenChannelRequest};
use node_manager_types::responses::ChannelInfo;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

use super::channel::Channel;

async fn post_open_channel(
    jwt: &str,
    open_channel_request: OpenChannelRequest,
    open_channel_callback: Callback<String>,
) -> Result<()> {
    let _fetched_channels: ChannelInfo = Request::post("http://127.0.0.1:8086/open-channel")
        .header("Authorization", &format!("Bearer {}", jwt))
        .json(&open_channel_request)?
        .send()
        .await?
        .json()
        .await?;

    open_channel_callback.emit("".to_string());

    Ok(())
}

pub enum State {
    Channels,
    OpenChannel,
}

#[derive(PartialEq, Properties)]
pub struct OpenChannelProps {
    jwt: String,
    back_callback: Callback<MouseEvent>,
}

pub enum OpenChannelMsg {
    Submit,
    ChannelOpened(String),
}

#[derive(Default)]
pub struct OpenChannel {
    input_node_ref: NodeRef,
    ip_input_node_ref: NodeRef,
    port_input_node_ref: NodeRef,
    amount_input_node_ref: NodeRef,
    push_amount_input_node_ref: NodeRef,
}

impl Component for OpenChannel {
    type Message = OpenChannelMsg;
    type Properties = OpenChannelProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            OpenChannelMsg::Submit => {
                let pubkey = self
                    .input_node_ref
                    .cast::<HtmlInputElement>()
                    .map(|i| PublicKey::from_str(&i.value()));

                let ip = self
                    .ip_input_node_ref
                    .cast::<HtmlInputElement>()
                    .map(|i| i.value());

                let port = self
                    .port_input_node_ref
                    .cast::<HtmlInputElement>()
                    .map(|i| i.value().parse::<u16>());

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

                if let (
                    Some(Ok(public_key)),
                    Some(ip),
                    Some(Ok(port)),
                    Some(Ok(amount)),
                    Some(Ok(push_amount)),
                ) = (pubkey, ip, port, amount, push_amount)
                {
                    let amount = Amount::from_sat(amount);
                    let push_amount = if push_amount > 0 {
                        Some(Amount::from_sat(push_amount))
                    } else {
                        None
                    };

                    let open_channel = OpenChannelRequest {
                        public_key,
                        ip,
                        port,
                        amount,
                        push_amount,
                    };

                    let callback = ctx.link().callback(OpenChannelMsg::ChannelOpened);
                    let jwt = ctx.props().jwt.clone();

                    spawn_local(async move {
                        post_open_channel(&jwt, open_channel, callback).await.ok();
                    });
                } else {
                    warn!("Sommethitng is missing");
                }

                false
            }
            OpenChannelMsg::ChannelOpened(response) => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_submit = ctx.link().callback(|_| OpenChannelMsg::Submit);

        html! {
                <>

            <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
            <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Open Channel" } </h5>
                  <div>
                      <div class="relative z-0 w-full mb-6 group">
                  <input name="peer_pubkey" id="peer_pubkey" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.input_node_ref.clone()} />
                  <label for="peer_pubkey" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Pubkey"}</label>
                  </div>
                    </div>
                      <div class="relative z-0 w-full mb-6 group">
                  <input name="peer_ip" id="peer_id" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.ip_input_node_ref.clone()} />
                  <label for="peer_ip" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Ip"}</label>

                  </div>
              <div class="relative z-0 w-full mb-6 group">
                  <input type="numeric" name="peer_port" id="peer_port" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.port_input_node_ref.clone()} />
                  <label for="peer_port" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Port"}</label>
            </div>
                      <div class="relative z-0 w-full mb-6 group">
                  <input type="numeric" name="channel_size" id="channel_size" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.amount_input_node_ref.clone()} />
                  <label for="channel_size" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Channel Size (sat)"}</label>
            </div>
              <div class="relative z-0 w-full mb-6 group">
                  <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.push_amount_input_node_ref.clone()} />
                  <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Push Amount"}</label>
            </div>
                <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={on_submit}>{"Create Invoice"}</button>
                    <button class="px-6 py-2 rounded-sm" onclick={ctx.props().back_callback.clone()}>{"Back"}</button>
        </a>
                </>
            }
    }
}

#[derive(Properties, PartialEq, Default, Clone)]
pub struct Props {
    pub jwt: String,
}

pub enum Msg {
    OpenChannelView,
    FetechedChannels(Vec<ChannelInfo>),
    Back,
}

#[derive(Default)]
enum View {
    #[default]
    Channels,
    OpenChannel,
}

#[derive(Default)]
pub struct Channels {
    view: View,
    channels: Vec<ChannelInfo>,
}

impl Component for Channels {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let channels_callback = ctx.link().callback(Msg::FetechedChannels);

        let jwt = ctx.props().jwt.clone();
        spawn_local(async move {
            get_channels(&jwt, channels_callback).await.ok();
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
        }
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        let open_channel_button = ctx.link().callback(|_| Msg::OpenChannelView);

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
                                                               </>
                                                               }
                                   }
                               View::OpenChannel => {
                        let back = ctx.link().callback(|_| Msg::Back);
                                   html!{ <OpenChannel  jwt={ctx.props().jwt.clone()} back_callback={back}/> }
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
