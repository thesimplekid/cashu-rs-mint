use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bip39::{Language, Mnemonic};
use cashu_crab::lightning_invoice::Invoice;
use cashu_crab::mint::Mint;
use cashu_crab::Amount;
use cashu_crab::Sha256;
use futures::{Stream, StreamExt};
use gl_client::bitcoin::Network;
use gl_client::node::ClnClient;
use gl_client::pb::cln;
use gl_client::pb::cln::Amount as Cln_Amount;
use gl_client::pb::cln::{
    AmountOrAny, ListinvoicesRequest, ListinvoicesResponse, PayRequest, PayResponse,
    WaitanyinvoiceRequest, WaitanyinvoiceResponse,
};
use gl_client::scheduler::Scheduler;
use gl_client::signer::model::cln::amount_or_any::Value;
use gl_client::signer::model::greenlight::cln::InvoiceResponse;
use gl_client::signer::Signer;
use gl_client::tls::TlsConfig;
use log::debug;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::database::Db;
use crate::utils::unix_time;

use super::{Error, InvoiceInfo, InvoiceStatus, LnProcessor};

#[derive(Clone)]
pub struct Greenlight {
    signer: Signer,
    signer_tx: tokio::sync::mpsc::Sender<()>,
    node: Arc<Mutex<ClnClient>>,
    db: Db,
    mint: Arc<Mutex<Mint>>,
}

impl Greenlight {
    pub async fn new(db: Db, mint: Arc<Mutex<Mint>>) -> Self {
        let mut rng = rand::thread_rng();
        let m = Mnemonic::generate_in_with(&mut rng, Language::English, 24).unwrap();
        let phrase = m.word_iter().fold("".to_string(), |c, n| c + " " + n);

        // Prompt user to safely store the phrase

        let seed = m.to_seed("");

        let tls = TlsConfig::new().unwrap();

        let secret = seed[0..32].to_vec();

        let signer = Signer::new(secret.clone(), Network::Bitcoin, tls).unwrap();

        let scheduler = Scheduler::new(signer.node_id(), Network::Bitcoin)
            .await
            .unwrap();

        // Passing in the signer is required because the client needs to prove
        // ownership of the `node_id`
        let res = scheduler
            .register(&signer, Some("".to_string()))
            .await
            .unwrap();

        let tls = TlsConfig::new().unwrap().identity(
            res.device_cert.as_bytes().to_vec(),
            res.device_key.as_bytes().to_vec(),
        );

        // Use the configured `tls` instance when creating `Scheduler` and `Signer`
        // instance going forward
        let signer = Signer::new(secret, Network::Bitcoin, tls.clone()).unwrap();
        let scheduler =
            Scheduler::with(signer.node_id(), Network::Bitcoin, "uri".to_string(), &tls)
                .await
                .unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let signer_clone = signer.clone();
        tokio::spawn(async move {
            if let Err(err) = signer_clone.run_forever(rx).await {
                debug!("{:?}", err);
            }
        });

        let node: gl_client::node::ClnClient = scheduler.schedule(tls).await.unwrap();
        let node = Arc::new(Mutex::new(node));

        Self {
            signer,
            signer_tx: tx,
            node,
            db,
            mint,
        }
    }
}

#[async_trait]
impl LnProcessor for Greenlight {
    async fn get_invoice(
        &self,
        amount: Amount,
        hash: Sha256,
        description: &str,
    ) -> Result<InvoiceInfo, Error> {
        let mut cln_client = self.node.lock().await;

        let cln_response = cln_client
            .invoice(cln::InvoiceRequest {
                amount_msat: Some(AmountOrAny {
                    value: Some(Value::Amount(Cln_Amount {
                        msat: u64::from(amount) * 1000,
                    })),
                }),
                description: description.to_string(),
                label: Uuid::new_v4().to_string(),
                expiry: None,
                fallbacks: vec![],
                preimage: None,
                cltv: None,
                deschashonly: Some(true),
            })
            .await
            .map_err(|_| Error::Custom("Tonic Error".to_string()))?;

        let InvoiceResponse {
            bolt11,
            payment_hash,
            ..
        } = cln_response.into_inner();

        let invoice = {
            let invoice = Invoice::from_str(&bolt11)?;
            let payment_hash = Sha256::from_str(&String::from_utf8(payment_hash)?)?;
            let invoice_info = InvoiceInfo::new(
                payment_hash,
                hash,
                invoice,
                amount,
                super::InvoiceStatus::Unpaid,
                "",
                None,
            );

            self.db.add_invoice(&invoice_info).await?;
            invoice_info
        };

        Ok(invoice)
    }

