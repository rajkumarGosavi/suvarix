//! Wires up `tracing` output so `tracing::debug!`/`info!`/`warn!` calls
//! throughout the app actually go somewhere — previously `tracing-subscriber`
//! was a dependency but never initialized, so every trace call anywhere in
//! the codebase was a silent no-op on every platform. Debug level only fires
//! in debug builds (`cfg!(debug_assertions)`); release builds cap at `INFO`
//! so verbose per-row/per-tick tracing doesn't run in production at all —
//! call sites can log liberally without worrying about release-build cost.
//!
//! Desktop: plain `tracing_subscriber::fmt` to stdout (visible in `pnpm tauri
//! dev`'s terminal). Android: bridged to logcat via `paranoid-android` (NDK
//! `__android_log_write`), viewable with `adb logcat -s suvarix`.

const LOG_TAG: &str = "suvarix";

fn max_level() -> tracing::Level {
    if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    }
}

#[cfg(target_os = "android")]
pub fn init() {
    use tracing_subscriber::prelude::*;

    let android_layer = paranoid_android::layer(LOG_TAG)
        .with_filter(tracing_subscriber::filter::LevelFilter::from_level(max_level()));
    tracing_subscriber::registry().with(android_layer).init();
}

#[cfg(not(target_os = "android"))]
pub fn init() {
    tracing_subscriber::fmt()
        .with_max_level(max_level())
        .with_target(true)
        .init();
}
