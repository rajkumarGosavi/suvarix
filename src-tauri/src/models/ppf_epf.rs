use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PpfEpfHolding {
    pub id: i64,
    pub account_type: String,
    pub account_number: Option<String>,
    pub balance: f64,
    pub interest_rate: f64,
    pub financial_year: Option<String>,
    pub employer_contrib: Option<f64>,
    pub employee_contrib: Option<f64>,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPpfEpfPayload {
    pub account_type: String,
    pub account_number: Option<String>,
    pub balance: f64,
    pub interest_rate: f64,
    pub financial_year: Option<String>,
    pub employer_contrib: Option<f64>,
    pub employee_contrib: Option<f64>,
}
