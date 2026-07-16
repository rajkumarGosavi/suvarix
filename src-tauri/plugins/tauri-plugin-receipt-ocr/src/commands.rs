use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::ReceiptOcrExt;
use crate::Result;

#[command]
pub(crate) async fn scan_receipt<R: Runtime>(
    app: AppHandle<R>,
    source: String,
) -> Result<ScanResult> {
    app.receipt_ocr().scan_receipt(ScanRequest { source })
}

#[command]
pub(crate) async fn is_supported<R: Runtime>(app: AppHandle<R>) -> Result<bool> {
    Ok(app.receipt_ocr().is_supported())
}
