use cashu_crab::Amount;
use yew::prelude::*;

#[derive(Properties, PartialEq, Default, Clone)]
pub struct Props {
    pub on_chain_confirmed: Amount,
    pub on_chain_pending: Amount,
    pub ln: Amount,
}

pub struct Balance;

impl Component for Balance {
    type Message = ();
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let total_balance =
            ctx.props().on_chain_confirmed + ctx.props().on_chain_pending + ctx.props().ln;
        html! {
            <>
        <a class="block flex-1 p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
            <h5 class="mb-2 text-2xl font-bold tracking-tight text-gray-900 dark:text-white"> { format!("Total Balance: {} sats", total_balance.to_sat()) } </h5>
            <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Lighting: {}", ctx.props().ln.to_sat())}</p>
            <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Onchain Spendable: {}", ctx.props().on_chain_confirmed.to_sat())}</p>
            <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Onchain Pending: {}", ctx.props().on_chain_pending.to_sat())}</p>
        </a>
            </>
            }
    }
}
