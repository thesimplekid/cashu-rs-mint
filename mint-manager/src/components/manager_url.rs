use std::str::FromStr;

use gloo::storage::{LocalStorage, Storage};
use log::warn;
use url::Url;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::app::NODE_URL_KEY;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub url_set_cb: Callback<Url>,
}

pub enum Msg {
    Submit,
}

#[derive(Default)]
pub struct SetManagerUrl {
    input_node_ref: NodeRef,
}

impl Component for SetManagerUrl {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Submit => {
                let pubkey = self
                    .input_node_ref
                    .cast::<HtmlInputElement>()
                    .map(|i| Url::from_str(&i.value()));

                if let Some(Ok(url)) = pubkey {
                    if let Err(err) = LocalStorage::set(NODE_URL_KEY, url.clone()) {
                        warn!("{:?}", err);
                    }
                    ctx.props().url_set_cb.emit(url);
                }

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_submit = ctx.link().callback(|_| Msg::Submit);
        html! {
                                <>
                            <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">


                            <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { "Mint Url" } </h5>
                                  <div>
                                      <div class="relative z-0 w-full mb-6 group">
                                  <input name="mint_manager_url" id="peer_pubkey" class="block py-2.5 px-0 w-full text-sm text-gray-900 bg-transparent border-0 border-b-2 border-gray-300 appearance-none dark:text-white dark:border-gray-600 dark:focus:border-blue-500 focus:outline-none focus:ring-0 focus:border-blue-600 peer" ref={self.input_node_ref.clone()} />
                                  <label for="mint_manager_url" class="peer-focus:font-medium absolute text-sm text-gray-500 dark:text-gray-400 duration-300 transform -translate-y-6 scale-75 top-3 -z-10 origin-[0] peer-focus:left-0 peer-focus:text-blue-600 peer-focus:dark:text-blue-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:translate-y-0 peer-focus:scale-75 peer-focus:-translate-y-6">{"Mint Manger Url"}</label>
                                  </div>
                                    </div>
                                <button class="px-6 py-2 rounded-sm shadow-sm dark:bg-violet-400 dark:text-gray-900" onclick={on_submit}>{"Connect Peer"}</button>
                        </a>
            </>
        }
    }
}
