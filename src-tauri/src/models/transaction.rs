use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: i64,
    pub date: String,
    pub r#type: String,
    pub asset_class: Option<String>,
    pub account_id: Option<i64>,
    pub holding_id: Option<i64>,
    pub amount: f64,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub source: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTransactionPayload {
    pub date: String,
    pub r#type: String,
    pub asset_class: Option<String>,
    pub account_id: Option<i64>,
    pub holding_id: Option<i64>,
    pub amount: f64,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFilter {
    pub r#type: Option<String>,
    pub asset_class: Option<String>,
    pub account_id: Option<i64>,
    pub category: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    /// Free-text match against description OR category (case-insensitive substring).
    pub search: Option<String>,
    /// "date" (default) or "amount".
    pub sort_by: Option<String>,
    /// "asc" or "desc" (default "desc").
    pub sort_dir: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
