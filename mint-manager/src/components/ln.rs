use std::str::FromStr;

use cashu_crab::{lightning_invoice::InvoiceDescription, Amount, Invoice};
use gloo_net::http::Request;
use node_manager_types::Bolt11;
use serde_json::Value;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Copy, Clone)]
enum State {
    Transactions,
    Send,
    Receive,
}

#[function_component(Ln)]
pub fn ln() -> Html {
    let state = use_state(|| State::Transactions);

    let pay_invoice = use_state(|| false);

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

    let description_node_ref = use_node_ref();
    let description_value_handle = use_state(|| String::new());
    let description_value = (*description_value_handle).clone();

    let description_onchange = {
        let input_node_ref = description_node_ref.clone();
        let description_value_handle = description_value_handle.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let input = input.value();

                description_value_handle.set(input);
            }
        })
    };

    let invoice = use_state(|| None);

    let generate_invoice = {
        let invoice = invoice.clone();
        let description = description_value.clone();
        Callback::from(move |_: MouseEvent| {
            let invoice = invoice.clone();
            let description = description.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetched_channels: Bolt11 = Request::get(&format!(
                    "http://127.0.0.1:8086/invoice?msat={}&description={}",
                    amount_value.to_msat(),
                    description
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

    let decoded_invoice_amount = use_state(|| None);
    let decoded_invoice_desc = use_state(|| None);

    let state_clone = state.clone();
    let receive = { Callback::from(move |_| state_clone.set(State::Receive)) };

    let state_clone = state.clone();
    let pay_button = { Callback::from(move |_| state_clone.set(State::Send)) };

    let pay_input_node_ref = use_node_ref();
    let pay_value_handle = use_state(String::default);
    let pay_value = (*pay_value_handle).clone();

    let pay_onchange = {
        let input_node_ref = pay_input_node_ref.clone();

        let decoded_invoice_desc = decoded_invoice_desc.clone();
        let decoded_invoice_amount = decoded_invoice_amount.clone();
        let pay_value_handle = pay_value_handle.clone();

        Callback::from(move |_| {
            let input = input_node_ref.cast::<HtmlInputElement>();

            if let Some(input) = input {
                let value = input.value();
                if let Ok(invoice) = Invoice::from_str(&value) {
                    let description = match invoice.description() {
                        InvoiceDescription::Direct(desc) => desc.clone().into_inner(),
                        InvoiceDescription::Hash(hash) => hash.0.to_string(),
                    };

                    decoded_invoice_desc.set(Some(description));

                    if let Some(amount) = invoice.amount_milli_satoshis() {
                        let amount = Amount::from_msat(amount);

                        decoded_invoice_amount.set(Some(amount));
                    }
                }

                pay_value_handle.set(value);
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

    let state_clone = state.clone();
    let close = {
        let invoice = invoice.clone();
        let pay_invoice = pay_invoice.clone();
        let decoded_invoice_desc = decoded_invoice_desc.clone();
        let decoded_invoice_amount = decoded_invoice_amount.clone();
        let amount_value_handle = amount_value_handle.clone();
        let description_value_handle = description_value_handle.clone();
        let pay_value_handle = pay_value_handle.clone();
        Callback::from(move |_| {
            invoice.set(None);
            pay_invoice.set(false);
            amount_value_handle.set(Amount::ZERO);
            description_value_handle.set(String::new());

            decoded_invoice_desc.set(None);
            decoded_invoice_amount.set(None);
            pay_value_handle.set(String::new());
            state_clone.set(State::Transactions);
        })
    };

    let state_clone = state.clone();
    html! {
    <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "Lightning" }</h5>
    {
        match *state_clone {
        State::Transactions => {
            html!{
            <>
            <button onclick={receive} class="p-8 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Receive"}</button>
            <button onclick={pay_button} class="p-8 px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Send"}</button>
            </>
            }
        }
            State::Receive => {
                html! {

          <div class="relative z-0 w-full mb-6 group">
                                  <div class="relative z-0 w-full mb-6 group">
              <label for="description" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Description"}</label>
              <input type="text" name="description" id="description" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={description_node_ref} value={description_value} onchange={description_onchange} />
                    </div>
                <div class="relative z-0 w-full mb-6 group">
            <label for="push_amount" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Amount (sat)"}</label>
              <input type="numeric" name="push_amount" id="push_amount" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={amount_node_ref} value={amount_value.to_sat().to_string()} onchange={amount_onchange} />
                        </div>
        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
            <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={generate_invoice}>{"Create Invoice"}</button>
                <button class="px-6 py-2 rounded-sm" onclick={close}>{"Back"}</button>
        </div>
        </div>
                }
            }
            State::Send => {
                html!{
                    <>
        <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
            {"Pay Invoice"}
        </h2>
          <input name="pay_invoice" id="pay_invoice" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={pay_input_node_ref} value={pay_value} onchange={pay_onchange} />
          <label for="pay_invoice" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Peer Ip"}</label>


                        if let Some(desc) = decoded_invoice_desc.clone().as_deref() {
                                        <p class="font-normal text-gray-700 dark:text-gray-400"> { format!("Description: {}", desc ) } </p>
                            }


                        if let Some(amount) = (*decoded_invoice_amount).clone() {
                                        <p class="font-normal text-gray-700 dark:text-gray-400"> { format!("Description: {}", amount.to_sat() ) } </p>
                            }

        <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
            <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={send_pay}>{"Pay"}</button>
                <button class="px-6 py-2 rounded-sm" onclick={close}>{"Back"}</button>
        </div>

                </>
                    }
            }
    }}
        </a>
    }
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
