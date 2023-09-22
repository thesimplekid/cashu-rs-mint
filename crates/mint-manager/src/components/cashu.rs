use cashu_sdk::Amount;
use yew::prelude::*;

#[derive(Properties, PartialEq, Default, Clone)]
pub struct Props {
    pub balance: Amount,
}

pub struct Cashu;

impl Component for Cashu {
    type Message = ();
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
                <>
        <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
            <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { format!("Cashu Outstanding: {} sats", ctx.props().balance.to_sat()) } </h5>
        </a>
                </>
                }
    }
}
