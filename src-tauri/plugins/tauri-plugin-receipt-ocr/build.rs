const COMMANDS: &[&str] = &["scan_receipt", "is_supported"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
