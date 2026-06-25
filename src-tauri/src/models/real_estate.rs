use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateHolding {
    pub id: i64,
    pub property_name: String,
    pub property_type: String,
    pub location: Option<String>,
    pub purchase_price: f64,
    pub purchase_date: String,
    pub current_value: Option<f64>,
    pub rental_income: Option<f64>,
    pub has_mortgage: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRealEstatePayload {
    pub property_name: String,
    pub property_type: String,
    pub location: Option<String>,
    pub purchase_price: f64,
    pub purchase_date: String,
    pub current_value: Option<f64>,
    pub rental_income: Option<f64>,
    pub has_mortgage: bool,
}
