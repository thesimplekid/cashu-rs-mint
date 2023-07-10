use std::str::FromStr;

use anyhow::Result;
use cashu_crab::types::InvoiceStatus;
use cashu_crab::{Amount, Invoice};
use gloo_net::http::Request;
use node_manager_types::Bolt11;
use serde_json::Value;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

async fn post_pay_invoice(
    jwt: &str,
    pay_invoice_request: Bolt11,
    callback: Callback<InvoiceStatus>,
) -> Result<()> {
    let _response: Value = Request::post("http://127.0.0.1:8086/pay-invoice")
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
    amount: Amount,
    description: &str,
    callback: Callback<Invoice>,
) -> Result<()> {
    let invoice: Bolt11 = Request::get(&format!(
        "http://127.0.0.1:8086/invoice?msat={}&description={}",
        amount.to_msat(),
        description
    ))
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
    NewInvoice(Invoice),
}

#[derive(Properties, PartialEq, Default, Clone)]
pub struct Props {
    pub jwt: String,
}

pub enum Msg {
    Close,
    ReceiveView,
    PayView,
    GenerateInvoice,
    SendPay,
    NewInvoice(Invoice),
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
                        get_invoice(&jwt, amount, &description, callback).await.ok();
                    });
                }
                false
            }
            Msg::SendPay => {
                let input = self.pay_input_node_ref.cast::<HtmlInputElement>();
                let jwt = ctx.props().jwt.clone();

                let callback = ctx.link().callback(Msg::Paid);

                if let Some(input) = input {
                    let bolt11 = Bolt11 {
                        bolt11: Invoice::from_str(&input.value()).unwrap(),
                    };
                    spawn_local(async move {
                        post_pay_invoice(&jwt, bolt11, callback).await.ok();
                    });
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
        html! {
                    <>

                <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "Lighting" }</h5>
            {
                match &self.view {
                        View::Transactions => {
                html!{
                <>
                <button onclick={receive} class="p-8 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Receive"}</button>
                <button onclick={pay_button} class="p-8 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Send"}</button>
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
                        <p class="flex-1 dark:text-gray-400">{invoice.to_string() }</p>
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
          <input name="pay_invoice" id="pay_invoice" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.pay_input_node_ref.clone()} />
          <label for="pay_invoice" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Ip"}</label>

                        if let Some((amount, description)) = info {
                                        <p class="font-normal text-gray-700 dark:text-gray-400"> { format!("Description: {}", description ) } </p>


                                        <p class="font-normal text-gray-700 dark:text-gray-400"> { format!("Description: {}", amount.to_sat() ) } </p>
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
