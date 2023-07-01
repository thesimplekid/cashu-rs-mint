use cashu_crab::Invoice;
use serde::{Deserialize, Serialize};

pub mod requests {
    use bitcoin::secp256k1::PublicKey;
    use cashu_crab::Amount;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CreateInvoiceParams {
        pub msat: u64,
        pub description: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct KeysendRequest {
        pub amount: u64,
        pub pubkey: PublicKey,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OpenChannelRequest {
        pub public_key: PublicKey,
        pub ip: String,
        pub port: u16,
        pub amount: Amount,
        pub push_amount: Option<Amount>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PayOnChainRequest {
        pub sat: u64,
        pub address: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CloseChannel {
        pub channel_id: Vec<u8>,
        pub peer_id: Option<PublicKey>,
    }
}

pub mod responses {
    use bitcoin::secp256k1::PublicKey;
    use cashu_crab::{types::InvoiceStatus, Amount, Sha256};
    // use ldk_node::ChannelDetails;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PayInvoiceResponse {
        pub paymnet_hash: Sha256,
        pub status: InvoiceStatus,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FundingAddressResponse {
        pub address: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct BalanceResponse {
        pub on_chain_spendable: Amount,
        pub on_chain_total: Amount,
        pub ln: Amount,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChannelInfo {
        pub peer_pubkey: PublicKey,
        pub channel_id: Vec<u8>,
        pub balance: Amount,
        pub value: Amount,
        pub is_usable: bool,
    }
    /*
    impl From<ChannelDetails> for ChannelInfo {
        fn from(channel_details: ChannelDetails) -> Self {
            Self {
                peer_pubkey: channel_details.counterparty_node_id,
                balance: Amount::from_msat(channel_details.balance_msat),
                value: Amount::from_sat(channel_details.channel_value_sats),
                is_usable: channel_details.is_usable,
            }
        }
    }
    */
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11 {
    pub bolt11: Invoice,
}
