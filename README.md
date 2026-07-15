<img src="public/brand/mark.svg" width="64" height="64" alt="Suvarix logo">

# Suvarix — Personal Finance Tracker

[![Build](https://github.com/rajkumarGosavi/suvarix/actions/workflows/build.yml/badge.svg)](https://github.com/rajkumarGosavi/suvarix/actions/workflows/build.yml)
![Version](https://img.shields.io/github/package-json/v/rajkumarGosavi/suvarix)
![License](https://img.shields.io/badge/license-proprietary-lightgrey)

A privacy-first, offline-first desktop app for Indian investors. Track all your assets, liabilities, income, and expenses in one place — everything stays on your device.

---

## Installation

Download the build for your OS from the **[latest release](https://github.com/rajkumarGosavi/suvarix/releases/latest)** (or the [download page](https://rajkumargosavi.github.io/suvarix/)): Windows `.msi`, macOS `.dmg`, Linux `.AppImage`/`.deb`, or Android `.apk`.

Suvarix isn't OS-code-signed yet, so the first launch shows an "unknown publisher" warning (a reputation prompt, not a virus detection):

- **Windows:** on the **"Windows protected your PC"** dialog click **More info → Run anyway**.
- **macOS:** right-click the app → **Open** → **Open**. If it says "damaged", run once: `xattr -dr com.apple.quarantine /Applications/Suvarix.app`.
- **Linux (.AppImage):** `chmod +x Suvarix_*.AppImage`, then run.

After install, Suvarix checks for updates automatically and prompts you to install new versions in-app (each verified before applying) — no need to re-download.

**New to Suvarix?** The **[full User Guide](USER_GUIDE.md)** walks through every feature; the [first-10-minutes](USER_GUIDE.md#your-first-10-minutes) section gets you productive fast.

---

## First Launch — Master Password

On first launch you'll be asked to accept the EULA and create a **master password**. This password is the encryption key for your database — the entire database file is encrypted at rest (SQLCipher, AES-256). Without the password the file is unreadable.

- Minimum 8 characters.
- If you forget it, data cannot be recovered — there is no reset. The database cannot be decrypted without it.
- You can change it later in **Settings → Security** (the database is re-encrypted with the new key).

After setting the password, a short **onboarding wizard** walks you through the main features. You can skip it and revisit features at your own pace.

---

## Navigation

The sidebar on the left contains all sections:

| Section | What it does |
|---|---|
| Dashboard | Net worth snapshot + allocation chart |
| Portfolio | All your asset holdings |
| Goals | Financial goals with progress tracking |
| Transactions | Income and expense ledger |
| Liabilities | Loans and credit cards |
| Reminders | Bills, recurring payments, net-worth milestones |
| Calendar | Month view of bills, SIPs, and FD/bond maturities |
| Income & Expenses | Category budgets and monthly trends |
| Data Sources | Import from Zerodha, Upstox, Angel One, Groww, MF Central, or CSV |
| Reports | Capital gains and net worth history |
| Settings | Security, appearance, diagnostics |

Collapse the sidebar with the **×** button at the top for more screen space.

---

## Dashboard

Shows a live snapshot of:
- **Net Worth** = Total Assets − Total Liabilities
- **Financial Health Score** — a single 0–100 gauge of your money situation (see below)
- **Asset allocation** — donut chart by category (equity, MF, FD, gold, etc.)
- **Monthly income vs. expenses** bar chart
- **Market indices** — Nifty 50, Sensex, Bank Nifty (refreshed on demand)

### Financial Health Score

A 0–100 score (graded A+ / A / B / C / D) built from six behaviour-first pillars, scored against Indian personal-finance rules of thumb:

| Pillar | Weight | What it measures |
|---|---|---|
| Savings Rate | 20% | Share of income saved over the last 3 months (target 20%+) |
| Emergency Fund | 20% | Months of expenses covered by liquid savings + FDs (target 6) |
| Debt Burden | 20% | EMIs as a share of income + credit-card utilisation |
| Diversification | 15% | Breadth of asset classes + no single class over ~40% |
| Protection | 10% | Life and health insurance in place |
| Net-Worth Trend | 15% | Positive and rising net worth over time |

Pillars with no data yet (e.g. no income logged) are skipped so the score isn't unfairly lowered. Each pillar shows a plain-language status and the single highest-leverage next step ("Do this next"), with the points it could add. The score is tracked daily so you can watch it improve.

---

## Portfolio

The Portfolio section has tabs for each asset class. All figures update live when you refresh prices.

### Equity (Stocks)

Add holdings manually or import via Zerodha (see Data Sources).

| Field | Notes |
|---|---|
| Symbol | NSE/BSE ticker, e.g. `RELIANCE` |
| Exchange | NSE or BSE |
| ISIN | 12-character identifier |
| Quantity | Number of shares |
| Avg Buy Price | Your cost basis per share |
<!-- | Current Price | Auto-refreshed from market data feed | -->

**P&L** = Quantity × (Current Price − Avg Buy Price)

### Mutual Funds

Add MF holdings manually or import via MF Central CAS (see Data Sources).

| Field | Notes |
|---|---|
| Scheme Name | Full AMFI scheme name |
| ISIN | Populated automatically on CAS import |
| Folio Number | From your CAS statement |
| Units | Number of units held |
| Avg NAV | Your cost basis; computed from CAS Summary |
| Current NAV | Auto-refreshed from mfapi.in |

### Fixed Deposits

Track FDs across banks. Enter the principal, interest rate, start date, and maturity date. Maturity value is calculated automatically. The app alerts you when an FD is maturing (30-day, 7-day, and matured notifications) and shows maturities on the Calendar.

### Bonds

Track government bonds, corporate bonds, and debentures. Enter issuer, bond type, face value, quantity, coupon rate, and maturity date. Maturity alerts work the same as FDs.

### PPF / EPF

Track your Public Provident Fund and Employee Provident Fund balances. Enter the current balance and interest rate; the app shows projected maturity value.

### Real Estate

Track property values. Enter purchase price, current estimated value, and purchase date.

### Gold

Track physical gold or Sovereign Gold Bonds. Enter weight (grams), purity, and purchase price.

### Crypto

Track cryptocurrency holdings. Enter symbol, quantity, and average buy price. Manual price updates.

### Insurance

Track life and health insurance policies — premium, sum assured, and maturity date.

---

## Goals

Set financial targets and track progress against your total portfolio value.

**Adding a goal:**
1. Click **Add Goal**.
2. Enter a name (e.g. "Buy a House"), category, target amount, and target date.
3. Optional: add notes.

**Progress** is calculated as:
```
Progress = Total Portfolio Value ÷ Target Amount × 100
```

All goals share the same "current value" — your total net worth. When your portfolio reaches or exceeds a goal's target, the card shows an **Achieved** badge.

**Categories:** Home, Vehicle, Education, Retirement, Travel, Emergency Fund, Other.

---

## Transactions

A simple income/expense ledger. Each transaction has:
- Date & time, amount, type (income or expense)
- Category — managed via the shared **Manage Categories** dialog (also reachable from Income & Expenses and Reminders)
- Account and optional notes/tag

Search, sort by date or amount, and filter by date/type — results are paginated across your full history. Use this to track day-to-day cash flow. The Income & Expenses section aggregates these.

**Bulk import:** Data Sources → Transaction CSV Import lets you upload any transaction/expense-tracker CSV and map its columns (date, amount, category, etc.) instead of entering rows one by one.

---

## Liabilities

### Loans

Track home loans, car loans, personal loans, etc. Enter:
- Principal, interest rate, tenure, start date
- The app generates a full **amortisation schedule** showing EMI breakdown (principal vs. interest) for every month.

### Credit Cards

Track credit card balances, credit limits, and due dates.

---

## Income & Expenses

- **Period** — This Month, Last Month, All Time, or a **Custom Range** (pick From/To dates).
- **Budget** — set a monthly budget per category. The app shows how much you've spent vs. budgeted, with a red highlight when over budget.
- **Monthly trend** — bar chart of income vs. expenses over the last 12 months.
- **Category summary** — breakdown of spending by category for the selected period.

---

## Data Sources

Broker connections: **Zerodha** (OAuth), **Upstox** (OAuth), **Angel One** (SmartAPI + TOTP). Each syncs equity holdings directly into your portfolio. File imports: **MF Central CAS** (PDF), **Groww** (CSV), a generic **Holdings CSV Import** dialog that works for every asset type, and a **Transaction CSV Import** for bulk-loading income/expense history. See the [User Guide](USER_GUIDE.md) for full setup steps.

### Zerodha Kite

Automatically import your equity holdings from Zerodha. Requires a free personal Kite Connect API key.

**One-time setup:**
1. Go to [kite.zerodha.com/developers](https://kite.zerodha.com/developers) and log in.
2. Create a new Kite Connect app (free for personal use).
3. Set the **Redirect URL** to exactly: `http://127.0.0.1:7459`
4. Copy your **API Key** and **API Secret**.
5. In Suvarix → Data Sources → Zerodha: paste both and click **Save & Connect**.
6. A browser window opens — log in to Zerodha. The app captures the token automatically.

**Daily reconnect:** Zerodha access tokens expire at midnight IST. Click **Reconnect** each day before syncing.

**Sync holdings:** Click **Sync Holdings** — your equity tab updates immediately.

### MF Central CAS Import

Import mutual fund holdings from an MF Central Consolidated Account Statement PDF.

**Two PDF types and why you need both:**

| PDF type | Contains | Download from |
|---|---|---|
| Summary CAS | Invested value → used to calculate Avg NAV | MF Central → Consolidated → Summary |
| Detailed CAS | ISIN codes | MF Central → Consolidated → Detailed |

**Import steps:**
1. Download both PDFs from [www.mfcentral.com](https://www.mfcentral.com) → Consolidated Account Statement.
2. In Suvarix → Data Sources → MF Central CAS:
   - Upload the **Summary PDF** in the left slot.
   - Upload the **Detailed PDF** in the right slot.
3. Enter your CAS password (the one you set when generating the statement).
4. Click **Parse** — a preview table shows all holdings with ISIN and Avg NAV.
5. Review, then click **Import** to save to your portfolio.

> If you only have one PDF, you can upload it alone. Summary-only import will have correct Avg NAV but no ISIN. Detailed-only will have ISIN but P&L will show ₹0 (no cost basis).

---

## Reports

### Capital Gains

Shows realised gains broken down by:
- **STCG** (Short Term Capital Gains) — held < 1 year for equity, < 3 years for debt
- **LTCG** (Long Term Capital Gains) — held longer

Useful for tax planning. Filter by financial year.

### Net Worth History

Chart of your net worth over time. Suvarix takes a snapshot each time you view the page. Use **Take Snapshot Now** to manually record the current value.

---

## Settings

### Security

- **Change Master Password** — requires current password.
- **Auto-lock** — lock the app after 5, 15, 30, or 60 minutes of inactivity, or disable.

### Appearance

Switch between **Light**, **Dark**, and **System** (follows your Windows theme).

### Data Management

| Action | What it does |
|---|---|
| Backup Database | Saves a `.db` file you can restore from later |
| Restore Database | Replaces all data with a backup file |
| Sync Backup (export/import) | Password-encrypted `.svbak` snapshot for moving data between devices — you choose the file location and a separate sync password |
| Auto Sync | Point Suvarix at a folder you already sync (Dropbox/Drive/OneDrive); it pushes/pulls an encrypted `.svbak` snapshot in the background on an interval (default 30 min) or via **Sync Now** |
| Wipe All Data | Permanently deletes all portfolio/transaction data (password and settings are kept) |

Keep regular backups. The database file is stored in `%APPDATA%\com.rajkumar.suvarix\suvarix.db`.

### Notifications & Tray

Suvarix can launch at login (Settings toggle) and sits in the system tray — closing the window hides it instead of quitting. While unlocked, a background scheduler checks bills and FD/bond maturities every 30 minutes and fires native Windows notifications. Your master password is never stored for this; notifications stop when you lock or quit.

### Diagnostics

Suvarix records usage events, errors, and page load times locally — nothing is sent anywhere.

- **Feature Usage** — which screens you visit most
- **Recent Errors** — any app errors that occurred
- **Performance** — average navigation times per screen

**Export** saves a JSON file you can attach to a feedback message and send to the developer.

**Clear** deletes all diagnostic data from your device.

---

## Privacy

- All data is stored in a local SQLite database on your device, encrypted at rest with SQLCipher (AES-256). Your master password is the encryption key.
- No data is ever sent to any server or cloud service run by Suvarix. Optional Auto Sync only writes an encrypted snapshot file into a folder *you* control and sync with your own cloud provider.
- Broker API credentials are stored only in your local database, with an **additional field-level AES-256-GCM layer** (keyed by your master password) on top of the SQLCipher at-rest encryption. They are **excluded from the `.svbak` sync snapshot**, so you re-enter them once per device.
- Diagnostic data (if you choose to export and share it) is entirely under your control.

---

## Uninstall

Uninstalling removes the app but **not** your data — your database, backups, and `.svbak` sync files stay until you delete them. (To erase in-app data first: **Settings → Data Management → Wipe All Data**.)

| OS | Uninstall | Then optionally delete |
|---|---|---|
| Windows | Settings → Apps → *Suvarix* → Uninstall | `%APPDATA%\com.rajkumar.suvarix\` |
| macOS | Drag **Suvarix.app** to Trash | `~/Library/Application Support/com.rajkumar.suvarix/` |
| Linux | `sudo apt remove suvarix` or delete the `.AppImage` | `~/.local/share/com.rajkumar.suvarix/` |
| Android | Long-press icon → Uninstall | — |

The exact path is shown at **Settings → About → Data directory**. See the [User Guide](USER_GUIDE.md#13-uninstall) for detail.

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| Sidebar collapse | Click the **×** / **≡** button |
| Lock app | Sidebar → **Lock App** |

---

## Troubleshooting

**App won't open after install**
- Right-click the shortcut → Run as administrator (first launch only).

**Windows SmartScreen blocks the installer**
- Click **More info** → **Run anyway**. The app is safe but unsigned.

**Zerodha login times out**
- The login window must be completed within 3 minutes. Click **Reconnect** and log in promptly.

**CAS import shows "No holdings found"**
- Make sure you entered the correct CAS PDF password.
- Try the Summary PDF alone first to verify parsing works.

**Portfolio P&L shows ₹0 for MF holdings**
- You imported from the Detailed CAS only. Re-import using both Summary + Detailed PDFs together to get Avg NAV populated.

**Data directory**
- Settings → About shows the exact path where your database is stored.

---

## Feedback

Export your diagnostics from **Settings → Diagnostics → Export** and share the JSON file along with a description of the issue.
