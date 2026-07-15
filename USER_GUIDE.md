# Suvarix — User Guide

Suvarix is a privacy-first personal finance desktop app for Indian investors. All data is stored locally on your device in an encrypted database (SQLCipher) — nothing is ever sent to the cloud.

---

## Table of Contents

1. [Getting Started](#1-getting-started)
2. [Dashboard](#2-dashboard)
3. [Portfolio](#3-portfolio)
4. [Goals](#4-goals)
5. [Transactions](#5-transactions)
6. [Liabilities](#6-liabilities)
7. [Income & Expenses](#7-income--expenses)
8. [Data Sources](#8-data-sources)
9. [Reminders & Calendar](#9-reminders--calendar)
10. [Reports](#10-reports)
11. [Settings](#11-settings)
12. [Security & Privacy](#12-security--privacy)
13. [Uninstall](#13-uninstall)

---

## 1. Getting Started

### Installation

Download the build for your OS from the **[latest release](https://github.com/rajkumarGosavi/suvarix/releases/latest)**:

| Platform | File |
|---|---|
| Windows 10/11 | `.msi` installer |
| macOS (Apple Silicon / Intel) | `.dmg` |
| Linux | `.AppImage` or `.deb` |
| Android | `.apk` (sideload) |

Suvarix isn't OS-code-signed yet, so the first launch shows an "unknown
publisher" warning. It's a *reputation* prompt, **not** a virus detection.

- **Windows:** on the **"Windows protected your PC"** dialog click **More info → Run anyway**.
- **macOS:** because the app isn't notarized yet, right-click the app → **Open** → **Open**. If macOS says it's "damaged", run once in Terminal: `xattr -dr com.apple.quarantine /Applications/Suvarix.app`.
- **Linux (.AppImage):** `chmod +x Suvarix_*.AppImage`, then run it.

Suvarix checks for updates automatically and offers to install new versions
in-app — every update is cryptographically verified before it's applied, so you
won't re-download installers.

### Your first 10 minutes

A quick path to get value fast:

1. **Add one real holding** — Portfolio → Equity (or MF/FD) → add manually. Watch it appear on the Dashboard net-worth tile.
2. **Connect a broker or import a file** — Data Sources → Zerodha/Upstox/Angel One, or import an MF Central CAS PDF / Groww CSV.
3. **Log income + a few expenses** — Transactions → Add (or bulk-import a CSV). This powers your savings rate and health score.
4. **Add liabilities** — Liabilities → add a loan (full EMI schedule) or credit card.
5. **Set one goal** — Goals → Add Goal (e.g. emergency fund).
6. **Check your Financial Health Score** on the Dashboard and do its single "Do this next" step.
7. **Turn on backups** — Settings → Data Management → Backup now, and consider Auto Sync into your cloud folder.

### First Launch — Set Master Password

When you open Suvarix for the first time you will see the **Setup** screen.

1. Read and tick the **EULA acceptance** checkbox.
2. Enter a master password (minimum 8 characters).
3. Re-enter it to confirm.
4. Click **Set Password**.

> Upgrading from an older version? You'll see a one-time **EULA screen** after unlock — accept to continue, or lock the app.

> Your master password is the encryption key for your database (SQLCipher, AES-256). Write it down and store it safely — if forgotten, the database cannot be decrypted and your data cannot be recovered.

### First-Run Onboarding

After setting your password, a short onboarding wizard introduces the main sections of the app. You can complete it or click **Skip** — either way it only appears once.

### Unlocking the App

After setup (or after auto-lock), you will see the **Unlock** screen.

- Enter your master password and click **Unlock**.
- The app opens to the Dashboard.

### Sidebar Navigation

The left sidebar contains all main sections:

| Icon | Section | Purpose |
|---|---|---|
| Home | Dashboard | Net worth overview and market pulse |
| Briefcase | Portfolio | Holdings across all 9 asset classes |
| Flag | Goals | Financial goals with progress tracking |
| List | Transactions | Full transaction log |
| Credit Card | Liabilities | Loans and credit cards |
| Bell | Reminders | Bills, recurring payments, net-worth milestones |
| Calendar | Calendar | Month view of bills, SIPs, and FD/bond maturities |
| Wallet | Income & Expenses | Budget tracking and category trends |
| Database | Data Sources | Broker sync (Zerodha/Upstox/Angel One), CAS & CSV imports, price refresh |
| Chart | Reports | Net worth history and capital gains |
| Cog | Settings | Security, backup, diagnostics, and preferences |

Click the **≡ / ×** button at the top of the sidebar to collapse or expand it. Collapsed mode shows only icons with tooltips on hover.

Click **Lock App** at the bottom of the sidebar to lock immediately.

> **Tray behaviour:** closing the window hides Suvarix to the system tray instead of quitting — background reminder notifications keep working while unlocked. Right-click the tray icon to quit fully. Enable **Launch at login** in Settings to start Suvarix (hidden) with Windows.

---

## 2. Dashboard

The Dashboard gives you an at-a-glance view of your complete financial picture.

### Net Worth Card

Displays your current **Total Assets**, **Total Liabilities**, and **Net Worth** (assets minus liabilities).

Values are calculated in real time from all your holdings.

### Asset Allocation Chart

A doughnut chart showing how your wealth is distributed across asset classes:

- Equity, Mutual Funds, FD/RD, PPF/EPF/NPS, Real Estate, Gold, Crypto, Insurance

Hover over any segment to see the exact amount and percentage.

<!-- ### Market Pulse

Shows live **Nifty 50**, **Sensex**, and **USD/INR** rates.

- Click **Fetch** to pull fresh market data.
- Values show "—" until fetched or when offline. -->

---

## 3. Portfolio

The Portfolio section has a tab for each asset class. Each tab lets you add, edit, and delete holdings.

### How to Add a Holding

1. Go to the relevant tab (e.g. **Equity**).
2. Click **Add [type]** in the top-right of the panel.
3. Fill in the form and click **Add**.

### How to Edit or Delete

Every row in the holdings table has **pencil (edit)** and **trash (delete)** icon buttons on the right.

---

### 3.1 Equity

Tracks NSE and BSE listed stocks.

| Field | Description |
|---|---|
| Symbol | NSE/BSE ticker (e.g. RELIANCE, INFY) |
| ISIN | 12-character ISIN code |
| Exchange | NSE or BSE |
| Name | Company name |
| Quantity | Number of shares held |
| Avg Buy Price | Your average cost per share (₹) |

**Current Price** is updated via Data Sources → Refresh Prices.

The table shows: Quantity, Avg Buy Price, Current Price, Current Value, P&L (₹ and %).

---

### 3.2 Mutual Funds

Tracks your MF holdings by folio.

| Field | Description |
|---|---|
| Scheme Name | Full fund name |
| AMC Name | Fund house (e.g. Mirae Asset) |
| Scheme Code | AMFI scheme code (used for NAV refresh) |
| Folio Number | Your folio with this AMC |
| Units | Total units held |
| Avg NAV | Your average purchase NAV (₹) |
| Direct Plan | Toggle for Direct vs Regular |
| Growth Option | Toggle for Growth vs Dividend |

**Current NAV** is updated via Data Sources → Refresh NAVs.

**Tip:** Use the MF Central CAS import (see [Data Sources](#8-data-sources)) to populate all your MF holdings automatically with correct ISINs and Avg NAV.

#### SIP Schedules

Below the MF holdings table is a **SIP Schedules** section.

**To add a SIP:**
1. Click **Add SIP**.
2. Enter the scheme name/code, amount (₹), frequency (monthly / quarterly / weekly), and debit day.
3. Set start date and optionally an end date.
4. Keep **Active** toggled on.
5. Click **Add SIP**.

The table shows the **Next SIP date** computed from the debit day and frequency.

---

### 3.3 Fixed Deposits (FD)

| Field | Description |
|---|---|
| Bank Name | Bank or NBFC name |
| Account Number | FD receipt / account number (optional) |
| Principal | Deposited amount (₹) |
| Interest Rate | Annual rate (%) |
| Compounding | quarterly, monthly, annual, simple |
| Tenure | Duration in months |
| Start Date | Date of deposit |
| Maturity Date | Date of maturity |
| Maturity Amount | Expected amount at maturity (₹) |
| Cumulative | Toggle (cumulative = no periodic payout) |

> Suvarix alerts you when an FD approaches maturity — an info toast at 30 days, warning at 7 days, and a "matured" alert (with native notification) once the date passes. Maturities also appear on the [Calendar](#9-reminders--calendar).

---

### 3.4 PPF / EPF / NPS

Tracks provident fund and pension accounts.

| Field | Description |
|---|---|
| Account Type | PPF, EPF, NPS, or VPF |
| Account Number | (optional) |
| Balance | Current total balance (₹) |
| Interest Rate | Annual rate (%) |
| Financial Year | FY the entry is for (e.g. 2024-25) |
| Employer Contribution | For EPF — employer's share (₹) |
| Employee Contribution | Your share (₹) |

---

### 3.5 Real Estate

| Field | Description |
|---|---|
| Property Name | Description (e.g. "3BHK Pune") |
| Property Type | residential, commercial, land, agricultural |
| Location | City or address |
| Purchase Price | What you paid (₹) |
| Purchase Date | Date of purchase |
| Current Value | Your estimated market value today (₹) |
| Rental Income | Monthly rental income (₹), if any |
| Has Mortgage | Toggle if this property has a linked loan |

---

### 3.6 Gold

| Field | Description |
|---|---|
| Gold Type | physical, digital, etf, sgb |
| Name | Description (e.g. "SGB 2023-24 Series I") |
| Weight (grams) | For physical gold |
| Purity | e.g. 24K, 22K |
| Units | For ETF/SGB/digital gold |
| Avg Buy Price | Cost per gram or unit (₹) |
| Maturity Date | For SGB — the 8-year maturity date |

---

### 3.7 Crypto

| Field | Description |
|---|---|
| Exchange Name | CoinDCX, WazirX, Binance, etc. |
| Coin Symbol | BTC, ETH, SOL, etc. |
| Quantity | Units held |
| Avg Buy Price | Your average cost per unit (₹) |

---

### 3.8 Bonds

Tracks government bonds, corporate bonds, and debentures.

| Field | Description |
|---|---|
| Issuer Name | e.g. HDFC Ltd., Government of India |
| Bond Type | government, corporate, etc. |
| ISIN | 12-character ISIN (optional) |
| Credit Rating | AAA, AA+, … (optional) |
| Face Value | Face value per bond (₹) |
| Quantity | Number of bonds/units |
| Purchase Price | Your cost per bond (₹) |
| Current Price | Market price per bond (₹, optional) |
| Coupon Rate | Annual coupon (%) |
| Coupon Frequency | Payout frequency |
| Purchase / Maturity Date | Holding period |

The table shows invested value, current value, P&L, days to maturity, and rating. Bond maturities trigger the same 30-day / 7-day / matured alerts as FDs and appear on the Calendar.

---

### 3.9 Insurance

| Field | Description |
|---|---|
| Type | life, health, term, ulip, vehicle, home |
| Provider | Insurer name |
| Policy Number | (optional) |
| Premium Amount | Per-period premium (₹) |
| Frequency | monthly, quarterly, annual |
| Coverage | Sum insured / coverage amount (₹) |
| Maturity Value | For ULIP/endowment (₹) |
| Start / End Date | Policy period |
| Next Due Date | Next premium due date |

---

## 4. Goals

Set financial targets and track progress based on your total portfolio value.

### Adding a Goal

1. Click **Add Goal**.
2. Fill in:
   - **Goal Name** — e.g. "Buy a House", "Kids Education", "Retirement"
   - **Category** — Home, Vehicle, Education, Retirement, Travel, Emergency Fund, or Other
   - **Target Amount (₹)** — the amount you need to reach
   - **Target Date** — when you want to achieve it
   - **Notes** — optional details
3. Click **Add Goal**.

### How Progress Is Calculated

```
Progress = Total Portfolio Value ÷ Target Amount × 100
```

All goals use the same number — your total net worth (all assets combined). This represents your overall financial progress toward each target.

### Goal Cards

Each goal appears as a card showing:
- Progress bar and percentage
- Current portfolio value vs. target amount
- Target date and months remaining
- "Overdue" tag if the target date has passed
- **Achieved** badge (green border) when your portfolio value reaches or exceeds the target

### Editing and Deleting Goals

Click the **pencil icon** on a goal card to edit any field. Click the **trash icon** to delete (with confirmation).

---

## 5. Transactions

The Transactions view is a full log of all financial activity — income, expenses, investments, and more.

### Adding a Transaction

1. Click **Add Transaction** (top right).
2. Fill in:
   - **Date** — transaction date and time (defaults to right now)
   - **Type** — buy, sell, dividend, interest, sip, redemption, deposit, withdrawal, expense, income, emi, transfer
   - **Amount** — in ₹
   - **Category** — pick from your managed category list (see [Managing Categories](#managing-categories) below)
   - **Tag** — optional free-text label for extra grouping beyond category
   - **Asset Class** — optional link to an asset class
   - **Description** — free text label
   - **Notes** — any additional detail
3. Click **Add**.

### Managing Categories

Click **Manage Categories** (top right, next to Add Transaction) to open the category list shared across Transactions, Budgets, and Recurring Transactions.

- Type a name and click **+** to add a new category.
- Click any category name to rename it inline (saves on Enter or when you click away).
- Click the **trash icon** to delete a category. You'll be asked to confirm. **Deletion is blocked if the category is still used** by any transaction, budget, or recurring transaction — you'll see an error naming how many records reference it. Recategorize or remove those records first, then delete the category.

### Searching, Sorting, and Filtering

- **Search box** — matches description, category, or tag across your full transaction history (not just the current page), with a short debounce as you type.
- **Column sort** — click the **Date** or **Amount** column header to sort; click again to reverse direction.
- **Filter bar** — narrow by **From Date** and **Type**. Click the **× button** to clear filters.
- Results are paginated — use the paginator below the table to move through pages.

### Importing Transactions from CSV

Use this when you have transaction/expense history exported from another app (e.g. MoneyWallet) or bank statement — different from the "Holdings CSV Import" used for portfolio holdings (see [Data Sources](#8-data-sources)).

1. Go to **Data Sources → Import → Import CSV** (under the Transaction CSV Import card).
2. Upload any CSV — Suvarix reads its header row and tries to guess which column is which.
3. On the mapping screen, confirm or correct the column mapping: **Date**, **Amount** (both required), and optionally **Category**, **Description**, **Notes**.
4. Review the preview table, then click **Import**.

**Accepted date formats:** `YYYY-MM-DD HH:MM:SS`, `YYYY-MM-DD`, `DD/MM/YYYY`, `DD-MM-YYYY`, or `MM/DD/YYYY`. Rows with a date Suvarix can't parse are skipped.

**Amount sign convention on import:** a negative amount in the source CSV is imported as an **expense**; zero or positive is imported as **income**. Rows with an unparseable or zero amount are skipped. Re-importing the same file won't create duplicates — rows matching an existing transaction's date, amount, and description are skipped automatically.

### Sign Convention

Credit-type transactions (income, dividend, interest, sell, redemption, deposit) are shown with a **+** prefix. All others show **−**.

---

## 6. Liabilities

Tracks your outstanding loans and credit card balances.

### Loans

#### Adding a Loan

1. Click **Add Loan**.
2. Fill in:
   - **Loan Type** — home, car, personal, education, gold
   - **Lender Name** — bank or NBFC
   - **Account Number** — optional
   - **Principal** — original loan amount (₹)
   - **Outstanding** — remaining balance (₹)
   - **Interest Rate** — annual rate (%)
   - **EMI Amount** — monthly instalment (₹)
   - **Tenure** — remaining months
   - **Disbursement Date** — when the loan was issued
   - **Next EMI Date** — optional

#### Viewing the Amortisation Schedule

Click the **calendar icon** on any loan row to open the amortisation schedule.

The schedule shows:
- **Summary** — total months remaining, total payment, total interest payable
- **Monthly table** — for each month: EMI, principal component, interest component, remaining balance

---

### Credit Cards

#### Adding a Credit Card

1. Click **Add Card**.
2. Fill in:
   - **Bank Name** — issuing bank
   - **Card Name** — e.g. HDFC Regalia (optional)
   - **Last 4 Digits** — for identification
   - **Credit Limit** — sanctioned limit (₹)
   - **Current Balance** — amount owed right now (₹)
   - **Due Date** — day of month (1–31) when bill is due
   - **Minimum Payment** — minimum amount due (₹)

---

## 7. Income & Expenses

Tracks your cash flow and compares spending against budgets.

### Period Selector

Choose between:
- **This Month** — current calendar month
- **Last Month** — previous calendar month
- **All Time** — all transactions
- **Custom Range** — pick a **From** and **To** date with the two date pickers that appear; the summary, chart, and category breakdown recompute for that exact range as soon as both dates are set

### Summary Cards

Three cards at the top show:
- **Total Income** — sum of income, salary, dividend, interest transactions
- **Total Expenses** — sum of expense, emi transactions
- **Net Savings** — income minus expenses

### Monthly Trend Chart

A 12-month bar chart showing income vs. expenses side by side. The most recent month is on the right.

### Category Breakdown

Two columns showing spending totals by category — Income and Expenses.

### Budget Manager

Set monthly spending limits for any expense category. Click **Manage Categories** here (or from Transactions/Reminders) to add, rename, or delete categories — see [Managing Categories](#managing-categories).

**To set a budget:**
1. Click the **pencil icon** next to any category.
2. Enter the monthly limit (₹) and click **Save**.

Each category shows a **progress bar** (capped at 100%). The bar turns amber above 75% and red when the budget is exceeded.

---

## 8. Data Sources

Import holdings automatically and keep prices up to date.

---

### 8.1 Zerodha Kite — Equity Import

Automatically import your Zerodha equity holdings. Requires a free personal Kite Connect API key.

#### One-Time Setup

1. Go to [kite.zerodha.com/developers](https://kite.zerodha.com/developers) and log in with your Zerodha account.
2. Click **Create new app**.
3. Set the **Redirect URL** to exactly: `http://127.0.0.1:7459`
4. Copy your **API Key** and **API Secret**.
5. In Suvarix → Data Sources → Zerodha: paste both and click **Save & Connect**.
6. Your browser opens the Zerodha login page — log in. The app captures the token automatically and shows "Connected".

> The API key and secret are stored only in your local database, with an **extra
> AES-256-GCM layer** (keyed by your master password) on top of the database
> encryption. They never leave your device and are **not** included in sync files
> — you enter them once per device.

#### Daily Reconnect

Zerodha access tokens expire at midnight IST every day. When the app shows "Token expired":

1. Click **Reconnect**.
2. Log in on the browser page that opens.
3. The app returns to Connected status.

#### Syncing Holdings

Click **Sync Holdings** to pull your latest equity positions from Zerodha. Your Portfolio → Equity tab updates immediately with current quantities and prices.

#### Disconnecting

Click **Disconnect** to remove your API credentials and access token from the app.

---

### 8.2 Upstox — Equity Import

Same pattern as Zerodha, using the Upstox API v2 (free tier available).

1. Create an app at the Upstox developer portal and set the redirect URL to `http://127.0.0.1:7460`.
2. In Suvarix → Data Sources → Upstox: paste the API key and secret, click **Save & Connect**, and log in on the browser page that opens.
3. Click **Sync Holdings** to import your equity positions.

---

### 8.3 Angel One — Equity Import

Uses the free Angel One SmartAPI. No browser redirect — you log in directly in the app.

1. Create a SmartAPI app at [smartapi.angelbroking.com](https://smartapi.angelbroking.com) to get an API key.
2. In Suvarix → Data Sources → Angel One: enter your API key, client ID, PIN, and TOTP secret.
3. Click **Login**, then **Sync Holdings**.

---

### 8.4 Groww & Holdings CSV Import

- **Groww** — export your holdings CSV from Groww and upload it in Data Sources. The parser maps Groww columns automatically.
- **Holdings CSV Import** — a generic import dialog that works for **every asset type** (equity, MF, FD, bonds, gold, crypto, …). Upload any CSV, map its columns to Suvarix fields in the preview, and import. Useful for spreadsheets or brokers without a direct integration.

> Multi-broker equity: holdings from different brokers are grouped by ISIN in the Equity tab, with a per-broker breakdown you can expand on each row.

---

### 8.5 MF Central CAS — Mutual Fund Import

Import all your mutual fund holdings from an MF Central Consolidated Account Statement PDF.

#### Why Two PDFs?

MF Central offers two CAS formats:

| PDF | Contains | Used for |
|---|---|---|
| **Summary CAS** | Invested value per folio | Calculating Avg NAV (cost basis) |
| **Detailed CAS** | ISIN codes | Identifying each scheme uniquely |

Uploading both together gives you the best of both: correct ISINs **and** correct Avg NAV, which means accurate P&L in your portfolio.

#### Downloading the PDFs

1. Go to [www.mfcentral.com](https://www.mfcentral.com) and log in.
2. Navigate to **Consolidated Account Statement**.
3. Download the **Summary** version — save as e.g. `CAS-Summary.pdf`.
4. Download the **Detailed** version — save as e.g. `CAS-Detailed.pdf`.
5. Note the password you set when generating each PDF (usually your PAN + date of birth).

#### Importing

1. In Suvarix → Data Sources → MF Central CAS:
   - Upload the Summary PDF in the **left slot** (labelled "Summary").
   - Upload the Detailed PDF in the **right slot** (labelled "Detailed").
2. Enter your CAS PDF **password**.
3. Click **Parse** — a preview table appears showing all holdings with ISIN and Avg NAV filled in.
4. Review the data, then click **Import** to save to your portfolio.

> If you only have one PDF, you can upload it alone in either slot. Summary-only: Avg NAV is correct, ISIN is blank. Detailed-only: ISIN is correct, Avg NAV is 0 (P&L will show ₹0).

#### What Gets Imported

- Scheme name, folio number, units, ISIN, Avg NAV, current NAV
- Existing holdings for the same folio + scheme are updated (not duplicated)
- An import log entry is created showing how many records were imported/skipped

---

### 8.6 Transaction CSV Import

A generic column-mapping importer for **transaction/ledger** history — separate from the Holdings CSV Import in 8.4, which is for portfolio holdings, not cash flow.

1. In Suvarix → Data Sources → **Transaction CSV Import**, upload any CSV export (bank statement, expense-tracker export, etc.).
2. Suvarix guesses the column mapping from the header row (looking for things like `date`, `amount`, `category`).
3. Confirm or correct the mapping — **Date** and **Amount** are required; **Category**, **Description**, and **Notes** are optional.
4. Review the preview table, then click **Import**. Imported rows appear immediately in [Transactions](#5-transactions).

See [Importing Transactions from CSV](#importing-transactions-from-csv) in the Transactions section for accepted date formats and the amount sign convention.

### 8.7 Price Refresh

#### Equity Prices

Click **Refresh Prices** to update the current price for all your equity holdings from market data feed.

- NSE stocks: fetched as `{SYMBOL}.NS`
- BSE stocks: fetched as `{SYMBOL}.BO`

The result shows how many holdings updated and lists any failures.

#### Mutual Fund NAVs

Click **Refresh NAVs** to update the current NAV for all your MF holdings from mfapi.in (free AMFI-sourced data).

Requires the **Scheme Code** to be set on each MF holding. Holdings imported via CAS will have the scheme code populated automatically.

---

## 9. Reminders & Calendar

### Reminders

Keeps you on top of recurring obligations. Three panels:

- **Bills** — add a bill with name, amount, and due date. Click **Mark Paid** when settled — a matching transaction is recorded automatically.
- **Recurring transactions** — define repeating income/expense entries (rent, salary, subscriptions). When items fall due, select them and click **Apply** to record them into the Transactions ledger in one step.
- **Milestones** — set net-worth milestones (e.g. ₹50L). You get a toast + native notification when your net worth crosses one.

### Calendar

A month view that plots all date-driven items in one place, colour-coded by type:

- Bill due dates
- SIP debit dates
- Recurring transactions
- **FD maturities** (amber) and **bond maturities** (purple)
- Goal target dates and milestones

Click any day to see its events. Maturity alerts (30-day / 7-day / matured) fire automatically when the app is open — see [Fixed Deposits](#33-fixed-deposits-fd). While the app is unlocked (even hidden in the tray), a background scheduler also checks bills and maturities every 30 minutes and fires native notifications.

---

## 10. Reports

Two report tabs: **Net Worth History** and **Capital Gains**.

### Net Worth History

Tracks your net worth over time using point-in-time snapshots.

#### Taking a Snapshot

Click **Take Snapshot** to record today's net worth (assets − liabilities) into history. Take a snapshot monthly to build a meaningful trend.

#### Viewing History

Use the period buttons (3 Months / 6 Months / 12 Months / 2 Years) to change the chart range.

The line chart plots three series: Net Worth, Total Assets, Total Liabilities.

The table below the chart lists every snapshot with exact values.

#### Exporting to CSV

Click **Export CSV** to save the net worth history to a `.csv` file.

---

### Capital Gains

Calculates realised gains from equity and mutual fund sell transactions.

#### Selecting the Financial Year and Method

- **Financial Year** — India FY (April 1 to March 31). Lists current and previous 4 FYs.
- **Method** — FIFO (default, required for Indian tax) or LIFO.

#### Gain Summary Cards

- **STCG (Short-Term Capital Gains)** — equity held < 12 months, debt < 36 months. Taxed at 20%.
- **LTCG (Long-Term Capital Gains)** — held longer. Taxed at 12.5% on gains above ₹1.25 lakh (Budget 2024 rates).

#### Tax Estimate

| Line | Calculation |
|---|---|
| STCG Tax | STCG × 20% |
| LTCG Exempt | Min(LTCG, ₹1,25,000) |
| LTCG Tax | Max(0, LTCG − ₹1,25,000) × 12.5% |
| **Total Estimated Tax** | STCG Tax + LTCG Tax |

> Indicative only. Surcharge, cess, and other income are not included. Consult a CA for your ITR.

#### Exporting to CSV

Click **Export CSV** to save the full gain/loss transactions table.

---

## 11. Settings

### Security

#### Change Master Password

1. Enter your **current password**.
2. Enter and confirm the **new password** (minimum 8 characters).
3. Click **Change Password**. The database is re-encrypted with the new key.

#### Auto-lock

| Option | Behaviour |
|---|---|
| 5 minutes | Locks after 5 min of no mouse/keyboard activity |
| 15 minutes | Default |
| 30 minutes | Relaxed |
| 1 hour | Long working sessions |
| Disabled | Never auto-locks (not recommended) |

Click **Save** after changing.

---

### Appearance

Switch between **Light**, **Dark**, and **System** (follows your Windows theme setting).

---

### Data Management

#### Backup Database

Click **Backup** to save a full copy of your database.

- A native save-file dialog opens.
- Default filename includes today's date (e.g. `suvarix-backup-2026-06-25.db`).
- Store backups on an external drive or cloud folder.

> Recommended: back up before updating the app and after entering large amounts of data.

#### Restore Database

Click **Restore** to replace all current data with a previously saved backup.

- You will see a confirmation prompt before proceeding.
- Restart the app after restoring to ensure all views reflect the restored data.

#### Sync Backup (manual, cross-device)

Exports/imports a password-encrypted **`.svbak` snapshot** — a portable copy of your financial data (holdings, transactions, goals, reminders, budgets) for moving between devices. Not the same as Backup Database (raw `.db` copy). **Broker API keys are deliberately excluded** from the snapshot for security — re-enter them once on each device.

- **Export:** click **Export Sync Backup**, choose a location, set a **sync password** (independent of your master password).
- **Import:** click **Import Sync Backup**, pick the `.svbak` file, enter its sync password. **This replaces ALL financial data on the device**; your master password is unchanged.

#### Auto Sync (background, via your own cloud folder)

Keeps two or more devices in sync without any Suvarix server:

1. Pick a **sync folder** that a cloud client you already use (Dropbox, Google Drive, OneDrive, Syncthing…) keeps synced across your devices.
2. Set a **sync password** — it encrypts the snapshot file (AES-256-GCM) and is itself stored encrypted under your master password.
3. Toggle **Auto Sync on**, optionally adjust the interval (default 30 minutes, minimum 5).
4. Repeat on your other device with the **same folder and sync password**.

Each tick pulls the folder's `suvarix-sync.svbak` if it's newer than what this device last applied (last-write-wins by export timestamp), then pushes a fresh snapshot. A toast appears only when newer data was actually pulled. **Sync Now** runs a cycle immediately. Sync only runs while the app is unlocked.

> Because sync is last-write-wins on the whole snapshot, avoid entering data on two devices at the same time — the later export wins.

#### Wipe All Data

Permanently deletes all portfolio, transaction, liability, budget, and snapshot records.

1. Click **Wipe Data**.
2. Type `DELETE` exactly in the confirmation dialog.
3. Click **Wipe All Data**.

Your master password and app settings are not affected.

---

### Diagnostics

Suvarix records usage events, errors, and page load times **locally on your device only**. Nothing is sent anywhere.

| Panel | What it shows |
|---|---|
| Feature Usage | Which screens you visit and how often |
| Recent Errors | Any app errors that occurred, with timestamp |
| Performance | Average navigation time per screen |

#### Exporting Diagnostics

Click **Export** to save a JSON file containing all three data sets. You can attach this file to a feedback message and send it to the developer — this helps diagnose issues without sharing any financial data.

#### Clearing Diagnostics

Click **Clear** (confirm with the dialog) to delete all diagnostic data from your device. Financial data is unaffected.

---

### About

Shows app name, version, data directory path, and privacy statement. The data directory path is where your `suvarix.db` database file lives.

---

## 12. Security & Privacy

### Local-Only Storage

All data is stored in a single SQLite database file on your computer, **encrypted at rest with SQLCipher (AES-256)**. Suvarix makes no network requests except when you explicitly click a price refresh, market indices fetch, or broker sync button. Auto Sync (if enabled) only writes an encrypted snapshot file to a local folder — any cloud upload is done by your own sync client, never by Suvarix.

### Master Password

Your master password **is the database encryption key** — the database file cannot be opened without it. It is required every time you open the app and after an auto-lock timeout. It is never stored anywhere (in any form) or transmitted; changing it re-encrypts the database with the new key. While the app is unlocked the password is held in memory only, and its bytes are scrubbed when you lock or quit.

### Broker Credentials

Broker API keys, secrets, and session tokens receive an **additional field-level
AES-256-GCM layer**, keyed by your master password, on top of the database's
SQLCipher encryption. They are sent only to the respective broker's own API
(never to the developer) and are **excluded from sync files**, so you re-enter
them once on each device.

### Database Location

| OS | Path |
|---|---|
| Windows | `C:\Users\<you>\AppData\Roaming\com.rajkumar.suvarix\` |
| macOS | `~/Library/Application Support/com.rajkumar.suvarix/` |
| Linux | `~/.local/share/com.rajkumar.suvarix/` |

The exact path is shown in **Settings → About → Data directory**.

### Backup Recommendation

Suvarix does not automatically back up your data. Set a reminder to use **Settings → Backup Database** regularly:
- After adding new holdings
- Before and after app updates
- Monthly as a routine habit

---

## 13. Uninstall

Uninstalling removes the application. **Your database, backups, and `.svbak` sync
files are not deleted by the uninstaller** — delete them yourself for a clean
wipe. (To erase data *inside* the app first, use **Settings → Data Management →
Wipe All Data**.)

| OS | Uninstall | Then optionally delete |
|---|---|---|
| Windows | Settings → Apps → *Suvarix* → Uninstall | `%APPDATA%\com.rajkumar.suvarix\` |
| macOS | Drag **Suvarix.app** to the Trash | `~/Library/Application Support/com.rajkumar.suvarix/` |
| Linux | Remove the `.deb` (`sudo apt remove suvarix`) or delete the `.AppImage` | `~/.local/share/com.rajkumar.suvarix/` |
| Android | Long-press the app icon → Uninstall | — |

The exact data-directory path for your install is shown at **Settings → About →
Data directory**. Any `.svbak` files you placed in a cloud folder remain until you
delete them there.

---

## Quick Reference — Common Tasks

| Task | Where |
|---|---|
| Add a stock holding | Portfolio → Equity → Add Equity |
| Import equity from a broker | Data Sources → Zerodha / Upstox / Angel One → Sync Holdings |
| Import holdings from any CSV | Data Sources → Holdings CSV Import |
| Add a mutual fund manually | Portfolio → Mutual Funds → Add MF |
| Import MF from MF Central CAS | Data Sources → MF Central CAS |
| Add a bond | Portfolio → Bonds → Add Bond |
| Set up a SIP | Portfolio → Mutual Funds → SIP Schedules → Add SIP |
| Add a financial goal | Goals → Add Goal |
| Record an expense | Transactions → Add Transaction (Type: expense) |
| Add/rename/delete a category | Transactions / Income & Expenses / Reminders → Manage Categories |
| Import transaction history from CSV | Data Sources → Transaction CSV Import |
| View spending for a custom date range | Income & Expenses → period selector → Custom Range |
| Add a home loan | Liabilities → Loans → Add Loan |
| View loan EMI schedule | Liabilities → Loans → calendar icon on the row |
| Set a monthly budget | Income & Expenses → Budget Manager → pencil icon |
| Track a bill or recurring payment | Reminders → Bills / Recurring |
| See upcoming maturities and dues | Calendar |
| Refresh stock prices | Data Sources → Equity → Refresh Prices |
| Refresh MF NAV | Data Sources → Mutual Funds → Refresh NAVs |
| View capital gains | Reports → Capital Gains tab |
| Export gains to CSV | Reports → Capital Gains → Export CSV |
| Take net worth snapshot | Reports → Net Worth History → Take Snapshot |
| Change password | Settings → Security → Change Password |
| Back up data | Settings → Data Management → Backup |
| Move data to another device | Settings → Data Management → Sync Backup (export/import `.svbak`) |
| Keep devices in sync automatically | Settings → Data Management → Auto Sync |
| Start with Windows / tray | Settings → Launch at login toggle |
| Export diagnostics for feedback | Settings → Diagnostics → Export |
| Lock the app | Sidebar → Lock App (bottom) |
