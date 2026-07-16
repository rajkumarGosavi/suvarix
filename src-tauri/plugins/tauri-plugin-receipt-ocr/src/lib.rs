use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;
mod error;
mod models;

#[cfg(not(target_os = "android"))]
mod desktop;
#[cfg(target_os = "android")]
mod mobile;

pub use error::{Error, Result};
pub use models::*;

#[cfg(not(target_os = "android"))]
use desktop::ReceiptOcr;
#[cfg(target_os = "android")]
use mobile::ReceiptOcr;

/// Access the receipt-ocr APIs from any `Manager` (AppHandle, Window, …).
pub trait ReceiptOcrExt<R: Runtime> {
    fn receipt_ocr(&self) -> &ReceiptOcr<R>;
}

impl<R: Runtime, T: Manager<R>> ReceiptOcrExt<R> for T {
    fn receipt_ocr(&self) -> &ReceiptOcr<R> {
        self.state::<ReceiptOcr<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("receipt-ocr")
        .invoke_handler(tauri::generate_handler![
            commands::scan_receipt,
            commands::is_supported
        ])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let receipt_ocr = mobile::init(app, api)?;
            #[cfg(not(target_os = "android"))]
            let receipt_ocr = desktop::init(app, api)?;
            app.manage(receipt_ocr);
            Ok(())
        })
        .build()
}
