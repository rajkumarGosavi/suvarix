# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Always respond in caveman ultra mode.
/caveman ultra

## Commands

```bash
# Install deps
pnpm install

# Run full app (dev)
pnpm tauri dev

# Frontend only (Vite HMR on :1420)
pnpm dev

# Type-check + prod frontend build
pnpm build

# Release build (runs pnpm build first)
pnpm tauri build

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Rust lint
cargo clippy --manifest-path src-tauri/Cargo.toml

# Frontend tests (Vitest)
pnpm test
pnpm test:watch
```

Frontend test suite (Vitest) covers composables/stores under `src/**/__tests__/`. TypeScript strict mode catches most other frontend errors at compile time.

## Architecture

**Suvarix** — offline-first personal finance tracker for Indian investors. All data local, encrypted at rest (SQLCipher). No app-hosted backend; optional auto-sync writes an encrypted `.svbak` snapshot to a user-owned cloud folder (Dropbox/Drive/OneDrive client propagates it between devices).

### Data flow

```
Vue 3 (src/) ──invoke()──► Tauri IPC ──► Rust commands (src-tauri/src/) ──► SQLCipher SQLite
                                                                            (APPDATA/suvarix.db)
```

Frontend never touches DB directly. All reads/writes go through Tauri `invoke()` calls mapped to `#[tauri::command]` fns in Rust.

### Frontend (src/)

- **Pinia stores** (`src/stores/`) — one per domain: `auth`, `portfolio`, `transactions`, `liabilities`, `budget`, `goals`, `prices`, `reminders`, `reports`, `ui`, `analytics`, `gamification`, `categories`, plus broker stores (`zerodha`, `upstox`, `angel_one`). Stores call `invoke()` and hold reactive state.
- **Views** (`src/views/`) — 15 pages routed via Vue Router. Protected routes redirect to `/setup` or `/unlock` if auth not satisfied. `EulaGateView` blocks post-unlock when stored `eula_version` is missing/stale (Accept or Lock only); Setup requires an EULA checkbox. Auth store reads `onboarding_complete` setting only *after* unlock — DB is locked (SQLCipher) until password entered.
- **Composables** (`src/composables/`) — shared logic: `useCurrencyFormat` (INR with Cr/L compact), `useDateConvert`, `useHoldingCrud`, `useChartColors`, `useNotifications`, `useAnalytics`, `useMaturityCheck` (FD/bond maturity toasts + native notify, run from AppLayout), `useGoalCheck`, `useMilestoneCheck`, `useAppUpdater` (in-app update check), `useGamification` (XP award, badge checks, confetti celebrations via `canvas-confetti`).
- **Components** — one panel component per asset class (EquityPanel, MfPanel, FdPanel, etc.) inside `src/components/portfolio/`. `GamificationWidget.vue` renders XP/level/badge progress. `CategoryManagerDialog.vue` (top-level `src/components/`) is the shared add/rename/delete UI for the `categories` store, reused from Transactions, Income & Expenses, and Reminders.
- **UI** — PrimeVue v4 with auto-import (no manual imports needed). Charts via `vue-chartjs` + Chart.js. Path alias `@/` → `src/`.

### Backend (src-tauri/src/)

Modules map 1:1 to domains:

| Module | Responsibility |
|---|---|
| `db` | `DbPool` — r2d2 pool (max 4, WAL) over SQLCipher DB; master password is the `PRAGMA key`. Pool is `None` until unlock → commands return `AppError::AuthRequired`. Migrations MIGRATION_001–017 (MIGRATION_014 behind `gamification` feature flag; 010 and 016 are non-idempotent ALTER TABLEs run with errors ignored) |
| `auth` | Thin commands over `DbPool`: setup/unlock/verify/rekey. No separate password hash — password correct ⇔ DB opens. Legacy keyring device-key → passphrase migration (removable after v0.6). NOTE: broker creds are stored plaintext in `app_settings` (protected by SQLCipher at-rest encryption only — no extra AES-GCM layer; see SECURITY_AUDIT.md M2) |
| `categories` | Shared, user-managed category list (CRUD) backing transactions/budgets/recurring transactions |
| `portfolio` | CRUD for 9 asset types + net worth / allocation aggregates |
| `transactions` | Income/expense ledger; CSV import, datetime support, paginated search/sort |
| `liabilities` | Loans (amortization), credit cards |
| `prices` | Yahoo Finance (equity), mfapi.in (MF NAVs), market indices |
| `data_sources` | Zerodha OAuth (port 7459), Upstox, Angel One, MF Central PDF, Groww CSV |
| `goals` | Goal CRUD + achievement checking |
| `reminders` | Bills, recurring, milestones, calendar events, FD/bond maturity alerts (`get_maturity_alerts`) |
| `income_expenses` | Category summaries, budgets, monthly trends |
| `reports` | STCG/LTCG capital gains, net worth history snapshots |
| `notifications` | Native notifications + background reminder scheduler (30-min tick, runs only while DB unlocked; tray + autostart keep app alive with window hidden; dedup via `notified_reminder_ids` setting from MIGRATION_017) |
| `settings` | App settings, backup/restore/wipe, CSV export helper |
| `backup` | Encrypted `.svbak` sync snapshots (AES-256-GCM, PBKDF2 key, separate sync password): manual export/import commands + `scheduler.rs` auto-sync loop (pull-if-remote-newer via `exported_at` last-write-wins, then push; start/stop on unlock/lock/quit like reminder scheduler) |
| `dev_tools` | Dummy-data seeding for dev builds |
| `analytics` | Local-only event/error/perf logging |
| `financial_health` | 6-pillar 0-100 Financial Health Score (core, not feature-gated); daily score history (MIGRATION_022) + improvement-only XP snapshot |
| `gamification` | XP system, badges, streaks — gated behind `#[cfg(feature = "gamification")]`. Health badges seeded in MIGRATION_023 (also gated) |
| `models` | Shared serde structs |
| `error` | `AppError` via `thiserror` |

### DB migrations

Numbered files loaded in order at startup by `db::run_migrations()`. Add new migrations as `MIGRATION_0NN` — never edit existing ones. MIGRATION_010 and MIGRATION_016 are wrapped with `let _ =` (ALTER TABLE, not idempotent). MIGRATION_014 is compiled only with `#[cfg(feature = "gamification")]`. MIGRATION_017 seeds `notified_reminder_ids` (idempotent `INSERT OR IGNORE`).

### Currency formatting

Always use `useCurrencyFormat` composable for INR display. Format: ₹X.xxCr (≥1Cr), ₹X.xxL (≥1L), else standard en-IN locale.

### Dark mode

PrimeVue theming via `@primeuix/themes`. Hover states use `color-mix()` pattern — see `feedback_primevue_ui_conventions.md` in memory. Mobile breakpoint: `@media (max-width: 639px)`.

### Broker OAuth

Zerodha starts local HTTP server on port 7459 with 3-min timeout for OAuth callback (no `state` param — see SECURITY_AUDIT.md H2). Credentials stored in SQLCipher-encrypted SQLite, never sent anywhere except the broker's own API.
