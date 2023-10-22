use std::path::PathBuf;
use std::time::SystemTime;

pub fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|x| x.as_secs())
        .unwrap_or(0)
}

pub fn expand_path(path: &str) -> Option<PathBuf> {
    if path.starts_with('~') {
        if let Some(home_dir) = dirs::home_dir().as_mut() {
            let remainder = &path[2..];
            home_dir.push(remainder);
            let expanded_path = home_dir;
            Some(expanded_path.clone())
        } else {
            None
        }
    } else {
        Some(PathBuf::from(path))
    }
}

pub fn cashu_crab_amount_to_ln_rs_amount(amount: cashu_sdk::Amount) -> ln_rs::Amount {
    ln_rs::Amount::from_sat(amount.to_sat())
}

pub fn ln_rs_amount_to_cashu_crab_amount(amount: ln_rs::Amount) -> cashu_sdk::Amount {
    cashu_sdk::Amount::from_sat(amount.to_sat())
}
