use std::str::FromStr;

use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use gloo_net::http::Request;
use log::warn;
use node_manager_types::requests::ConnectPeerRequest;
use node_manager_types::responses::ChannelInfo;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

async fn post_connect_peer(
    jwt: &str,
    connect_peer_request: ConnectPeerRequest,
    connect_peer_callback: Callback<String>,
) -> Result<()> {
    let _fetched_channels: ChannelInfo = Request::post("http://127.0.0.1:8086/connect-peer")
        .header("Authorization", &format!("Bearer {}", jwt))
        .json(&connect_peer_request)?
        .send()
        .await?
        .json()
        .await?;

    connect_peer_callback.emit("".to_string());

    Ok(())
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub jwt: String,
    pub back_callback: Callback<MouseEvent>,
}

pub enum Msg {
    Submit,
    PeerConnected,
}

#[derive(Default)]
pub struct ConnectPeer {
    input_node_ref: NodeRef,
    ip_input_node_ref: NodeRef,
    port_input_node_ref: NodeRef,
}

impl Component for ConnectPeer {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Submit => {
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

                if let (Some(Ok(public_key)), Some(ip), Some(Ok(port))) = (pubkey, ip, port) {
                    let connect_request = ConnectPeerRequest {
                        public_key,
                        ip,
                        port,
                    };

                    let callback = ctx.link().callback(|_| Msg::PeerConnected);
                    let jwt = ctx.props().jwt.clone();

                    spawn_local(async move {
                        post_connect_peer(&jwt, connect_request, callback)
                            .await
                            .ok();
                    });
                } else {
                    warn!("Sommethitng is missing");
                }

                true
            }
            Msg::PeerConnected => true,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_submit = ctx.link().callback(|_| Msg::Submit);
        html! {
                <>

            <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
            <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Connect Peer" } </h5>
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
                <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={on_submit}>{"Connect Peer"}</button>
                    <button class="px-6 py-2 rounded-sm" onclick={ctx.props().back_callback.clone()}>{"Back"}</button>
        </a>
                </>

        }
    }
}
