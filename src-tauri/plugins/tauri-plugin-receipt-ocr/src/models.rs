use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    /// "camera" or "gallery"
    pub source: String,
}

/// One recognized text line, ordered top-to-bottom by its bounding box.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLine {
    pub text: String,
    pub top: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// true when the user backed out of the camera/gallery without an image.
    pub cancelled: bool,
    pub full_text: String,
    pub lines: Vec<OcrLine>,
}
