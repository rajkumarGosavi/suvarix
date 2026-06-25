use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InsuranceHolding {
    pub id: i64,
    pub insurance_type: String,
    pub provider: String,
    pub policy_number: Option<String>,
    pub premium_amount: f64,
    pub premium_freq: String,
    pub coverage_amount: Option<f64>,
    pub maturity_value: Option<f64>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub next_due_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddInsurancePayload {
    pub insurance_type: String,
    pub provider: String,
    pub policy_number: Option<String>,
    pub premium_amount: f64,
    pub premium_freq: String,
    pub coverage_amount: Option<f64>,
    pub maturity_value: Option<f64>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub next_due_date: Option<String>,
}
