# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Always respond in caveman ultra mode.
/caveman ultra

## Commands

```bash
# Install deps
pnpm install

# Run full app (dev)
cargo tauri dev

# Frontend only (Vite HMR on :1420)
pnpm dev

# Type-check + prod frontend build
pnpm build

# Release build (runs pnpm build first)
cargo tauri build

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Rust lint
cargo clippy --manifest-path src-tauri/Cargo.toml
```

No frontend test suite. TypeScript strict mode catches most frontend errors at compile time.

## Architecture

**Suvarix** — offline-first personal finance tracker for Indian investors. All data local (SQLite). No cloud.

### Data flow

```
Vue 3 (src/) ──invoke()──► Tauri IPC ──► Rust commands (src-tauri/src/) ──► SQLite
                                                                            (APPDATA/suvarix.db)
```

Frontend never touches DB directly. All reads/writes go through Tauri `invoke()` calls mapped to `#[tauri::command]` fns in Rust.

### Frontend (src/)

- **Pinia stores** (`src/stores/`) — one per domain: `auth`, `portfolio`, `transactions`, `liabilities`, `budget`, `goals`, `prices`, `reminders`, `reports`, `ui`, `analytics`, plus broker stores (`zerodha`, `upstox`, `angel_one`). Stores call `invoke()` and hold reactive state.
- **Views** (`src/views/`) — 11 pages routed via Vue Router. Protected routes redirect to `/setup` or `/unlock` if auth not satisfied.
- **Composables** (`src/composables/`) — shared logic: `useCurrencyFormat` (INR with Cr/L compact), `useDateConvert`, `useHoldingCrud`, `useChartColors`, `useNotifications`, `useAnalytics`.
- **Components** — one panel component per asset class (EquityPanel, MfPanel, FdPanel, etc.) inside `src/components/portfolio/`.
- **UI** — PrimeVue v4 with auto-import (no manual imports needed). Charts via `vue-chartjs` + Chart.js. Path alias `@/` → `src/`.

### Backend (src-tauri/src/)

Modules map 1:1 to domains:

| Module | Responsibility |
|---|---|
| `db` | SQLite pool init, WAL mode, 11 migrations (MIGRATION_001–010) |
| `auth` | Master password (PBKDF2 + salt), AES-GCM encryption for broker creds |
| `portfolio` | CRUD for 9 asset types + net worth / allocation aggregates |
| `transactions` | Income/expense ledger |
| `liabilities` | Loans (amortization), credit cards |
| `prices` | Yahoo Finance (equity), mfapi.in (MF NAVs), market indices |
| `data_sources` | Zerodha OAuth (port 7459), Upstox, Angel One, MF Central PDF, Groww CSV |
| `goals` | Goal CRUD + achievement checking |
| `reminders` | Bills, recurring, milestones, calendar events |
| `income_expenses` | Category summaries, budgets, monthly trends |
| `reports` | STCG/LTCG capital gains, net worth history snapshots |
| `notifications` | Tauri toast + reminder scheduler |
| `settings` | App settings, backup/restore/wipe |
| `analytics` | Local-only event/error/perf logging |
| `models` | Shared serde structs |
| `error` | `AppError` via `thiserror` |

### DB migrations

Numbered files loaded in order at startup by `db::run_migrations()`. Add new migrations as `MIGRATION_0NN` — never edit existing ones.

### Currency formatting

Always use `useCurrencyFormat` composable for INR display. Format: ₹X.xxCr (≥1Cr), ₹X.xxL (≥1L), else standard en-IN locale.

### Dark mode

PrimeVue theming via `@primeuix/themes`. Hover states use `color-mix()` pattern — see `feedback_primevue_ui_conventions.md` in memory. Mobile breakpoint: `@media (max-width: 639px)`.

### Broker OAuth

Zerodha starts local HTTP server on port 7459 with 3-min timeout for OAuth callback. Credentials stored encrypted (AES-GCM) in SQLite, never transmitted.
