use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub r#type: String,
    pub provider: Option<String>,
    pub external_id: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}
