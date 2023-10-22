use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use gloo_net::http::Request;
use ln_rs_models::{requests, Amount, ChannelStatus};
use serde_json::Value;
use url::Url;
use yew::platform::spawn_local;
use yew::prelude::*;

async fn post_close_channel(
    url: &Url,
    jwt: &str,
    close_channel_request: requests::CloseChannel,
    channel_close_cb: Callback<String>,
) -> Result<()> {
    let url = url.join("close")?;

    let _: Value = Request::post(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .json(&close_channel_request)
        .unwrap()
        .send()
        .await?
        .json()
        .await?;

    channel_close_cb.emit("OK".to_string());
    Ok(())
}

#[derive(PartialEq, Properties, Clone)]
pub struct Props {
    pub url: Url,
    pub jwt: String,
    // pub onedit: Callback<(usize, String)>,
    pub channel_id: String,
    pub peer_id: Option<PublicKey>,
    pub local_balance: Amount,
    pub remote_balance: Amount,
    pub status: ChannelStatus,
}

pub enum Msg {
    Delete,
    ChannelClosed,
}

#[derive(Default)]
pub struct Channel {}

impl Component for Channel {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Delete => {
                let url = ctx.props().url.clone();
                let jwt = ctx.props().jwt.clone();
                let channel_close = requests::CloseChannel {
                    channel_id: ctx.props().channel_id.clone(),
                    peer_id: ctx.props().peer_id,
                };

                let callback = ctx.link().callback(|_| Msg::ChannelClosed);

                spawn_local(async move {
                    let _ = post_close_channel(&url, &jwt, channel_close, callback).await;
                });

                true
            }
            Msg::ChannelClosed => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let Props {
            url: _,
            jwt: _,
            channel_id: _,
            peer_id,
            local_balance,
            remote_balance,
            status,
        } = ctx.props().clone();

        let on_delete = ctx.link().callback(|_| Msg::Delete);

        html! {
            <a class="block p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
          <div class="flex flex-row mb-4">
            <div class="p-2 w-full">
            if let Some(peer) = peer_id {

                <p class="font-normal text-gray-700 dark:text-gray-400"> {
                    format!("Peer id: {}...{}", &peer.to_string()[..8], &peer.to_string()[56..]) } </p>
            }
                <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Local Balance: {}", local_balance.to_sat())}</p>
                <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Remote Balance: {}", remote_balance.to_sat())}</p>
                <p class="font-normal text-gray-700 dark:text-gray-400">{ format!("Status: {}", status.to_string() ) }</p>
            </div>
            <div class="p-2 w-full flex justify-end">
                <button type="button" class="focus:outline-none text-white bg-red-700 hover:bg-red-800 focus:ring-4 focus:ring-red-300 font-medium rounded-lg text-sm px-5 py-2.5 mr-2 mb-2 dark:bg-red-600 dark:hover:bg-red-700 dark:focus:ring-red-900" onclick={on_delete}> {"Close"} </button>
            </div>
            </div>
            </a>
        }
    }
}
