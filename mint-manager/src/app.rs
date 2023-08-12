use std::str::FromStr;

use anyhow::Result;
use cashu_crab::Amount;
use gloo::storage::{LocalStorage, Storage};
use gloo_net::http::Request;
use ln_rs_models::responses::BalanceResponse;
use log::debug;
use url::Url;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::components::manager_url::SetManagerUrl;
use crate::utils::ln_rs_amount_to_cashu_crab_amount;

use crate::components::{
    balance::Balance, cashu::Cashu, channels::Channels, ln::Ln, login::Login, on_chain::OnChain,
};

pub const NODE_URL_KEY: &str = "node_url";

pub const JWT_KEY: &str = "auth_token";

async fn get_balances(
    url: &Url,
    jwt: &str,
    fetech_callback: Callback<BalanceResponse>,
) -> Result<()> {
    let url = url.join("/balance")?;

    let fetched_balance: BalanceResponse = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    fetech_callback.emit(fetched_balance);
    Ok(())
}

async fn get_cashu(url: &Url, jwt: &str, fetech_callback: Callback<Amount>) -> Result<()> {
    let url = url.join("/outstanding")?;
    let fetched_balance: Amount = Request::get(url.as_str())
        .header("Authorization", &format!("Bearer {}", jwt))
        .send()
        .await?
        .json()
        .await?;

    fetech_callback.emit(fetched_balance);
    Ok(())
}

async fn check_login(url: &Url, jwt: &str, callback: Callback<bool>) -> Result<()> {
    let url = url.join("/auth")?;
    let response = Request::post(url.as_str())
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

#[derive(Debug, Default)]
pub enum View {
    #[default]
    Dashboard,
    SetUrl,
    Login,
}

pub enum Msg {
    LoggedIn(String),
    FetechedBalances(BalanceResponse),
    FetechedCashu(Amount),
    CheckedAuth(bool),
    UrlSet(Url),
}

#[derive(Debug, Default)]
pub struct App {
    view: View,
    jwt: Option<String>,
    node_url: Option<Url>,
    on_chain_confirmed: Amount,
    on_chain_pending: Amount,
    ln: Amount,
    cashu_in_circulation: Amount,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let node_url: Option<Url> = LocalStorage::get::<String>(NODE_URL_KEY)
            .ok()
            .and_then(|u| Url::from_str(&u).ok());

        let jwt = LocalStorage::get::<String>(JWT_KEY);

        debug!("node: {:?}", node_url);

        debug!("Jwt: {:?}", jwt);

        match (node_url, jwt) {
            (Some(url), Ok(jwt)) => {
                let balance_callback = ctx.link().callback(Msg::FetechedBalances);
                let cashu_callback = ctx.link().callback(Msg::FetechedCashu);
                let jwt_clone = jwt.clone();
                let check_auth = ctx.link().callback(Msg::CheckedAuth);
                let node_url = url.clone();

                spawn_local(async move {
                    if check_login(&node_url, &jwt_clone, check_auth).await.is_ok() {
                        get_balances(&node_url, &jwt_clone, balance_callback)
                            .await
                            .ok();
                        get_cashu(&node_url, &jwt_clone, cashu_callback).await.ok();
                    }
                });

                Self {
                    jwt: Some(jwt),
                    node_url: Some(url),
                    ..Default::default()
                }
            }
            // Mint Url is not set
            (None, _) => Self {
                view: View::SetUrl,
                ..Default::default()
            },
            // Mint url is set but user not logged in
            (Some(url), Err(_)) => Self {
                node_url: Some(url),
                view: View::Login,
                ..Default::default()
            },
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::LoggedIn(jwt) => {
                let jwt_clone = jwt.clone();
                let balance_callback = ctx.link().callback(Msg::FetechedBalances);
                let url = self.node_url.clone().unwrap();

                spawn_local(async move {
                    get_balances(&url, &jwt_clone, balance_callback).await.ok();
                });

                self.jwt = Some(jwt);

                true
            }
            Msg::FetechedBalances(balance_response) => {
                self.on_chain_confirmed =
                    ln_rs_amount_to_cashu_crab_amount(balance_response.on_chain_spendable);

                self.on_chain_pending = ln_rs_amount_to_cashu_crab_amount(
                    balance_response.on_chain_total - balance_response.on_chain_spendable,
                );
                self.ln = ln_rs_amount_to_cashu_crab_amount(balance_response.ln);

                true
            }
            Msg::FetechedCashu(amount) => {
                self.cashu_in_circulation = amount;

                true
            }
            Msg::CheckedAuth(status) => {
                if !status {
                    LocalStorage::delete(JWT_KEY);
                    true
                } else {
                    false
                }
            }
            Msg::UrlSet(url) => {
                self.node_url = Some(url);
                self.view = View::Login;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        log::debug!("{:?}", self.view);
        html! {
                        <main>

                    {

                         match &self.view {
                    View::Dashboard => {
                        html!{
                            <>
                  <div class="flex flex-row mb-4">
                    <div class="p-2 w-1/3">
                      <Balance on_chain_confirmed={self.on_chain_confirmed} on_chain_pending={self.on_chain_pending} ln={self.ln}/>
                    </div>
                    <div class="p-2 w-1/3">
                      <Cashu balance={self.cashu_in_circulation}/>
                    </div>
                  </div>
                  <div class="flex flex-row">
                    <div class="p-2 w-full">
                      <OnChain jwt={self.jwt.clone().unwrap()} url={self.node_url.clone().unwrap()}/>
                    </div>
                    <div class="p-2 w-full">
                      <Ln jwt={self.jwt.clone().unwrap()} url={self.node_url.clone().unwrap()}/>
                    </div>
                    <div class="p-2 w-full">
                      <Channels jwt={self.jwt.clone().unwrap()} url={self.node_url.clone().unwrap()} />
                    </div>
                            </div>

                            </>
                        }

                    }
                    View::Login => {
                let logged_in_callback = ctx.link().callback(Msg::LoggedIn);
                        html! {
                            <>
                    <Login {logged_in_callback} node_url={self.node_url.clone().unwrap()} />

                            </>
                        }
                    }
                    View::SetUrl => {

                        let url_set_cb = ctx.link().callback(Msg::UrlSet);
                        html!{

                        <>
                        <SetManagerUrl {url_set_cb} />
                        </>
                        }
                    }
                }
        }

                </main>
        }
    }
}
