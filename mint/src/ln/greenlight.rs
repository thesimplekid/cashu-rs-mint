use std::fs;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bip39::{Language, Mnemonic};
use bitcoin::secp256k1::PublicKey;
use bitcoin::Address;
use cashu_crab::lightning_invoice::Invoice;
use cashu_crab::mint::Mint;
use cashu_crab::types::InvoiceStatus as CrabInvoiceStatus;
use cashu_crab::Amount;
use cashu_crab::Sha256;
use futures::{Stream, StreamExt};
use gl_client::bitcoin::Network;
use gl_client::node::ClnClient;
use gl_client::pb::cln;
use gl_client::pb::cln::listfunds_outputs::ListfundsOutputsStatus;
use gl_client::pb::cln::pay_response::PayStatus;
use gl_client::scheduler::Scheduler;
use gl_client::signer::model::cln::amount_or_any::Value as SignerValue;
use gl_client::signer::model::cln::Amount as SignerAmount;
use gl_client::signer::model::cln::GetinfoRequest;
use gl_client::signer::model::cln::ListpeerchannelsRequest;
use gl_client::signer::model::greenlight::cln::InvoiceResponse;
use gl_client::signer::Signer;
use gl_client::tls::TlsConfig;
use node_manager_types::ChannelStatus;
use node_manager_types::{requests, responses, Bolt11};
use tokio::sync::Mutex;
use tracing::debug;
use tracing::log::warn;
use uuid::Uuid;

use crate::config::Settings;
use crate::database::Db;
use crate::utils::unix_time;

