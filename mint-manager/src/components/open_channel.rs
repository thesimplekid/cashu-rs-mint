use std::str::FromStr;

use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use cashu_crab::Amount;
use gloo_net::http::Request;
use log::warn;
use node_manager_types::requests::OpenChannelRequest;
use node_manager_types::responses::{self, ChannelInfo};
use web_sys::{EventTarget, HtmlInputElement};
use yew::platform::spawn_local;
use yew::prelude::*;

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

#[derive(PartialEq, Properties)]
pub struct Props {
    pub jwt: String,
    pub peers: Vec<responses::PeerInfo>,
    pub back_callback: Callback<MouseEvent>,
}

pub enum Msg {
    Submit,
    ChannelOpened(String),
    Selected(String),
}

#[derive(Default)]
pub struct OpenChannel {
    input_node_ref: NodeRef,
    ip_input_node_ref: NodeRef,
    port_input_node_ref: NodeRef,
    amount_input_node_ref: NodeRef,
    push_amount_input_node_ref: NodeRef,
    select_node_ref: NodeRef,
    selected: Option<PublicKey>,
}

impl Component for OpenChannel {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Submit => {
                log::debug!("{:?}", self.select_node_ref.cast::<HtmlInputElement>());
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

                    let callback = ctx.link().callback(Msg::ChannelOpened);
                    let jwt = ctx.props().jwt.clone();

                    spawn_local(async move {
                        post_open_channel(&jwt, open_channel, callback).await.ok();
                    });
                } else {
                    warn!("Sommethitng is missing");
                }

                false
            }
            Msg::ChannelOpened(_response) => false,
            Msg::Selected(sel) => {
                log::debug! {"sel: {:?}", sel};
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_submit = ctx.link().callback(|_| Msg::Submit);

        let onchange = ctx.link().callback(|e: Event| {
            log::debug!("{:?}", e);

            log::debug!("{:?}", e.current_target().unwrap());
            // Events can bubble so this listener might catch events from child
            // elements which are not of type HtmlInputElement

            Msg::Submit
        });

        let t = vec!["a", "b", "c"];
        let onselect = ctx.link().callback(|_| Msg::Selected("t".to_string()));
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

            <select
                ref={self.select_node_ref.clone()}
                {onchange}
            >
                <option value="" disabled=true selected={self.selected.is_none()}>
                    { "Select Peer" }
                </option>
                { for t.iter().map(|p| {html!{
                <option value="" onselect={onselect.clone()}>
                    { p }
                </option>
            }

            }
            ) }
            </select>
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
