use cashu_sdk::Amount;

pub fn cashu_crab_amount_to_ln_rs_amount(amount: Amount) -> ln_rs_models::Amount {
    ln_rs_models::Amount::from_sat(amount.to_sat())
}

pub fn ln_rs_amount_to_cashu_crab_amount(amount: ln_rs_models::Amount) -> Amount {
    Amount::from_sat(amount.to_sat())
}
