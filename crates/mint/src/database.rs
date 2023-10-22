use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Result};
use cashu_sdk::nuts::nut00::Proofs;
use cashu_sdk::nuts::nut02::Id;
use cashu_sdk::secret::Secret;
use cashu_sdk::{Amount, Sha256};
use ln_rs::InvoiceInfo;
use redb::{Database, ReadableTable, TableDefinition};
use tokio::sync::Mutex;

use crate::types::KeysetInfo;

// Key: KeysetId
// Value: Keyset
const KEYSETS: TableDefinition<&str, &str> = TableDefinition::new("keysets");

const CONFIG: TableDefinition<&str, &str> = TableDefinition::new("config");

// Config Keys
const IN_CIRCULATION: &str = "in_circulation";

// Key: Random Hash
// Value Serialized Invoice Info
const INVOICES: TableDefinition<&str, &str> = TableDefinition::new("invoices");

// Key: Payment Hash
// Value: Random Hash
const HASH: TableDefinition<&str, &str> = TableDefinition::new("hash");

// KEY: Secret
// VALUE: serialized proof
const USED_PROOFS: TableDefinition<&str, &str> = TableDefinition::new("used_proofs");

#[derive(Debug, Clone)]
pub struct Db {
    db: Arc<Mutex<Database>>,
}

impl Db {
    /// Init Database
    pub async fn new(path: PathBuf) -> Result<Self> {
        if let Err(_err) = fs::create_dir_all(&path) {}
        let db_path = path.join("cashu-mint-rs.redb");
        let database = Database::create(db_path)?;

        let write_txn = database.begin_write()?;
        {
            let _ = write_txn.open_table(KEYSETS)?;
            let _ = write_txn.open_table(CONFIG)?;
            let _ = write_txn.open_table(INVOICES)?;
            let _ = write_txn.open_table(HASH)?;
            let _ = write_txn.open_table(USED_PROOFS)?;
        }
        write_txn.commit()?;

        Ok(Self {
            db: Arc::new(Mutex::new(database)),
        })
    }

    pub async fn add_keyset(&self, keyset_info: &KeysetInfo) -> Result<()> {
        let db = self.db.lock().await;

        let write_txn = db.begin_write()?;
        {
            let mut keysets_table = write_txn.open_table(KEYSETS)?;

            keysets_table.insert(
                keyset_info.id.to_string().as_str(),
                keyset_info.as_json()?.as_str(),
            )?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn get_all_keyset_info(&self) -> Result<HashMap<String, KeysetInfo>> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;
        let keysets_table = read_txn.open_table(KEYSETS)?;

        let keysets = keysets_table
            .iter()?
            .flatten()
            .map(|(k, v)| {
                (
                    k.value().to_string(),
                    serde_json::from_str(v.value()).unwrap(),
                )
            })
            .collect();

        Ok(keysets)
    }

    pub async fn _get_keyset_info(&self, keyset_id: &str) -> Result<KeysetInfo> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;
        let keysets_table = read_txn.open_table(KEYSETS)?;

        let keyset_info = match keysets_table.get(keyset_id.to_string().as_str())? {
            Some(contact) => serde_json::from_str(contact.value())?,
            None => bail!("Keyset Not Found"),
        };

        Ok(keyset_info)
    }

