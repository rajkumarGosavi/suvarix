use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MfHolding {
    pub id: i64,
    pub account_id: i64,
    pub scheme_code: String,
    pub scheme_name: String,
    pub amc_name: String,
    pub folio_number: String,
    pub units: f64,
    pub avg_nav: f64,
    pub current_nav: Option<f64>,
    pub nav_date: Option<String>,
    pub is_direct: bool,
    pub is_growth: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMfPayload {
    pub account_id: i64,
    pub scheme_code: String,
    pub scheme_name: String,
    pub amc_name: String,
    pub folio_number: String,
    pub units: f64,
    pub avg_nav: f64,
    pub is_direct: bool,
    pub is_growth: bool,
}
