use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Loan {
    pub id: i64,
    pub loan_type: String,
    pub lender_name: String,
    pub account_number: Option<String>,
    pub principal: f64,
    pub outstanding: f64,
    pub interest_rate: f64,
    pub emi_amount: f64,
    pub tenure_months: i64,
    pub disbursement_date: String,
    pub next_emi_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLoanPayload {
    pub loan_type: String,
    pub lender_name: String,
    pub account_number: Option<String>,
    pub principal: f64,
    pub outstanding: f64,
    pub interest_rate: f64,
    pub emi_amount: f64,
    pub tenure_months: i64,
    pub disbursement_date: String,
    pub next_emi_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreditCard {
    pub id: i64,
    pub bank_name: String,
    pub card_name: Option<String>,
    pub last_four: Option<String>,
    pub credit_limit: f64,
    pub current_balance: f64,
    pub due_date: Option<i64>,
    pub min_payment: Option<f64>,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCreditCardPayload {
    pub bank_name: String,
    pub card_name: Option<String>,
    pub last_four: Option<String>,
    pub credit_limit: f64,
    pub current_balance: f64,
    pub due_date: Option<i64>,
    pub min_payment: Option<f64>,
}
