use std::str::FromStr;

use cashu_crab::Amount;
use gloo::storage::LocalStorage;
use gloo_net::http::Request;
use gloo_storage::Storage;
use log::debug;
use node_manager_types::responses;
use node_manager_types::responses::BalanceResponse;
use nostr::event::builder::EventBuilder;
use nostr::event::kind::Kind;
use nostr::event::unsigned::UnsignedEvent;
use nostr::event::Event;
use nostr::key::Keys;
use nostr::secp256k1::XOnlyPublicKey;
use yew::prelude::*;

use crate::{
    bindings,
    components::{channels::Channels, ln::Ln, on_chain::OnChain},
};

async fn get_pubkey() -> Option<String> {
    let key = bindings::get_pubkey().await;
    key.as_string()
}

async fn sign_event(event: UnsignedEvent) -> Option<Event> {
    let signed_event = bindings::sign_event(
        event.created_at.as_i64(),
        event.content,
        event.pubkey.to_string(),
    )
    .await
    .as_string();

    if let Some(event) = signed_event {
        let event: Event = serde_json::from_str(&event).unwrap();
        debug!("sig: {:?}", event.as_json());
        return Some(event);
    }

    None
}

#[function_component(Login)]
pub fn login() -> Html {
    let onclick = {
        move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let pubkey = get_pubkey().await.unwrap();
                let pubkey = XOnlyPublicKey::from_str(&pubkey).unwrap();

                let keys = Keys::from_public_key(pubkey);

                let event =
                    EventBuilder::new(Kind::TextNote, "", &[]).to_unsigned_event(keys.public_key());

                let signed_event = sign_event(event).await.unwrap();

                debug!("{:?}", signed_event.as_json());
                let jwt: responses::LoginResponse =
                    Request::post("http://127.0.0.1:8086/nostr-login")
                        .json(&signed_event)
                        .unwrap()
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();

                debug!("{:?}", jwt);
                LocalStorage::set("auth_token", jwt.token).unwrap();
            })
        }
    };

    html! {
        <button type="button" class="focus:outline-none text-white bg-red-700 hover:bg-red-800 focus:ring-4 focus:ring-red-300 font-medium rounded-lg text-sm px-5 py-2.5 mr-2 mb-2 dark:bg-red-600 dark:hover:bg-red-700 dark:focus:ring-red-900" {onclick}> {"Sign In"} </button>
    }
}

#[function_component(Balance)]
pub fn balance() -> HtmlResult {
    let balance = use_state(|| BalanceResponse::default());
    {
        let balance = balance.clone();
        use_effect_with_deps(
            move |_| {
                let balance = balance.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(jwt) = LocalStorage::get::<String>("auth_token") {
                        let fetched_balance: BalanceResponse =
                            Request::get("http://127.0.0.1:8086/balance")
                                .header("Authorization", &format!("Bearer {}", jwt))
                                .send()
                                .await
                                .unwrap()
                                .json()
                                .await
                                .unwrap();
                        balance.set(fetched_balance);
                    }
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
                    if let Ok(jwt) = LocalStorage::get::<String>("auth_token") {
                        let fetched_balance: Amount =
                            Request::get("http://127.0.0.1:8086/outstanding")
                                .header("Authorization", &format!("Bearer {}", jwt))
                                .send()
                                .await
                                .unwrap()
                                .json()
                                .await
                                .unwrap();
                        balance.set(fetched_balance);
                    }
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
        <Login/>
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
