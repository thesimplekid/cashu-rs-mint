use anyhow::Result;
use cashu_crab::Amount;
use gloo::storage::LocalStorage;
use gloo_net::http::Request;
use gloo_storage::Storage;
use node_manager_types::responses::BalanceResponse;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::components::{
    balance::Balance, cashu::Cashu, channels::Channels, ln::Ln, login::Login, on_chain::OnChain,
};

async fn get_balances(jwt: &str, fetech_callback: Callback<BalanceResponse>) -> Result<()> {
    let fetched_balance: BalanceResponse = Request::get("http://127.0.0.1:8086/balance")
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    fetech_callback.emit(fetched_balance);
    Ok(())
}

async fn get_cashu(jwt: &str, fetech_callback: Callback<Amount>) -> Result<()> {
    let fetched_balance: Amount = Request::get("http://127.0.0.1:8086/outstanding")
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    fetech_callback.emit(fetched_balance);
    Ok(())
}

async fn check_login(jwt: &str, callback: Callback<bool>) -> Result<()> {
    let response = Request::post("http://127.0.0.1:8086/auth")
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?;

    if response.ok() {
        callback.emit(true);
    } else {
        callback.emit(false);
    }

    Ok(())
}

pub enum Msg {
    LoggedIn(String),
    FetechedBalances(BalanceResponse),
    FetechedCashu(Amount),
    CheckedAuth(bool),
}

#[derive(Debug, Clone, Default)]
pub struct App {
    jwt: Option<String>,
    on_chain_confimed: Amount,
    on_chain_pending: Amount,
    ln: Amount,
    cashu_in_circulation: Amount,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let jwt = if let Ok(jwt) = LocalStorage::get::<String>("auth_token") {
            let balance_callback = ctx.link().callback(Msg::FetechedBalances);
            let cashu_callback = ctx.link().callback(Msg::FetechedCashu);
            let jwt_clone = jwt.clone();
            let check_auth = ctx.link().callback(Msg::CheckedAuth);
            spawn_local(async move {
                if check_login(&jwt_clone, check_auth).await.is_ok() {
                    get_balances(&jwt_clone, balance_callback).await.ok();
                    get_cashu(&jwt_clone, cashu_callback).await.ok();
                }
            });

            Some(jwt)
        } else {
            None
        };

        Self {
            jwt,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::LoggedIn(jwt) => {
                let jwt_clone = jwt.clone();
                let balance_callback = ctx.link().callback(Msg::FetechedBalances);

                spawn_local(async move {
                    get_balances(&jwt_clone, balance_callback).await.ok();
                });

                self.jwt = Some(jwt);

                true
            }
            Msg::FetechedBalances(balance_response) => {
                self.on_chain_confimed = balance_response.on_chain_spendable;
                self.on_chain_pending =
                    balance_response.on_chain_total - balance_response.on_chain_spendable;
                self.ln = balance_response.ln;

                true
            }
            Msg::FetechedCashu(amount) => {
                self.cashu_in_circulation = amount;

                true
            }
            Msg::CheckedAuth(status) => {
                if !status {
                    LocalStorage::delete("auth_token");
                    true
                } else {
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let logged_in_callback = ctx.link().callback(Msg::LoggedIn);

        html! {
                <main>
            if self.jwt.is_some() {
          <div class="flex flex-row mb-4">
            <div class="p-2 w-1/3">
              <Balance on_chain_confimed={self.on_chain_confimed} on_chain_pending={self.on_chain_pending} ln={self.ln}/>
            </div>
            <div class="p-2 w-1/3">
              <Cashu balance={self.cashu_in_circulation}/>
            </div>
          </div>
          <div class="flex flex-row">
            <div class="p-2 w-full">
              <OnChain jwt={self.jwt.clone().unwrap()}/>
            </div>
            <div class="p-2 w-full">
              <Ln jwt={self.jwt.clone().unwrap()}/>
            </div>
            <div class="p-2 w-full">
              <Channels jwt={self.jwt.clone().unwrap()} />
            </div>
          </div>}
            else {

            <Login {logged_in_callback} />}
        </main>

            }
    }
}
