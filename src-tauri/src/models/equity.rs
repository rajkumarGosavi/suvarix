use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EquityHolding {
    pub id: i64,
    pub account_id: i64,
    pub isin: String,
    pub symbol: String,
    pub exchange: String,
    pub name: String,
    pub quantity: f64,
    pub avg_buy_price: f64,
    pub current_price: Option<f64>,
    pub price_updated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddEquityPayload {
    pub account_id: i64,
    pub isin: String,
    pub symbol: String,
    pub exchange: String,
    pub name: String,
    pub quantity: f64,
    pub avg_buy_price: f64,
}
