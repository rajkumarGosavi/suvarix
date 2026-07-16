use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

const PLUGIN_IDENTIFIER: &str = "com.plugin.receiptocr";

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<ReceiptOcr<R>> {
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "ReceiptOcrPlugin")?;
    Ok(ReceiptOcr(handle))
}

/// Android implementation — camera/gallery intent + ML Kit OCR in Kotlin.
pub struct ReceiptOcr<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> ReceiptOcr<R> {
    pub fn scan_receipt(&self, payload: ScanRequest) -> crate::Result<ScanResult> {
        self.0
            .run_mobile_plugin("scanReceipt", payload)
            .map_err(Into::into)
    }

    pub fn is_supported(&self) -> bool {
        true
    }
}
