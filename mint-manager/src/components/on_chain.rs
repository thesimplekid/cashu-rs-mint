use gloo_net::http::Request;
use node_manager_types::responses::FundingAddressResponse;
use yew::prelude::*;

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

    html! {
            <>

        <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
                <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white">{ "On Chain" }</h5>
                if on_chain_address.is_some() {
            <h2 class="flex items-center gap-2 text-xl font-semibold leadi tracki">
                {"Generated Address"}
            </h2>
            <p class="flex-1 dark:text-gray-400">{on_chain_address.as_ref().unwrap() }</p>
            <div class="flex flex-col justify-end gap-3 mt-6 sm:flex-row">
                <button class="px-6 py-2 rounded-sm" onclick={close}>{"Back"}</button>
                // TODO: Copy button
                // <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">{"Copy"}</button>
        </div>
                } else {


            <button onclick={generate_address} class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900">
                 { "Generate Address" }
             </button>
        }
    </a>
            </>
            }
}
