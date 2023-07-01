use std::str::FromStr;

use bitcoin::secp256k1::PublicKey;
use cashu_crab::{Amount, Invoice};
use gloo_net::http::Request;
use node_manager_types::requests;
use node_manager_types::{
    requests::OpenChannelRequest,
    responses::{BalanceResponse, ChannelInfo, FundingAddressResponse},
    Bolt11,
};
use serde_json::Value;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::channel::Channel;

#[function_component(Balance)]
pub fn balance() -> HtmlResult {
    let balance = use_state(|| BalanceResponse::default());
    {
        let balance = balance.clone();
        use_effect_with_deps(
            move |_| {
                let balance = balance.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let fetched_balance: BalanceResponse =
                        Request::get("http://127.0.0.1:8086/balance")
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();
                    balance.set(fetched_balance);
                });
                || ()
            },
            (),
        );
    }
    let total_balance = balance.on_chain_total + balance.ln;
    let pending = balance.on_chain_total - balance.on_chain_spendable;

    Ok(html! {
    <>

    <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { format!("Total Balance: {} sats", total_balance.to_sat()) } </h5>
        <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Lighting: {}", balance.ln.to_sat())}</p>
        <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Onchain Spendable: {}", balance.on_chain_spendable.to_sat())}</p>
        <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Onchain Pending: {}", pending.to_sat())}</p>
    </a>

    </>
    })
}

#[function_component(Channels)]
pub fn channels() -> Html {
    let channels = use_state(|| vec![]);
    {
        let channels = channels.clone();
        use_effect_with_deps(
            move |_| {
                let channels = channels.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let fetched_channels: Vec<ChannelInfo> =
                        Request::get("http://127.0.0.1:8086/channels")
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();
                    channels.set(fetched_channels);
                });
                || ()
            },
            (),
        );
    }

    let delete_channel = {
        let channels = channels.clone();
        Callback::from(move |(pubkey, channel_id)| {
            log::debug!("{:?}", pubkey);
            log::debug!("{:?}", channel_id);

            let close_channel_request = requests::CloseChannel {
                channel_id,
                peer_id: pubkey,
            };

            post_close_channel(close_channel_request);
            let channels = channels.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetched_channels: Vec<ChannelInfo> =
                    Request::get("http://127.0.0.1:8086/channels")
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();
                channels.set(fetched_channels);
            });
        })
    };

    html! {
        <>
    <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
            <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "Channels" }</h5>
    {

             channels.iter().map(|channel| {

                let remote_balance = channel.value - channel.balance;

                html!{
                    <Channel delete_channel={delete_channel.clone()} channel_id={channel.channel_id.clone()} peer_id= {channel.peer_pubkey} local_balance={channel.balance} {remote_balance}/>
            }}).collect::<Html>()
    }
        </a>
        </>
        }
}

fn post_close_channel(close_channel_request: requests::CloseChannel) {
    wasm_bindgen_futures::spawn_local(async move {
        let _fetched_channels: Value = Request::post("http://127.0.0.1:8086/close")
            .json(&close_channel_request)
            .unwrap()
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        log::debug!("{:?}", _fetched_channels);
    });
}

fn post_pay_invoice(pay_invoice_request: Bolt11) {
    wasm_bindgen_futures::spawn_local(async move {
        let _fetched_channels: Value = Request::post("http://127.0.0.1:8086/pay-invoice")
            .json(&pay_invoice_request)
            .unwrap()
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        log::debug!("{:?}", _fetched_channels);
    });
}

