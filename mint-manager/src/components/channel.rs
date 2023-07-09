use bitcoin::secp256k1::PublicKey;
use cashu_crab::Amount;
use node_manager_types::ChannelStatus;
use yew::prelude::*;

#[derive(PartialEq, Properties, Clone)]
pub struct ChannelProps {
    // pub ontoggle: Callback<usize>,
    pub delete_channel: Callback<(Option<PublicKey>, String)>,
    // pub onedit: Callback<(usize, String)>,
    pub channel_id: String,
    pub peer_id: Option<PublicKey>,
    pub local_balance: Amount,
    pub remote_balance: Amount,
    pub status: ChannelStatus,
}

#[function_component(Channel)]
pub fn entry(props: &ChannelProps) -> Html {
    let ChannelProps {
        delete_channel,
        channel_id,
        peer_id,
        local_balance,
        remote_balance,
        status,
    } = props.clone();

    let on_delete = {
        let delete_channel = delete_channel.clone();
        move |_| delete_channel.emit((peer_id.clone(), channel_id.clone()))
    };

    html! {
        <a class="block p-6 bg-white border border-gray-200 rounded-lg shadow hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700 dark:hover:bg-gray-700">
      <div class="flex flex-row mb-4">
        <div class="p-2 w-full">
        if let Some(peer) = peer_id {

            <p class="font-normal text-gray-700 dark:text-gray-400"> {
                format!("Peer id: {}...{}", &peer.to_string()[..8], &peer.to_string()[56..]) } </p>
        }
            <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Local Balance: {}", local_balance.to_sat())}</p>
            <p class="font-normal text-gray-700 dark:text-gray-400">{format!("Remote Balance: {}", remote_balance.to_sat())}</p>
            <p class="font-normal text-gray-700 dark:text-gray-400">{ format!("Status: {}", status.to_string() ) }</p>
        </div>
        <div class="p-2 w-full flex justify-end">
            <button type="button" class="focus:outline-none text-white bg-red-700 hover:bg-red-800 focus:ring-4 focus:ring-red-300 font-medium rounded-lg text-sm px-5 py-2.5 mr-2 mb-2 dark:bg-red-600 dark:hover:bg-red-700 dark:focus:ring-red-900" onclick={on_delete}> {"Close"} </button>
        </div>
        </div>
        </a>
    }
}