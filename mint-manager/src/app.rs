use cashu_crab::Amount;
use gloo_net::http::Request;
use node_manager_types::responses::BalanceResponse;
use yew::prelude::*;

use crate::components::{channels::Channels, ln::Ln, on_chain::OnChain};

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

#[function_component(Cashu)]
pub fn cashu() -> HtmlResult {
    let balance = use_state(|| Amount::ZERO);
    {
        let balance = balance.clone();
        use_effect_with_deps(
            move |_| {
                let balance = balance.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let fetched_balance: Amount = Request::get("http://127.0.0.1:8086/outstanding")
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

    Ok(html! {
    <>

    <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
        <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { format!("Cashu Outstanding: {} sats", balance.to_sat()) } </h5>
    </a>

    </>
    })
}

#[function_component(App)]
pub fn app() -> Html {
    html! {
            <main>
      <div class="flex flex-row mb-4">
        <div class="p-2 w-1/3">
          <Balance/>
        </div>
        <div class="p-2 w-1/3">
          <Cashu/>
        </div>
      </div>
      <div class="flex flex-row">
        <div class="p-2 w-full">
          <OnChain/>
        </div>
        <div class="p-2 w-full">
          <Ln/>
        </div>
        <div class="p-2 w-full">
          <Channels/>
        </div>
      </div>
    </main>

        }
}
