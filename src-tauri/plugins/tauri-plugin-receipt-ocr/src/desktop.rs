use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;
use crate::Error;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<ReceiptOcr<R>> {
    // AppHandle kept (not PhantomData) so ReceiptOcr is Send + Sync, as
    // required by Manager::state(), regardless of R's own bounds.
    Ok(ReceiptOcr(app.clone()))
}

/// Desktop stub — receipt scanning is Android-only; UI gates on `is_supported`.
pub struct ReceiptOcr<R: Runtime>(#[allow(dead_code)] AppHandle<R>);

impl<R: Runtime> ReceiptOcr<R> {
    pub fn scan_receipt(&self, _payload: ScanRequest) -> crate::Result<ScanResult> {
        Err(Error::NotSupported)
    }

    pub fn is_supported(&self) -> bool {
        false
    }
}
