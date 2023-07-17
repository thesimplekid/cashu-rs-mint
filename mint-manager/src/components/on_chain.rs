use anyhow::Result;
use cashu_crab::Amount;
use gloo_net::http::Request;
use node_manager_types::{requests::PayOnChainRequest, responses::FundingAddressResponse};
use url::Url;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

async fn get_new_addr(jwt: &str, url: &Url, new_addr_callback: Callback<String>) -> Result<()> {
    let url = url.join("fund")?;
    let fetched_channels: FundingAddressResponse = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    new_addr_callback.emit(fetched_channels.address);

    Ok(())
}

async fn pay_on_chain(
    jwt: &str,
    url: &Url,
    pay_request: PayOnChainRequest,
    pay_on_chain_callback: Callback<String>,
) -> Result<()> {
    let url = url.join("pay-on-chain")?;
    let response: String = Request::post(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .json(&pay_request)?
        .send()
        .await?
        .json()
        .await?;

    pay_on_chain_callback.emit(response);

    Ok(())
}

#[derive(Properties, PartialEq, Clone)]
pub struct Props {
    pub jwt: String,
    pub url: Url,
}

pub enum Msg {
    FetechNewAddr,
    NewAddr(String),
    Close,
    SendView,
    Pay,
    Paid(String),
}

#[derive(Default)]
enum View {
    #[default]
    Transactions,
    NewAddress(String),
    Send,
    Sent(String),
}

#[derive(Default)]
pub struct OnChain {
    view: View,
    amount_node_ref: NodeRef,
    address_node_ref: NodeRef,
}

impl Component for OnChain {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetechNewAddr => {
                let callback = ctx.link().callback(Msg::NewAddr);
                let jwt = ctx.props().jwt.clone();

                let url = ctx.props().url.clone();
                spawn_local(async move {
                    get_new_addr(&jwt, &url, callback).await.ok();
                });
                false
            }
            Msg::NewAddr(addr) => {
                self.view = View::NewAddress(addr);
                true
            }
            Msg::SendView => {
                self.view = View::Send;
                true
            }
            Msg::Close => {
                self.view = View::Transactions;
                true
            }
            Msg::Pay => {
                let callback = ctx.link().callback(Msg::Paid);
                let jwt = ctx.props().jwt.clone();
                let url = ctx.props().url.clone();

                let mut amount_value = None;
                let mut address = None;

                let amount = self.amount_node_ref.cast::<HtmlInputElement>();

                if let Some(input) = amount {
                    let input = input.value().parse::<u64>().unwrap();
                    amount_value = Some(Amount::from_sat(input));
                }

                let input = self.address_node_ref.cast::<HtmlInputElement>();

                if let Some(input) = input {
                    address = Some(input.value());
                }

                if let (Some(amount), Some(address)) = (amount_value, address) {
                    let pay_request = PayOnChainRequest {
                        sat: amount.to_sat(),
                        address,
                    };

                    spawn_local(async move {
                        pay_on_chain(&jwt, &url, pay_request, callback).await.ok();
                    });
                }

                false
            }
            Msg::Paid(txid) => {
                self.view = View::Sent(txid);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let generate_address = ctx.link().callback(|_| Msg::FetechNewAddr);
        let close = ctx.link().callback(|_| Msg::Close);
        let send = ctx.link().callback(|_| Msg::SendView);
        let pay = ctx.link().callback(|_| Msg::Pay);
        html! {
                <>

            <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                    <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "On Chain" }</h5>
        {
            match &self.view {
                View::Transactions => {
                    html! {
                        <>
                        <div class="flex space-x-2">
                            <button onclick={generate_address} class="flex-1 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                                 { "Generate Address" }
                            </button>
                            <button onclick={send} class="flex-1 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                                 { "Send" }
                             </button>
                        </div>

                        </>
                    }
                }
                View::NewAddress(address) => {
                    html! {
                     <>
                        <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                            {"Generated Address"}
                        </h2>
                        <p class="flex-1 dark:text-gray-400">{address }</p>
                        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                            <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
                            // TODO: Copy button
                            // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
                    </div>

                    </>
                                }
                }
                View::Send => {
                    html! {
                            <>
                            <div class="relative z-0 w-full mb-6 group">
                                <label for="description" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Address"}</label>
                                <input type="text" name="description" id="description" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.address_node_ref.clone()} />
                            </div>
                            <div class="relative z-0 w-full mb-6 group">
                                <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Amount (sat)"}</label>
                                <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.amount_node_ref.clone()} />
                            </div>
                            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                                <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={pay}>{"Send"}</button>
                                <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
                            </div>
                            </>

                    }
                }
                View::Sent(txid) => {
                    html! {
                        <>
                        <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                            {"Txid"}
                        </h2>
                        <p class="flex-1 dark:text-gray-400">{ txid.to_string() }</p>
                        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                            <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
                            // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
                        </div>
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
