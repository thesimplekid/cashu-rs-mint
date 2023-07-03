use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bitcoin_hashes::Hash;
use cashu_crab::Amount;
use cashu_crab::Sha256;
use ldk_node::bitcoin::Network;
use ldk_node::io::SqliteStore;
use ldk_node::lightning_invoice::Invoice;
use ldk_node::{Builder, Config};
use ldk_node::{Event, Node};
use log::debug;

use crate::config::Settings;
use crate::database::Db;
use crate::utils::unix_time;

use super::Error;
use super::InvoiceInfo;
use super::InvoiceStatus;
use super::LnNodeManager;
use super::LnProcessor;

const SECS_IN_DAY: u32 = 86400;

#[derive(Clone)]
pub struct Ldk {
    pub node: Arc<Node<SqliteStore>>,
    pub db: Db,
}

impl Ldk {
    pub async fn new(_settings: &Settings, db: Db) -> anyhow::Result<Self> {
        let config = Config {
            log_level: ldk_node::LogLevel::Info,
            ..Default::default()
        };
        let mut builder = Builder::from_config(config);
        builder.set_entropy_seed_path("./myseed".to_string());
        builder.set_network(Network::Signet);
        builder.set_esplora_server("https://mutinynet.com/api".to_string());
        builder.set_gossip_source_rgs("https://rgs.mutinynet.com/snapshot/".to_string());
        /*
        builder.set_esplora_server("https://blockstream.info/testnet/api".to_string());
        builder.set_gossip_source_rgs(
            "https://rapidsync.lightningdevkit.org/testnet/snapshot".to_string(),
        );
        */

        let node = Arc::new(builder.build()?);

        node.start()?;

        Ok(Self { node, db })
    }
}

#[async_trait]
impl LnProcessor for Ldk {
    async fn get_invoice(
        &self,
        amount: Amount,
        hash: Sha256,
        description: &str,
    ) -> Result<InvoiceInfo, Error> {
        let invoice = self
            .node
            .receive_payment(amount.to_msat(), description, SECS_IN_DAY)?;

        let inoice_info = InvoiceInfo::new(
            Sha256::from_str(&invoice.payment_hash().to_owned().to_string())?,
            hash,
            invoice,
            amount,
            InvoiceStatus::Unpaid,
            "",
            None,
        );
        Ok(inoice_info)
    }

    async fn wait_invoice(&self) -> Result<(), Error> {
        while let Some(event) = self.node.next_event() {
            match event {
                Event::PaymentReceived {
                    payment_hash,
                    amount_msat: _,
                } => {
                    let payment_hash =
                        Sha256::from_str(&String::from_utf8(payment_hash.0.to_vec())?)?;

                    let mut invoice_info = self
                        .db
                        .get_invoice_info_by_payment_hash(&payment_hash)
                        .await?;

                    invoice_info.status = InvoiceStatus::Paid;
                    invoice_info.confirmed_at = Some(unix_time());

                    self.db.add_invoice(&invoice_info).await?;

                    self.node.event_handled();
                }
                _ => {
                    debug!("{:?}", event);
                    // TODO: Do something with this
                    self.node.event_handled();
                }
            }
        }
        Ok(())
    }

    async fn check_invoice_status(&self, payment_hash: &Sha256) -> Result<InvoiceStatus, Error> {
        let payment_hash = ldk_node::lightning::ln::PaymentHash(payment_hash.to_byte_array());

        let payment = self
            .node
            .list_payments_with_filter(|p| p.hash == payment_hash);

        let status = payment[0].status.into();

        Ok(status)
    }

    async fn pay_invoice(
        &self,
        invoice: Invoice,
        _max_fee: Option<Amount>,
    ) -> Result<(String, Amount), Error> {
        let payment_hash = self.node.send_payment(&invoice)?;
        let payment = self
            .node
            .list_payments_with_filter(|p| p.hash == payment_hash);

        Ok((
            String::from_utf8(payment_hash.0.to_vec())?,
            Amount::from_msat(payment[0].amount_msat.unwrap()),
        ))
    }
}
