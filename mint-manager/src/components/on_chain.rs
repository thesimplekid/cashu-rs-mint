use cashu_crab::Amount;
use gloo::storage::{LocalStorage, Storage};
use gloo_net::http::Request;
use node_manager_types::{requests::PayOnChainRequest, responses::FundingAddressResponse};
use serde_json::Value;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Clone)]
enum State {
    Transactions,
    NewAddress,
    Send,
    Sent,
}

#[function_component(OnChain)]
pub fn on_chain() -> Html {
    let state = use_state(|| State::Transactions);
    let on_chain_address = use_state(|| None);

    let generate_address = {
        let on_chain_address = on_chain_address.clone();
        let state = state.clone();
        Callback::from(move |_| {
            if let Ok(jwt) = LocalStorage::get::<String>("auth_token") {
                let on_chain_address = on_chain_address.clone();
                let state_clone = state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let fetched_channels: FundingAddressResponse =
                        Request::get("http://127.0.0.1:8086/fund")
                            .header("Authorization", &format!("Bearer {}", jwt))
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();
                    on_chain_address.set(Some(fetched_channels.address));
                    state_clone.set(State::NewAddress);
                });
            }
        })
    };

    let amount_node_ref = use_node_ref();
    let amount_value_handle = use_state(|| Amount::ZERO);
    let amount_value = (*amount_value_handle).clone();

    let amount_onchange = {
        let input_node_ref = amount_node_ref.clone();
        let amount_value_handle = amount_value_handle.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value().parse::<u64>().unwrap();
                let amount = Amount::from_sat(input);

                amount_value_handle.set(amount);
            }
        })
    };

    let address_node_ref = use_node_ref();
    let address_value_handle = use_state(|| String::new());
    let address_value = (*address_value_handle).clone();

    let address_onchange = {
        let input_node_ref = address_node_ref.clone();
        let description_value_handle = address_value_handle.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value();

                description_value_handle.set(input);
            }
        })
    };

    let state_clone = state.clone();
    let send = {
        Callback::from(move |_| {
            state_clone.set(State::Send);
        })
    };

    let txid = use_state(|| String::new());

    let pay = {
        let amount_value = amount_value.clone();
        let address_value = address_value.clone();

        let state_clone = state.clone();
        let txid = txid.clone();
        Callback::from(move |_| {
            let pay_request = PayOnChainRequest {
                sat: amount_value.to_sat(),
                address: address_value.clone(),
            };
            let state_clone = state_clone.clone();

            let txid = txid.clone();

            if let Ok(jwt) = LocalStorage::get::<String>("auth_token") {
                wasm_bindgen_futures::spawn_local(async move {
                    let response: String =
                        Request::post(&format!("http://127.0.0.1:8086/pay-on-chain"))
                            .header("Authorization", &format!("Bearer {}", jwt))
                            .json(&pay_request)
                            .unwrap()
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();
                    txid.set(response);
                    state_clone.set(State::Sent);
                });
            }
        })
    };
    let close = {
        let on_chain_address = on_chain_address.clone();
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let on_chain_address = on_chain_address.clone();
            on_chain_address.set(None);
            state.set(State::Transactions);

            amount_value_handle.set(Amount::ZERO);
            address_value_handle.set(String::new());
        })
    };

    let state_clone = state.clone();
    html! {
            <>

        <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "On Chain" }</h5>
    {
        match *state_clone {
                State::Transactions => {
                    html!{
                        <>
            <button onclick={generate_address} class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                 { "Generate Address" }
             </button>


            <button onclick={send} class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                 { "Send" }
             </button>

                        </>
                    }
                }
                State::Send => {
                    html!{
                        <>
          <div class="relative z-0 w-full mb-6 group">
                                  <div class="relative z-0 w-full mb-6 group">
              <label for="description" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Address"}</label>
              <input type="text" name="description" id="description" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={address_node_ref} value={address_value} onchange={address_onchange} />
                    </div>
                <div class="relative z-0 w-full mb-6 group">
            <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Amount (sat)"}</label>
              <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={amount_node_ref} value={amount_value.to_sat().to_string()} onchange={amount_onchange} />
                        </div>
        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
            <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={pay}>{"Send"}</button>
                <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
        </div>
        </div>

                        </>
                    }
                }
                State::NewAddress => {
                    html!{
                        <>
            <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                {"Generated Address"}
            </h2>
                        if on_chain_address.is_some() {
            <p class="flex-1 dark:text-gray-400">{on_chain_address.as_ref().unwrap() }</p>
                        }
            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
                // TODO: Copy button
                // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
        </div>

                        </>
                    }
                }
                State::Sent => {
                    html!{
                    <>
            <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                {"Txid"}
            </h2>
            <p class="flex-1 dark:text-gray-400">{ txid.to_string() }</p>
            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                <button class="px-6 py-2 rounded-sm" onclick={close.clone()}>{"Back"}</button>
                // TODO: Copy button
                // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
        </div>

                        </>}

                }
            }
        }

    </a>
            </>
            }
}