use super::LnNodeManager;
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
    pub async fn new(db: Db, settings: &Settings, mint: Arc<Mutex<Mint>>) -> Result<Self, Error> {
        let network = Network::Testnet;

        let mut rng = rand::thread_rng();

        let m = match fs::metadata("seed") {
            Ok(_) => {
                // FIXME:
                let seed = fs::read_to_string("seed")?;
                Mnemonic::from_str(&seed)?
            }
            Err(_) => Mnemonic::generate_in_with(&mut rng, Language::English, 24)?,
        };

        let phrase = m.word_iter().fold("".to_string(), |c, n| c + " " + n);

        // Prompt uoer to safely store the phrase
        // FIXME: don't just log real seeds
        tracing::info!("Seed Phrase: {:?}", phrase);

        let seed = m.to_seed("");

        let tls = TlsConfig::new()?;

        let secret = seed[0..32].to_vec();

        let signer = Signer::new(secret.clone(), network, tls)?;

        let scheduler = Scheduler::new(signer.node_id(), network).await?;

        let (device_cert, device_key) =
            match (fs::metadata("device_cert"), fs::metadata("device_key")) {
                (Ok(_), Ok(_)) => (
                    fs::read_to_string("device_cert")?,
                    fs::read_to_string("device_key")?,
                ),
                _ => {
                    // Passing in the signer is required because the client needs to prove
                    // ownership of the `node_id`
                    let res = scheduler
                        .register(&signer, settings.ln.greenlight_invite_code.clone())
                        .await?;
                    let device_cert = res.device_cert;
                    let device_key = res.device_key;

                    fs::write("device_cert", &device_cert)?;
                    fs::write("device_key", &device_key)?;

                    (device_cert, device_key)
                }
            };

        tracing::info!("cert {:?}", device_cert);
        tracing::info!("key {:?}", device_key);

        let tls = TlsConfig::new()?.identity(
            device_cert.as_bytes().to_vec(),
            device_key.as_bytes().to_vec(),
        );

        // Use the configured `tls` instance when creating `Scheduler` and `Signer`
        // instance going forward
        let signer = Signer::new(secret, network, tls.clone())?;

        let scheduler = Scheduler::new(signer.node_id(), network).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let signer_clone = signer.clone();
        tokio::spawn(async move {
            if let Err(err) = signer_clone.run_forever(rx).await {
                debug!("{:?}", err);
            }
        });

        let mut node: gl_client::node::ClnClient = scheduler.schedule(tls).await?;
        let info = node
            .getinfo(GetinfoRequest::default())
            .await
            .map_err(|x| Error::TonicError(x.to_string()))?;
        tracing::warn!("Info {:?}", info);

        let node = Arc::new(Mutex::new(node));
        tracing::warn!("Node up");
        Ok(Self {
            signer,
            signer_tx: tx,
            node,
            db,
            mint,
        })
    }

    pub async fn recover(
        seed_phrase: &str,
        db: Db,
        settings: &Settings,
        mint: Arc<Mutex<Mint>>,
    ) -> Result<Self, Error> {
        let network = Network::Testnet;
        let m = Mnemonic::parse(seed_phrase)?;
        let tls = TlsConfig::new()?;

        let seed = m.to_seed("");
        let secret = seed[0..32].to_vec();
        let signer = Signer::new(secret.clone(), network, tls)?;

        let scheduler = Scheduler::new(signer.node_id(), network).await?;

        let recover = scheduler.recover(&signer).await?;

        let device_cert = recover.device_cert;
        let device_key = recover.device_key;

        fs::write("device_cert", &device_cert)?;
        fs::write("device_key", &device_key)?;

        let tls = TlsConfig::new()?.identity(
            device_cert.as_bytes().to_vec(),
            device_key.as_bytes().to_vec(),
        );

        let signer = Signer::new(secret, network, tls.clone())?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let signer_clone = signer.clone();
        tokio::spawn(async move {
            if let Err(err) = signer_clone.run_forever(rx).await {
                debug!("{:?}", err);
            }
        });

        let mut node: gl_client::node::ClnClient = scheduler.schedule(tls).await?;
        let info = node
            .getinfo(GetinfoRequest::default())
            .await
            .map_err(|x| Error::TonicError(x.to_string()))?;

        tracing::warn!("Info {:?}", info);

        let node = Arc::new(Mutex::new(node));
        tracing::warn!("Node up");
        Ok(Self {
            signer,
            signer_tx: tx,
            node,
            db,
            mint,
        })
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
                amount_msat: Some(cln::AmountOrAny {
                    value: Some(SignerValue::Amount(cln::Amount {
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

            self.db.add_invoice(&invoice_info).await?;
        }

        Ok(())
    }

    async fn check_invoice_status(&self, payment_hash: &Sha256) -> Result<InvoiceStatus, Error> {
        let mut cln_client = self.node.lock().await;

        let cln_response = cln_client
            .list_invoices(cln::ListinvoicesRequest {
                payment_hash: Some(payment_hash.to_string().as_bytes().to_vec()),
                ..Default::default()
            })
            .await
            .map_err(|_| Error::Custom("Tonic Error".to_string()))?;

        let cln::ListinvoicesResponse { invoices, .. } = cln_response.into_inner();

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

        Ok(status)
    }

    async fn pay_invoice(
        &self,
        invoice: Invoice,
        max_fee: Option<Amount>,
    ) -> Result<(String, Amount), Error> {
        let mut cln_client = self.node.lock().await;

        let maxfee = max_fee.map(|amount| cln::Amount {
            msat: amount.to_msat(),
        });

        let cln_response = cln_client
            .pay(cln::PayRequest {
                bolt11: invoice.to_string(),
                maxfee,
                ..Default::default()
            })
            .await
            .map_err(|_| Error::Custom("Tonic Error".to_string()))?;

        let cln::PayResponse {
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
) -> Result<impl Stream<Item = cln::WaitanyinvoiceResponse>, Error> {
    let cln_client = cln_client.lock().await.clone();
    Ok(futures::stream::unfold(
        (cln_client, last_pay_index),
        |(mut cln_client, mut last_pay_idx)| async move {
            // We loop here since some invoices aren't zaps, in which case we wait for the next one and don't yield
            loop {
                // info!("Waiting for index: {last_pay_idx:?}");
                let invoice_res = cln_client
                    .wait_any_invoice(cln::WaitanyinvoiceRequest {
                        lastpay_index: last_pay_idx,
                        timeout: None,
                    })
                    .await;

                let invoice: cln::WaitanyinvoiceResponse = invoice_res.unwrap().into_inner();

                last_pay_idx = invoice.pay_index;

                break Some(((invoice), (cln_client, last_pay_idx)));
            }
        },
    )
    .boxed())
}

#[async_trait]
impl LnNodeManager for Greenlight {
    async fn new_onchain_address(&self) -> Result<Address, Error> {
        let mut node = self.node.lock().await;

        let new_addr = node
            .new_addr(cln::NewaddrRequest { addresstype: None })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?;

        let address = match new_addr.into_inner().bech32 {
            Some(addr) => addr,
            None => return Err(Error::Custom("Could not get address".to_string())),
        };

        let address = Address::from_str(&address)?.assume_checked();

        Ok(address)
    }

    async fn open_channel(
        &self,
        open_channel_request: requests::OpenChannelRequest,
    ) -> Result<String, Error> {
        let mut node = self.node.lock().await;

        let requests::OpenChannelRequest {
            public_key,
            host: _,
            port: _,
            amount,
            push_amount,
        } = open_channel_request;

        let amount = cln::AmountOrAll {
            value: Some(cln::amount_or_all::Value::Amount(SignerAmount {
                msat: amount.to_msat(),
            })),
        };

        let push_msat = push_amount.map(|pa| SignerAmount { msat: pa.to_msat() });

        let request = cln::FundchannelRequest {
            id: public_key.serialize().to_vec(),
            amount: Some(amount),
            push_msat,
            ..Default::default()
        };

        let response = node
            .fund_channel(request)
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?;

        let txid = response.into_inner().txid;

        Ok(String::from_utf8(txid)?)
    }

    async fn list_channels(&self) -> Result<Vec<responses::ChannelInfo>, Error> {
        let mut node = self.node.lock().await;

        let channels_response = node
            .list_peer_channels(ListpeerchannelsRequest { id: None })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();

        warn!("{:?}", channels_response);

        let channels = channels_response
            .channels
            .into_iter()
            .map(|x| from_list_channels_to_info(x).unwrap())
            .collect();

        Ok(channels)
    }

    async fn get_balance(&self) -> Result<responses::BalanceResponse, Error> {
        let mut node = self.node.lock().await;

        let response = node
            .list_funds(cln::ListfundsRequest { spent: None })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();

        let mut on_chain_total = Amount::default();

        let mut on_chain_spendable = Amount::ZERO;
        let mut ln = Amount::ZERO;

        for output in response.outputs {
            match &output.status() {
                ListfundsOutputsStatus::Unconfirmed => {
                    on_chain_total += Amount::from_msat(
                        output.amount_msat.unwrap_or(cln::Amount::default()).msat,
                    );
                }
                ListfundsOutputsStatus::Immature => {
                    on_chain_total += Amount::from_msat(
                        output.amount_msat.unwrap_or(cln::Amount::default()).msat,
                    );
                }
                ListfundsOutputsStatus::Confirmed => {
                    on_chain_total += Amount::from_msat(
                        output
                            .amount_msat
                            .clone()
                            .unwrap_or(cln::Amount::default())
                            .msat,
                    );
                    on_chain_spendable += Amount::from_msat(
                        output.amount_msat.unwrap_or(cln::Amount::default()).msat,
                    );
                }
                ListfundsOutputsStatus::Spent => (),
            }
        }

        for channel in response.channels {
            ln += Amount::from_msat(
                channel
                    .our_amount_msat
                    .unwrap_or(cln::Amount::default())
                    .msat,
            );
        }

        Ok(responses::BalanceResponse {
            on_chain_spendable,
            on_chain_total,
            ln,
        })
    }

    async fn pay_invoice(&self, bolt11: Bolt11) -> Result<responses::PayInvoiceResponse, Error> {
        let mut node = self.node.lock().await;
        let pay_request = cln::PayRequest {
            bolt11: bolt11.bolt11.to_string(),
            ..Default::default()
        };

        let response = node
            .pay(pay_request)
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();

        let status = match response.status() {
            PayStatus::Complete => CrabInvoiceStatus::Paid,
            PayStatus::Pending => CrabInvoiceStatus::InFlight,
            PayStatus::Failed => CrabInvoiceStatus::Expired,
        };

        Ok(responses::PayInvoiceResponse {
            payment_hash: Sha256::from_str(&String::from_utf8(response.payment_hash)?)?,
            status,
        })
    }

    async fn create_invoice(&self, amount: Amount, description: String) -> Result<Invoice, Error> {
        let mut node = self.node.lock().await;

        let amount_msat = cln::AmountOrAny {
            value: Some(cln::amount_or_any::Value::Amount(SignerAmount {
                msat: amount.to_msat(),
            })),
        };

        let response = node
            .invoice(cln::InvoiceRequest {
                amount_msat: Some(amount_msat),
                description,
                label: Uuid::new_v4().to_string(),
                expiry: Some(3600),
                fallbacks: vec![],
                preimage: None,
                cltv: None,
                deschashonly: None,
            })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();
        let bolt11 = response.bolt11;

        Ok(Invoice::from_str(&bolt11)?)
    }

    async fn pay_on_chain(&self, address: Address, amount: Amount) -> Result<String, Error> {
        let mut node = self.node.lock().await;

        let satoshi = Some(cln::AmountOrAll {
            value: Some(cln::amount_or_all::Value::Amount(cln::Amount {
                msat: amount.to_msat(),
            })),
        });

        let response = node
            .withdraw(cln::WithdrawRequest {
                destination: address.to_string(),
                satoshi,
                ..Default::default()
            })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();

        Ok(String::from_utf8(response.txid)?)
    }

    async fn close(&self, channel_id: String, peer_id: Option<PublicKey>) -> Result<(), Error> {
        let mut node = self.node.lock().await;

        let destination = peer_id.map(|x| x.to_string());
        let _response = node
            .close(cln::CloseRequest {
                id: channel_id,
                destination,
                ..Default::default()
            })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?;
        Ok(())
    }

    async fn pay_keysend(&self, destination: PublicKey, amount: Amount) -> Result<String, Error> {
        let mut node = self.node.lock().await;
        let amount_msat = SignerAmount {
            msat: amount.to_msat(),
        };
        let response = node
            .key_send(cln::KeysendRequest {
                destination: destination.serialize().to_vec(),
                amount_msat: Some(amount_msat),
                ..Default::default()
            })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();

        Ok(String::from_utf8(response.payment_hash)?)
    }

    async fn connect_peer(
        &self,
        public_key: PublicKey,
        host: String,
        port: u16,
    ) -> Result<responses::PeerInfo, Error> {
        let mut node = self.node.lock().await;

        let _response = node
            .connect_peer(cln::ConnectRequest {
                id: public_key.to_string(),
                host: Some(host.clone()),
                port: Some(port.into()),
            })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))?
            .into_inner();

        let peer_info = responses::PeerInfo {
            peer_pubkey: public_key,
            host,
            port,
            connected: true,
        };

        Ok(peer_info)
    }

    async fn list_peers(&self) -> Result<Vec<responses::PeerInfo>, Error> {
        let mut node = self.node.lock().await;

        let response = node
            .list_peers(cln::ListpeersRequest {
                ..Default::default()
            })
            .await
            .map_err(|err| Error::TonicError(err.to_string()))
            .unwrap()
            .into_inner();

        let peers = response
            .peers
            .iter()
            .map(|x| from_peer_to_info(x).unwrap())
            .collect();

        Ok(peers)
    }
}

fn from_peer_to_info(peer: &cln::ListpeersPeers) -> Result<responses::PeerInfo, Error> {
    let peer_pubkey = PublicKey::from_slice(&peer.id).unwrap();

    let connected = peer.connected;

    let remote_addr: Vec<String> = peer.clone().netaddr[0]
        .split(':')
        .map(|s| s.to_string())
        .collect();

    let host = remote_addr[0].to_string();
    let port = remote_addr[1].parse::<u16>().unwrap();

    Ok(responses::PeerInfo {
        peer_pubkey,
        host,
        port,
        connected,
    })
}

fn from_list_channels_to_info(
    list_channel: cln::ListpeerchannelsChannels,
) -> Result<responses::ChannelInfo, Error> {
    let remote_balance = list_channel.funding.as_ref().map_or(Amount::ZERO, |a| {
        Amount::from_msat(
            a.remote_funds_msat
                .clone()
                .unwrap_or(SignerAmount { msat: 0 })
                .msat,
        )
    });

    let local_balance = list_channel.clone().funding.map_or(Amount::ZERO, |a| {
        Amount::from_msat(a.local_funds_msat.unwrap_or(SignerAmount { msat: 0 }).msat)
    });

    let is_usable = list_channel
        .state
        // FIXME: Not sure what number is active
        .map(|s| matches!(s, 0))
        .unwrap_or(false);

    let status = if list_channel.state.unwrap_or(0) > 1 {
        ChannelStatus::Active
    } else {
        ChannelStatus::Inactive
    };

    Ok(responses::ChannelInfo {
        peer_pubkey: PublicKey::from_slice(
            &list_channel
                .clone()
                .peer_id
                .ok_or(Error::Custom("No peer id".to_string()))?,
        )?,
        channel_id: hex::encode(list_channel.clone().channel_id()),
        balance: local_balance,
        value: local_balance + remote_balance,
        is_usable,
        status,
    })
}