#[function_component(Ln)]
pub fn ln() -> Html {
    let pay_invoice = use_state(|| false);

    let amount_node_ref = use_node_ref();
    let amount_value_handle = use_state(|| Amount::ZERO);
    let amount_value = (*amount_value_handle).clone();

    let amount_onchange = {
        let input_node_ref = amount_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value().parse::<u64>().unwrap();
                let amount = Amount::from_sat(input);

                amount_value_handle.set(amount);
            }
        })
    };

    let invoice = use_state(|| None);

    let generate_invoice = {
        let invoice = invoice.clone();
        Callback::from(move |_| {
            let invoice = invoice.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetched_channels: Bolt11 = Request::get(&format!(
                    "http://127.0.0.1:8086/invoice?msat={}",
                    amount_value.to_msat()
                ))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
                invoice.set(Some(fetched_channels.bolt11));
            });
        })
    };

    let pay_button = {
        let pay_invoice = pay_invoice.clone();

        Callback::from(move |_| pay_invoice.set(true))
    };

    let pay_input_node_ref = use_node_ref();
    let pay_value_handle = use_state(String::default);
    let pay_value = (*pay_value_handle).clone();

    let pay_onchange = {
        let input_node_ref = pay_input_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                pay_value_handle.set(input.value());
            }
        })
    };

    let send_pay = {
        let pay_value = pay_value.clone();

        log::debug!("{}", pay_value);

        Callback::from(move |_e: MouseEvent| {
            let open_request = Bolt11 {
                bolt11: Invoice::from_str(&pay_value).unwrap(),
            };

            post_pay_invoice(open_request);
        })
    };

    let close = {
        let invoice = invoice.clone();
        let pay_invoice = pay_invoice.clone();
        Callback::from(move |_| {
            let invoice = invoice.clone();
            invoice.set(None);
            pay_invoice.set(false);
        })
    };

    html! {
    <div>
    <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "lightning" } </h5>
            if invoice.is_some() {
                <div class="flex flex-col max-w-md gap-2 p-6 rounded-md shadow-md dark:bg-gray-900 dark:text-gray-100">
        <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
            {"Invoice"}
        </h2>
        <p class="flex-1 dark:text-gray-400">{invoice.as_ref().unwrap() }</p>
        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
            <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Close"}</button>
            // TODO: Copy button
            // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
        </div>
    </div>
            }

        if *pay_invoice {
                <div class="flex flex-col max-w-md gap-2 p-6 rounded-md shadow-md dark:bg-gray-900 dark:text-gray-100">
        <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
            {"Pay Invoice"}
        </h2>
          <input name="peer_ip" id="peer_id" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={pay_input_node_ref} value={pay_value} onchange={pay_onchange} />
          <label for="peer_ip" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Ip"}</label>
        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
            <button class="px-6 py-2 rounded-sm" onclick={close}>{"Close"}</button>
            <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={send_pay}>{"Pay"}</button>
        </div>
    </div>

        }
        if invoice.is_none() {
          <div class="relative z-0 w-full mb-6 group">
              <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={amount_node_ref} value={amount_value.to_sat().to_string()} onchange={amount_onchange} />
              <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Amount (sat)"}</label>
        </div>
        }
            <button onclick={generate_invoice} class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm w-full sm:w-auto px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Generate Invoice"}</button>
            <button onclick={pay_button} class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm w-full sm:w-auto px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Pay Invoice"}</button>
    </a>
        </div>
        }
}

#[function_component(OnChain)]
pub fn on_chain() -> Html {
    let on_chain_address = use_state(|| None);

    let generate_address = {
        let on_chain_address = on_chain_address.clone();
        Callback::from(move |_| {
            let on_chain_address = on_chain_address.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetched_channels: FundingAddressResponse =
                    Request::get("http://127.0.0.1:8086/fund")
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();
                on_chain_address.set(Some(fetched_channels.address));
            });
        })
    };

    let close = {
        let on_chain_address = on_chain_address.clone();
        Callback::from(move |_| {
            let on_chain_address = on_chain_address.clone();
            on_chain_address.set(None);
        })
    };

    log::debug!("{:?}", on_chain_address);

    html! {
            <>

        <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                if on_chain_address.is_some() {
                    <div class="flex flex-col max-w-md gap-2 p-6 rounded-md shadow-md dark:bg-gray-900 dark:text-gray-100">
            <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                {"Generated Address"}
            </h2>
            <p class="flex-1 dark:text-gray-400">{on_chain_address.as_ref().unwrap() }</p>
            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                <button class="px-6 py-2 rounded-sm" onclick={close}>{"Close"}</button>
                // TODO: Copy button
                // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
            </div>
        </div>
                }
                <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "On Chain" }</h5>


            <button onclick={generate_address} class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm w-full sm:w-auto px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">
                 { "Generate Address" }
             </button>
    </a>
            </>
            }
}

fn post_open_channel(open_channel_request: OpenChannelRequest) {
    wasm_bindgen_futures::spawn_local(async move {
        let _fetched_channels: ChannelInfo = Request::post("http://127.0.0.1:8086/open-channel")
            .json(&open_channel_request)
            .unwrap()
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
    });
}

