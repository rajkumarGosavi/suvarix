use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BondHolding {
    pub id: i64,
    pub account_id: Option<i64>,
    pub isin: Option<String>,
    pub issuer_name: String,
    pub bond_type: String,
    pub face_value: f64,
    pub quantity: f64,
    pub purchase_price: f64,
    pub current_price: Option<f64>,
    pub coupon_rate: f64,
    pub coupon_frequency: String,
    pub purchase_date: String,
    pub maturity_date: Option<String>,
    pub credit_rating: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBondPayload {
    pub account_id: Option<i64>,
    pub isin: Option<String>,
    pub issuer_name: String,
    pub bond_type: String,
    pub face_value: f64,
    pub quantity: f64,
    pub purchase_price: f64,
    pub current_price: Option<f64>,
    pub coupon_rate: f64,
    pub coupon_frequency: String,
    pub purchase_date: String,
    pub maturity_date: Option<String>,
    pub credit_rating: Option<String>,
}
