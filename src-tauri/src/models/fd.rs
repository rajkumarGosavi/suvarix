use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FdHolding {
    pub id: i64,
    pub account_id: Option<i64>,
    pub bank_name: String,
    pub account_number: Option<String>,
    pub principal: f64,
    pub interest_rate: f64,
    pub compounding: String,
    pub tenure_months: i64,
    pub start_date: String,
    pub maturity_date: String,
    pub maturity_amount: Option<f64>,
    pub is_cumulative: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFdPayload {
    pub account_id: Option<i64>,
    pub bank_name: String,
    pub account_number: Option<String>,
    pub principal: f64,
    pub interest_rate: f64,
    pub compounding: String,
    pub tenure_months: i64,
    pub start_date: String,
    pub maturity_date: String,
    pub maturity_amount: Option<f64>,
    pub is_cumulative: bool,
}