#[function_component(OpenChannel)]
pub fn open_channel() -> Html {
    let input_node_ref = use_node_ref();

    let pubkey_value_handle = use_state(String::default);
    let pubkey_value = (*pubkey_value_handle).clone();

    let pubkey_onchange = {
        let input_node_ref = input_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                pubkey_value_handle.set(input.value());
            }
        })
    };

    let ip_input_node_ref = use_node_ref();
    let ip_value_handle = use_state(String::default);
    let ip_value = (*ip_value_handle).clone();

    let ip_onchange = {
        let input_node_ref = ip_input_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                ip_value_handle.set(input.value());
            }
        })
    };

    let port_input_node_ref = use_node_ref();
    let port_value_handle = use_state(|| 0);
    let port_value = (*port_value_handle).clone();

    let port_onchange = {
        let input_node_ref = port_input_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value().parse::<u16>().unwrap();
                port_value_handle.set(input);
            }
        })
    };

    let amount_input_node_ref = use_node_ref();
    let amount_value_handle = use_state(|| Amount::from_sat(0));
    let amount_value = (*amount_value_handle).clone();

    let amount_onchange = {
        let input_node_ref = amount_input_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value().parse::<u64>().unwrap();
                let amount = Amount::from_sat(input);

                amount_value_handle.set(amount);
            }
        })
    };

    let push_amount_input_node_ref = use_node_ref();
    let push_amount_value_handle = use_state(|| Amount::from_sat(0));
    let push_amount_value = (*push_amount_value_handle).clone();

    let push_amount_onchange = {
        let input_node_ref = push_amount_input_node_ref.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value().parse::<u64>().unwrap();
                let amount = Amount::from_sat(input);

                push_amount_value_handle.set(amount);
            }
        })
    };

    let on_submit = {
        let pubkey = pubkey_value.clone();
        let port_value = port_value;
        let ip = ip_value.clone();
        let push_amount = Some(push_amount_value);
        let port = port_value;
        let amount = amount_value;

        Callback::from(move |_e: MouseEvent| {
            let public_key = PublicKey::from_str(&pubkey).unwrap();
            let open_request = OpenChannelRequest {
                public_key,
                port,
                ip: ip.clone(),
                push_amount,
                amount,
            };

            post_open_channel(open_request);
        })
    };

    html! {
             <>
        <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Open Channel" } </h5>
              <div>
                  <div class="relative z-0 w-full mb-6 group">
              <input name="peer_pubkey" id="peer_pubkey" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={input_node_ref} value={pubkey_value} onchange={pubkey_onchange} />
              <label for="peer_pubkey" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Pubkey"}</label>
              </div>
                </div>
                  <div class="relative z-0 w-full mb-6 group">
              <input name="peer_ip" id="peer_id" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={ip_input_node_ref} value={ip_value} onchange={ip_onchange} />
              <label for="peer_ip" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Ip"}</label>

              </div>
          <div class="relative z-0 w-full mb-6 group">
              <input type="numeric" name="peer_port" id="peer_port" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={port_input_node_ref} value={port_value.to_string()} onchange={port_onchange} />
              <label for="peer_port" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Port"}</label>
        </div>
                  <div class="relative z-0 w-full mb-6 group">
              <input type="numeric" name="channel_size" id="channel_size" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={amount_input_node_ref} value={amount_value.to_sat().to_string()} onchange={amount_onchange} />
              <label for="channel_size" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Channel Size (sat)"}</label>
        </div>
          <div class="relative z-0 w-full mb-6 group">
              <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={push_amount_input_node_ref} value={push_amount_value.to_sat().to_string()} onchange={push_amount_onchange} />
              <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Push Amount"}</label>
        </div>
            <button onclick={on_submit} class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm w-full sm:w-auto px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">
                 { "Open Channel" }
             </button>
    </a>
             </>
             }
}

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <main>
        <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
            <div>
                <Balance/>
            </div>
            <div>
                <Channels/>
            </div>
            <div>
                <OpenChannel/>
            </div>
            <div>
                <OnChain/>
            </div>
            <div>
                <Ln/>
            </div>
        </div>

        </main>
    }
}
