use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub imported: i64,
    pub skipped: i64,
}