    pub async fn set_active_keyset(&self, keyset_id: &Id) -> Result<()> {
        let db = self.db.lock().await;

        let write_txn = db.begin_write()?;
        {
            let mut config_table = write_txn.open_table(CONFIG)?;

            config_table.insert("active_keyset", keyset_id.to_string().as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn _get_active_keyset(&self) -> Result<Option<String>> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;
        let keysets_table = read_txn.open_table(CONFIG)?;

        let keyset_info = keysets_table
            .get("active_keyset")?
            .map(|k| k.value().to_string());

        Ok(keyset_info)
    }

    pub async fn set_last_pay_index(&self, last_pay_index: u64) -> Result<()> {
        let db = self.db.lock().await;

        let write_txn = db.begin_write()?;
        {
            let mut config_table = write_txn.open_table(CONFIG)?;

            config_table.insert("last_pay_index", last_pay_index.to_string().as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn get_last_pay_index(&self) -> Result<u64> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;
        let config_table = read_txn.open_table(CONFIG)?;

        let last_pay_index = match config_table.get("last_pay_index")? {
            Some(contact) => contact.value().parse()?,
            None => 0,
        };

        Ok(last_pay_index)
    }

    pub async fn set_in_circulation(&self, amount: &Amount) -> Result<()> {
        let db = self.db.lock().await;

        let write_txn = db.begin_write()?;
        {
            let mut config_table = write_txn.open_table(CONFIG)?;

            config_table.insert(IN_CIRCULATION, serde_json::to_string(amount)?.as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn get_in_circulation(&self) -> Result<Amount> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;
        let config_table = read_txn.open_table(CONFIG)?;

        let last_pay_index = match config_table.get(IN_CIRCULATION)? {
            Some(contact) => serde_json::from_str(contact.value())?,
            None => Amount::ZERO,
        };

        Ok(last_pay_index)
    }

    pub async fn add_invoice(&self, invoice_info: &InvoiceInfo) -> Result<()> {
        let db = self.db.lock().await;

        let write_txn = db.begin_write()?;
        {
            let mut invoices_table = write_txn.open_table(INVOICES)?;

            invoices_table.insert(
                invoice_info.hash.to_string().as_str(),
                invoice_info.as_json()?.as_str(),
            )?;

            let mut hash_table = write_txn.open_table(HASH)?;
            hash_table.insert(
                invoice_info.payment_hash.to_string().as_str(),
                invoice_info.hash.to_string().as_str(),
            )?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn get_invoice_info(&self, hash: &Sha256) -> Result<InvoiceInfo> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;
        let invoices_table = read_txn.open_table(INVOICES)?;

        let invoices_info = match invoices_table.get(hash.to_string().as_str())? {
            Some(invoice) => serde_json::from_str(invoice.value())?,
            None => bail!("Invoice Not Found"),
        };

        Ok(invoices_info)
    }

    pub async fn get_invoice_info_by_payment_hash(
        &self,
        payment_hash: &Sha256,
    ) -> Result<InvoiceInfo> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;

        let hash_table = read_txn.open_table(HASH)?;

        let hash = match hash_table.get(payment_hash.to_string().as_str())? {
            Some(hash) => {
                let hash = hash.value();
                hash.to_string()
            }
            None => bail!("Hash Mapping not found"),
        };

        let invoices_table = read_txn.open_table(INVOICES)?;

        let invoices_info = match invoices_table.get(hash.as_str())? {
            Some(invoice) => serde_json::from_str(invoice.value())?,
            None => bail!("Invoice Not Found"),
        };

        Ok(invoices_info)
    }

    /*
        pub async fn get_pending_invoices(&self) -> Result<HashMap<Sha256, InvoiceInfo>> {
            let db = self.db.lock().await;

            let read_txn = db.begin_read()?;

            let invoice_table = read_txn.open_table(HASH)?;

            let pending_invoices = invoice_table
                .iter()?
                .flatten()
                .filter_map(|(k, v)| {
                    let invoice: Result<InvoiceInfo, _> = serde_json::from_str(v.value());
                    invoice
                        .ok()
                        .filter(|invoice| invoice.status != InvoiceStatus::Unpaid)
                        .map(|invoice| (Sha256::from_str(k.value()).unwrap(), invoice))
                })
                .collect::<HashMap<Sha256, InvoiceInfo>>();

            Ok(pending_invoices)
        }

        pub async fn get_unissued_invoices(&self) -> Result<HashMap<Sha256, InvoiceInfo>> {
            let db = self.db.lock().await;

            let read_txn = db.begin_read()?;

            let invoice_table = read_txn.open_table(HASH)?;

            let pending_invoices = invoice_table
                .iter()?
                .flatten()
                .filter_map(|(k, v)| {
                    let invoice: Result<InvoiceInfo, _> = serde_json::from_str(v.value());
                    invoice
                        .ok()
                        .filter(|invoice| {
                            invoice.status.eq(&InvoiceStatus::Paid)
                                && invoice.token_status.eq(&InvoiceTokenStatus::NotIssued)
                        })
                        .map(|invoice| (Sha256::from_str(k.value()).unwrap(), invoice))
                })
                .collect::<HashMap<Sha256, InvoiceInfo>>();

            Ok(pending_invoices)
        }
    */
    pub async fn add_used_proofs(&self, proofs: &Proofs) -> Result<()> {
        let db = self.db.lock().await;

        let write_txn = db.begin_write()?;
        {
            let mut used_proof_table = write_txn.open_table(USED_PROOFS)?;

            for proof in proofs {
                used_proof_table.insert(
                    proof.secret.to_string().as_str(),
                    serde_json::to_string(proof)?.as_str(),
                )?;
            }
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn get_spent_secrets(&self) -> Result<HashSet<Secret>> {
        let db = self.db.lock().await;

        let read_txn = db.begin_read()?;

        let used_proofs_table = read_txn.open_table(USED_PROOFS)?;

        let used_proofs = used_proofs_table
            .iter()?
            .flatten()
            .flat_map(|(k, _v)| Secret::from_str(k.value()))
            .collect();
        Ok(used_proofs)
    }
}
