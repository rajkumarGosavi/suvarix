use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GoldHolding {
    pub id: i64,
    pub gold_type: String,
    pub name: Option<String>,
    pub weight_grams: Option<f64>,
    pub purity: Option<String>,
    pub units: Option<f64>,
    pub avg_buy_price: f64,
    pub current_price: Option<f64>,
    pub account_id: Option<i64>,
    pub maturity_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddGoldPayload {
    pub gold_type: String,
    pub name: Option<String>,
    pub weight_grams: Option<f64>,
    pub purity: Option<String>,
    pub units: Option<f64>,
    pub avg_buy_price: f64,
    pub account_id: Option<i64>,
    pub maturity_date: Option<String>,
}
