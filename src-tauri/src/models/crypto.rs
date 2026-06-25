use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CryptoHolding {
    pub id: i64,
    pub account_id: Option<i64>,
    pub exchange_name: String,
    pub coin_symbol: String,
    pub quantity: f64,
    pub avg_buy_price: f64,
    pub current_price: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCryptoPayload {
    pub account_id: Option<i64>,
    pub exchange_name: String,
    pub coin_symbol: String,
    pub quantity: f64,
    pub avg_buy_price: f64,
}
