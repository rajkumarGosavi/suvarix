# Getting Started — Suvarix

Offline-first personal finance tracker for Indian investors. All data stored locally in SQLite — no cloud, no accounts.

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | stable (≥ 1.77) | [rustup.rs](https://rustup.rs) |
| Node.js | ≥ 20 | [nodejs.org](https://nodejs.org) |
| pnpm | ≥ 9 | `npm i -g pnpm` |
| Tauri CLI deps | — | See below |

### Tauri system deps

**Windows** — Install [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++" workload. WebView2 ships with Windows 10/11.

**macOS** — `xcode-select --install`

**Linux** — `sudo apt install libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`

---

## Setup

```bash
# 1. Clone
git clone https://github.com/rajkumarGosavi/suvarix.git
cd suvarix

# 2. Install frontend deps
pnpm install

# 3. Run dev app (starts Vite + Rust backend together)
pnpm tauri dev
```

First run auto-creates DB at:
- **Windows:** `%APPDATA%\com.rajkumar.suvarix\suvarix.db`
- **macOS:** `~/Library/Application Support/com.rajkumar.suvarix/suvarix.db`
- **Linux:** `~/.local/share/com.rajkumar.suvarix/suvarix.db`

All migrations run automatically on startup — no manual DB setup needed.

---

## First Run

App opens to onboarding. Set master password — used to encrypt broker credentials (PBKDF2 + AES-GCM). Password not recoverable. Keep it.

---

## Feature Flags

### Gamification (XP, badges, streaks)

Controlled by two independent flags — both must be enabled together:

**Frontend** (`.env.development` for dev, `.env` for prod):
```
VITE_GAMIFICATION=true   # dev default
VITE_GAMIFICATION=false  # prod default
```

**Backend** — pass feature flag to Cargo:
```bash
# Dev with gamification
pnpm tauri dev -- --features gamification

# Release build with gamification
pnpm tauri build -- --features gamification
```

Without `--features gamification`, `MIGRATION_014` (gamification tables) is skipped and all gamification commands are compiled out.

---

## Commands Reference

```bash
# Dev (full app, HMR on :1420)
pnpm tauri dev

# Frontend only (no Rust backend)
pnpm dev

# Type-check
pnpm build

# Release build
pnpm tauri build

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Rust lint
cargo clippy --manifest-path src-tauri/Cargo.toml

# Frontend tests (Vitest)
pnpm test
pnpm test:watch
```

---

## Architecture in 30 Seconds

```
Vue 3 (src/) ──invoke()──► Tauri IPC ──► Rust (src-tauri/src/) ──► SQLite
```

- Frontend **never** touches DB directly — all reads/writes via `invoke()`
- Rust `#[tauri::command]` fns handle all business logic
- Pinia stores hold reactive state, call `invoke()`, expose to views

Key dirs:

| Path | What |
|---|---|
| `src/views/` | 11 routed pages |
| `src/stores/` | Pinia stores (one per domain) |
| `src/composables/` | Shared logic (`useCurrencyFormat`, `useGamification`, etc.) |
| `src/components/portfolio/` | Per-asset panel components |
| `src-tauri/src/` | Rust modules (one per domain) |
| `src-tauri/src/db/migrations.rs` | All DB schema — `MIGRATION_001` through `MIGRATION_016` |

---

## Adding a DB Migration

1. Add `const MIGRATION_0NN: &str = "..."` in `migrations.rs`
2. Call `conn.execute_batch(MIGRATION_0NN)` in `run_migrations()`
3. **Never edit existing migrations** — they run on every startup against existing DBs

---

## Broker Integrations

| Broker | Method | Notes |
|---|---|---|
| Zerodha | OAuth | Local HTTP server port 7459, 3-min timeout |
| Upstox | OAuth | Similar flow |
| Angel One | API key | Stored encrypted in DB |
| MF Central | PDF import | Parse holdings PDF |
| Groww | CSV import | Holdings CSV export |

Credentials stored AES-GCM encrypted in SQLite, never transmitted.

---

## Common Pitfalls

- **`pnpm dev` alone** — frontend only, no Rust backend, `invoke()` calls fail
- **Missing C++ build tools on Windows** — Rust compilation fails with linker errors
- **Gamification widget shows nothing** — `VITE_GAMIFICATION=true` set but `--features gamification` not passed to Cargo (or vice versa)
- **UI components not found** — PrimeVue uses auto-import; never manually import PrimeVue components
