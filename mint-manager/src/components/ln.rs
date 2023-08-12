use std::str::FromStr;

use anyhow::Result;
use cashu_crab::types::InvoiceStatus;
use cashu_crab::{Amount, Bolt11Invoice};
use gloo_net::http::Request;
use ln_rs::node_manager_types::Bolt11;
use serde_json::Value;
use url::Url;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

async fn post_pay_invoice(
    jwt: &str,
    url: &Url,
    pay_invoice_request: Bolt11,
    callback: Callback<InvoiceStatus>,
) -> Result<()> {
    let url = url.join("pay-invoice")?;

    let _response: Value = Request::post(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .json(&pay_invoice_request)?
        .send()
        .await?
        .json()
        .await?;

    // TODO: Need to handle responses and not always return paid
    callback.emit(InvoiceStatus::Paid);

    Ok(())
}

async fn get_invoice(
    jwt: &str,
    url: &Url,
    amount: Amount,
    description: &str,
    callback: Callback<Bolt11Invoice>,
) -> Result<()> {
    let mut url = url.join("invoice")?;
    url.set_query(Some(&format!(
        "msat={}&description={}",
        amount.to_msat(),
        description
    )));

    let invoice: Bolt11 = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    callback.emit(invoice.bolt11);

    Ok(())
}

#[derive(Default)]
enum View {
    #[default]
    Transactions,
    Send(Option<(Amount, String)>),
    Receive,
    NewInvoice(Bolt11Invoice),
}

#[derive(Properties, PartialEq, Clone)]
pub struct Props {
    pub url: Url,
    pub jwt: String,
}

pub enum Msg {
    Close,
    ReceiveView,
    PayView,
    GenerateInvoice,
    SendPay,
    InvoiceChange,
    NewInvoice(Bolt11Invoice),
    Paid(InvoiceStatus),
}

#[derive(Default)]
pub struct Ln {
    view: View,
    amount_node_ref: NodeRef,
    description_node_ref: NodeRef,
    pay_input_node_ref: NodeRef,
}

impl Component for Ln {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Close => {
                self.view = View::Transactions;
                true
            }
            Msg::ReceiveView => {
                self.view = View::Receive;
                true
            }
            Msg::PayView => {
                self.view = View::Send(None);
                true
            }
            Msg::GenerateInvoice => {
                let callback = ctx.link().callback(Msg::NewInvoice);
                let jwt = ctx.props().jwt.clone();
                let url = ctx.props().url.clone();
                let mut amount_value = None;
                let mut description = None;

                let amount = self.amount_node_ref.cast::<HtmlInputElement>();

                if let Some(input) = amount {
                    let input = input.value().parse::<u64>().unwrap();
                    amount_value = Some(Amount::from_sat(input));
                }

                let input = self.description_node_ref.cast::<HtmlInputElement>();

                if let Some(input) = input {
                    description = Some(input.value());
                }

                if let (Some(amount), Some(description)) = (amount_value, description) {
                    spawn_local(async move {
                        get_invoice(&jwt, &url, amount, &description, callback)
                            .await
                            .ok();
                    });
                }
                false
            }
            Msg::SendPay => {
                let input = self.pay_input_node_ref.cast::<HtmlInputElement>();
                let jwt = ctx.props().jwt.clone();
                let url = ctx.props().url.clone();

                let callback = ctx.link().callback(Msg::Paid);

                if let Some(input) = input {
                    let bolt11 = Bolt11 {
                        bolt11: Bolt11Invoice::from_str(&input.value()).unwrap(),
                    };
                    spawn_local(async move {
                        post_pay_invoice(&jwt, &url, bolt11, callback).await.ok();
                    });
                }

                false
            }
            Msg::InvoiceChange => {
                let input = self.pay_input_node_ref.cast::<HtmlInputElement>();
                if let Some(input) = input {
                    if let Ok(invoice) = Bolt11Invoice::from_str(&input.value()) {
                        let description = match invoice.description() {
                            cashu_crab::lightning_invoice::Bolt11InvoiceDescription::Direct(
                                des,
                            ) => des.to_string(),
                            cashu_crab::lightning_invoice::Bolt11InvoiceDescription::Hash(_des) => {
                                "".to_string()
                            }
                        };
                        self.view = View::Send(Some((
                            Amount::from_msat(invoice.amount_milli_satoshis().unwrap_or(0)),
                            description,
                        )));

                        return true;
                    }
                }

                false
            }
            Msg::NewInvoice(invoice) => {
                self.view = View::NewInvoice(invoice);
                true
            }
            Msg::Paid(_status) => {
                self.view = View::Transactions;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let close = ctx.link().callback(|_| Msg::Close);
        let receive = ctx.link().callback(|_| Msg::ReceiveView);
        let pay_button = ctx.link().callback(|_| Msg::PayView);
        let generate_invoice = ctx.link().callback(|_| Msg::GenerateInvoice);
        let send_pay = ctx.link().callback(|_| Msg::SendPay);

        let invoice_change = ctx.link().callback(|_| Msg::InvoiceChange);
        html! {
                    <>

                <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "Lightning" }</h5>
            {
                match &self.view {
                    View::Transactions => {
                html!{
                    <>
                    <div class="flex space-x-2">
                        <button onclick={receive} class="flex-1 p-8 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Receive"}</button>
                        <button onclick={pay_button} class="flex-1 p-8 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Send"}</button>
                    </div>
                    </>
                }
                }
                    View::Receive => {
                        html! {
                            <div class="relative z-0 w-full mb-6 group">
                                <div class="relative z-0 w-full mb-6 group">
                                  <label for="description" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Description"}</label>
                                  <input type="text" name="description" id="description" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.description_node_ref.clone()} />
                                </div>
                                <div class="relative z-0 w-full mb-6 group">
                                    <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Amount (sat)"}</label>
                                    <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.amount_node_ref.clone()} />
                                </div>
                                <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                                    <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={generate_invoice}>{"Create Invoice"}</button>
                                    <button class="px-6 py-2 rounded-sm" onclick={close}>{"Back"}</button>
                                </div>
                            </div>
                        }
                    }
                    View::NewInvoice(invoice) => {
                        html! {
                            <>
                            <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                            {"Generated Invoice"}
                            </h2>
                            <p class="flex-1 dark:text-gray-400" style="max-width: 33vw; word-wrap: break-word;">{invoice.to_string() }</p>
                            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                                <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
                            // TODO: Copy button
                            // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
                            </div>

                            </>
                        }
                    }
                    View::Send(info) => {
                        html!{
                            <>
                            <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                                {"Pay Invoice"}
                            </h2>
                            <input name="pay_invoice" id="pay_invoice" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.pay_input_node_ref.clone()} onchange={invoice_change} />

                            if let Some((amount, description)) = info {
                                <div class="flex">
                                        <p class="font-normal text-gray-700 dark:text-gray-400"> { format!("Description: {}", description ) } </p>
                                        <p class="font-normal text-gray-700 dark:text-gray-400" style="margin-left: auto;"> { format!("Amount (sat): {}", amount.to_sat() ) } </p>
                                </div>
                            }

                            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                                <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={send_pay}>{"Pay"}</button>
                                <button class="px-6 py-2 rounded-sm" onclick={close}>{"Back"}</button>
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
