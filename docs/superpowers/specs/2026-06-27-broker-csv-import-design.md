# Broker CSV Import — Design Spec

**Feature:** CSV import of equity holdings for Zerodha, Upstox, Angel One  
**Status:** Approved

---

## Context

Suvarix already supports:
- Zerodha / Upstox — OAuth API sync (desktop only)
- Angel One — SmartAPI JWT login + sync
- Groww — CSV import (frontend parse → Rust upsert)
- MF Central — PDF CAS import

Two user needs not yet covered:
1. Users without Kite Connect / Upstox / SmartAPI credentials who want to import broker holdings
2. Users importing historical holdings exported before connecting OAuth

---

## Scope

CSV import for **Zerodha, Upstox, Angel One** equity holdings only. No MF, no FD, no transactions. Import overwrites all holdings for that broker account (same behavior as OAuth sync).

---

## Architecture

### Backend — one shared command replacing `import_groww_csv`

File: `src-tauri/src/data_sources/commands.rs`

`GrowwRow` renamed → `BrokerCsvRow` (fields unchanged). `import_groww_csv` replaced by:

```rust
#[tauri::command]
pub fn import_broker_equity_csv(
    broker: String,         // "zerodha" | "upstox" | "angel_one" | "groww"
    display_name: String,   // "Zerodha" | "Upstox" | "Angel One" | "Groww"
    rows: Vec<BrokerCsvRow>,
    state: State<DbState>,
) -> Result<ImportResult>
```

- Converts `Vec<BrokerCsvRow>` → `Vec<BrokerHolding>`
- Calls existing `write_broker_holdings(&mut conn, &broker, &display_name, &holdings)`
- Returns `ImportResult { imported, skipped }`
- No new DB schema, no new migrations

Register in `src-tauri/src/lib.rs`, remove `import_groww_csv`. Adding broker 5 = add frontend parser only, zero Rust changes.

### Frontend — CSV parsers

Three parse functions in `src/views/DataSourcesView.vue`, each following the Groww `parseGrowwCsv` pattern: split lines → normalize headers → map column indices → filter invalid rows.

**Column mapping per broker:**

| Field | Zerodha | Upstox | Angel One |
|---|---|---|---|
| symbol | `instrument`, `symbol` | `instrument name`, `trading symbol` | `symbol`, `instrument` |
| isin | `isin` | `isin` | `isin` |
| quantity | `qty`, `quantity` | `quantity`, `net qty` | `net qty`, `qty` |
| avgPrice | `avg cost`, `average price`, `avg price` | `avg. cost price`, `buy avg price` | `avg buy price`, `average buy price` |
| ltp | `ltp`, `last price` | `ltp`, `current price` | `ltp`, `cmp` |
| exchange | `exchange`, `segment`, `exch` | `exchange`, `exch` | `exchange`, `exch` |

All parse fns filter rows where `quantity <= 0 || avgPrice <= 0`.

### Frontend — UI

Location: `src/views/DataSourcesView.vue`, within each broker's existing `.zerodha-card`.

Each broker card gets a collapsible **"Or import from CSV"** section at the bottom, always shown regardless of OAuth state. Pattern:

```
[ existing OAuth content ]
─────────────────────────
▸ Or import from CSV
  [when expanded]
  Instructions: "Go to Zerodha Console → Portfolio → Download"
  FileUpload (.csv, max 5MB)
  DataTable preview (5 rows: Symbol | ISIN | Qty | Avg Price | Exchange)
  Message (error)
  Button "Import N Holdings"  [shown after valid parse]
  Tag "✓ N holdings imported" / "N skipped" [shown after success]
```

**State per broker** (3× sets, one per broker):

```ts
const zerodhaRows = ref<BrokerCsvRow[]>([])
const zerodhaPreview = ref<BrokerCsvRow[]>([])
const zerodhaImporting = ref(false)
const zerodhaImportResult = ref<ImportResult | null>(null)
const zerodhaImportError = ref<string | null>(null)
const zerodhaShowCsv = ref(false)
// same pattern for upstox*, angelOne*
```

**Import call:**

```ts
const result = await invoke<ImportResult>("import_broker_equity_csv", {
    broker: "zerodha",
    displayName: "Zerodha",
    rows: zerodhaRows.value.map(r => ({
        symbol: r.symbol,
        isin: r.isin,
        quantity: r.quantity,
        avgPrice: r.avgPrice,
        ltp: r.ltp,
        exchange: r.exchange,
    })),
})
```

Track analytics event `{broker}_csv_import_completed` with `{ imported, skipped }`.

---

## How to export CSVs (instructions shown in UI)

- **Zerodha:** Console (console.zerodha.com) → Portfolio → Holdings → Download icon
- **Upstox:** Upstox web → Portfolio → Holdings → Export
- **Angel One:** Angel One web → Portfolio → Holdings → Download

---

## Error handling

- Parse errors: shown in per-broker `Message` component (e.g. "No valid rows found. Check the CSV format.")
- Import errors: Rust `AppError` surfaced via `invoke` rejection → shown in same `Message`
- Zero-row parse: block Import button, show "No valid rows found"

---

## What this does NOT do

- No column-mapping wizard (YAGNI — formats are known and stable)
- No merging with OAuth holdings (import replaces, same as sync)
- No MF, FD, or other asset types
- No new DB migrations