    async fn wait_invoice(&self) -> Result<(), Error> {
        let last_pay_index = self.db.get_last_pay_index().await?;
        let node = self.node.clone();
        let mut invoices = invoice_stream(node, Some(last_pay_index)).await?;

        while let Some(invoice) = invoices.next().await {
            if let Some(pay_idx) = invoice.pay_index {
                self.db.set_last_pay_index(pay_idx).await?;
            }

            let payment_hash = Sha256::from_str(&String::from_utf8(invoice.payment_hash)?)?;

            let mut invoice_info = self
                .db
                .get_invoice_info_by_payment_hash(&payment_hash)
                .await?;

            invoice_info.status = InvoiceStatus::Paid;
            invoice_info.confirmed_at = Some(unix_time());

            let mut mint = self.mint.lock().await;

            self.db.add_invoice(&invoice_info).await?;
        }

        Ok(())
    }

    async fn check_invoice_status(&self, payment_hash: &Sha256) -> Result<InvoiceStatus, Error> {
        let mut cln_client = self.node.lock().await;

        let cln_response = cln_client
            .list_invoices(ListinvoicesRequest {
                payment_hash: Some(payment_hash.to_string().as_bytes().to_vec()),
                ..Default::default()
            })
            .await
            .map_err(|_| Error::Custom("Tonic Error".to_string()))?;

        let ListinvoicesResponse { invoices, .. } = cln_response.into_inner();

        let status = {
            debug!("{:?}", invoices);
            let i = invoices[0].clone();

            i.status().into()
        };

        let mut invoice = self
            .db
            .get_invoice_info_by_payment_hash(payment_hash)
            .await?;

        invoice.status = status;

        self.db.add_invoice(&invoice).await?;

        let mut mint = self.mint.lock().await;

        Ok(status)
    }

    async fn pay_invoice(
        &self,
        invoice: Invoice,
        max_fee: Option<Amount>,
    ) -> Result<(String, Amount), Error> {
        let mut cln_client = self.node.lock().await;

        let maxfee = max_fee.map(|amount| Cln_Amount {
            msat: amount.to_msat(),
        });

        let cln_response = cln_client
            .pay(PayRequest {
                bolt11: invoice.to_string(),
                maxfee,
                ..Default::default()
            })
            .await
            .map_err(|_| Error::Custom("Tonic Error".to_string()))?;

        let PayResponse {
            payment_preimage,
            amount_sent_msat,
            ..
        } = cln_response.into_inner();
        let invoice = (
            serde_json::to_string(&payment_preimage)?,
            Amount::from_msat(amount_sent_msat.unwrap().msat),
        );

        Ok(invoice)
    }
}

async fn invoice_stream(
    cln_client: Arc<Mutex<ClnClient>>,
    last_pay_index: Option<u64>,
) -> Result<impl Stream<Item = WaitanyinvoiceResponse>, Error> {
    let cln_client = cln_client.lock().await.clone();
    Ok(futures::stream::unfold(
        (cln_client, last_pay_index),
        |(mut cln_client, mut last_pay_idx)| async move {
            // We loop here since some invoices aren't zaps, in which case we wait for the next one and don't yield
            loop {
                // info!("Waiting for index: {last_pay_idx:?}");
                let invoice_res = cln_client
                    .wait_any_invoice(WaitanyinvoiceRequest {
                        lastpay_index: last_pay_idx,
                        timeout: None,
                    })
                    .await;

                let invoice: WaitanyinvoiceResponse = invoice_res.unwrap().into_inner();

                last_pay_idx = invoice.pay_index;

                break Some(((invoice), (cln_client, last_pay_idx)));
            }
        },
    )
    .boxed())
}

pub fn fee_reserve(invoice_amount: Amount) -> Amount {
    let fee_reserse = (u64::from(invoice_amount) as f64 * 0.01) as u64;

    Amount::from(fee_reserse)
}
