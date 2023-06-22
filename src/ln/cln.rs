use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use async_trait::async_trait;
use cashu_crab::lightning_invoice::Invoice;
use cashu_crab::mint::Mint;
use cashu_crab::Amount;
use cashu_crab::Sha256;
use cln_rpc::model::{
    InvoiceRequest, ListinvoicesRequest, PayRequest, WaitanyinvoiceRequest, WaitanyinvoiceResponse,
};
use cln_rpc::primitives::{Amount as CLN_Amount, AmountOrAny};
use futures::{Stream, StreamExt};
use log::{debug, warn};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::database::Db;
use crate::utils::unix_time;

use super::{InvoiceInfo, InvoiceStatus, LnProcessor};

#[derive(Clone)]
pub struct Cln {
    rpc_socket: PathBuf,
    db: Db,
    mint: Arc<Mutex<Mint>>,
}

impl Cln {
    pub async fn new(rpc_socket: PathBuf, db: Db, mint: Arc<Mutex<Mint>>) -> Self {
        Self {
            rpc_socket,
            db,
            mint,
        }
    }
}

#[async_trait]
impl LnProcessor for Cln {
    async fn get_invoice(
        &self,
        amount: Amount,
        hash: Sha256,
        description: &str,
    ) -> Result<InvoiceInfo> {
        let mut cln_client = cln_rpc::ClnRpc::new(&self.rpc_socket).await?;

        let cln_response = cln_client
            .call(cln_rpc::Request::Invoice(InvoiceRequest {
                amount_msat: AmountOrAny::Amount(CLN_Amount::from_sat(amount.into())),
                description: description.to_string(),
                label: Uuid::new_v4().to_string(),
                expiry: None,
                fallbacks: None,
                preimage: None,
                cltv: None,
                deschashonly: Some(true),
            }))
            .await?;

        let invoice = match cln_response {
            cln_rpc::Response::Invoice(invoice_response) => {
                let invoice = Invoice::from_str(&invoice_response.bolt11)?;
                let payment_hash = Sha256::from_str(&invoice_response.payment_hash.to_string())?;
                let invoice_info = InvoiceInfo::new(
                    payment_hash,
                    hash,
                    invoice,
                    amount,
                    super::InvoiceStatus::Unpaid,
                    "",
                    None,
                );

                invoice_info
            }
            _ => panic!("CLN returned wrong response kind"),
        };
        Ok(invoice)
    }

    async fn wait_invoice(&self) -> Result<()> {
        let last_pay_index = self.db.get_last_pay_index().await?;
        let mut invoices = invoice_stream(&self.rpc_socket, Some(last_pay_index)).await?;

        while let Some(invoice) = invoices.next().await {
            if let Some(pay_idx) = invoice.pay_index {
                self.db.set_last_pay_index(pay_idx).await?;
            }

            let payment_hash = Sha256::from_str(&invoice.payment_hash.to_string())?;

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

    async fn check_invoice_status(&self, payment_hash: &Sha256) -> Result<InvoiceStatus> {
        let mut cln_client = cln_rpc::ClnRpc::new(&self.rpc_socket).await?;

        let cln_response = cln_client
            .call(cln_rpc::Request::ListInvoices(ListinvoicesRequest {
                payment_hash: Some(payment_hash.to_string()),
                label: None,
                invstring: None,
                offer_id: None,
            }))
            .await?;

        let status = match cln_response {
            cln_rpc::Response::ListInvoices(invoice_response) => {
                debug!("{:?}", invoice_response);
                let i = invoice_response.invoices[0].clone();

                i.status.into()
            }
            _ => {
                warn!("CLN returned wrong response kind");
                bail!("CLN returned wrong response kind")
            }
        };

        let mut invoice = self
            .db
            .get_invoice_info_by_payment_hash(payment_hash)
            .await?;

        invoice.status = status;

        self.db.add_invoice(&invoice).await?;

        Ok(status)
    }

    async fn pay_invoice(
        &self,
        invoice: Invoice,
        max_fee: Option<Amount>,
    ) -> Result<(String, Amount)> {
        let mut cln_client = cln_rpc::ClnRpc::new(&self.rpc_socket).await?;

        let maxfee = max_fee.map(|amount| CLN_Amount::from_sat(u64::from(amount)));

        let cln_response = cln_client
            .call(cln_rpc::Request::Pay(PayRequest {
                bolt11: invoice.to_string(),
                amount_msat: None,
                label: None,
                riskfactor: None,
                maxfeepercent: None,
                retry_for: None,
                maxdelay: None,
                exemptfee: None,
                localinvreqid: None,
                exclude: None,
                maxfee,
                description: None,
            }))
            .await?;

        let invoice = match cln_response {
            cln_rpc::Response::Pay(pay_response) => (
                serde_json::to_string(&pay_response.payment_preimage)?,
                Amount::from(pay_response.amount_sent_msat.msat() / 1000),
            ),
            _ => panic!(),
        };

        Ok(invoice)
    }
}

async fn invoice_stream(
    socket_addr: &PathBuf,
    last_pay_index: Option<u64>,
) -> Result<impl Stream<Item = WaitanyinvoiceResponse>> {
    let cln_client = cln_rpc::ClnRpc::new(&socket_addr).await?;

    Ok(futures::stream::unfold(
        (cln_client, last_pay_index),
        |(mut cln_client, mut last_pay_idx)| async move {
            // We loop here since some invoices aren't zaps, in which case we wait for the next one and don't yield
            loop {
                // info!("Waiting for index: {last_pay_idx:?}");
                let invoice_res = cln_client
                    .call(cln_rpc::Request::WaitAnyInvoice(WaitanyinvoiceRequest {
                        timeout: None,
                        lastpay_index: last_pay_idx,
                    }))
                    .await;

                let invoice: WaitanyinvoiceResponse = match invoice_res {
                    Ok(invoice) => invoice,
                    Err(e) => {
                        warn!("Error fetching invoice: {e}");
                        // Let's not spam CLN with requests on failure
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        // Retry same request
                        continue;
                    }
                }
                .try_into()
                .expect("Wrong response from CLN");

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
