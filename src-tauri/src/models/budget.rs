use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    pub id: i64,
    pub category: String,
    pub monthly_limit: f64,
    pub period: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetStatus {
    pub category: String,
    pub monthly_limit: f64,
    pub spent: f64,
    pub remaining: f64,
    pub percent_used: f64,
}
