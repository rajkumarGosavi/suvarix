# Suvarix — User Guide

Suvarix is a privacy-first personal finance desktop app for Indian investors. All data is stored locally on your device — nothing is ever sent to the cloud.

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
9. [Reports](#9-reports)
10. [Settings](#10-settings)
11. [Security & Privacy](#11-security--privacy)

---

## 1. Getting Started

### Installation

1. Download `Suvarix_0.1.0_x64-setup.exe` from the link shared with you.
2. Double-click the installer and follow the steps.
3. If Windows shows a blue **"Windows protected your PC"** warning, click **More info → Run anyway**. This is expected for unsigned personal software.

### First Launch — Set Master Password

When you open Suvarix for the first time you will see the **Setup** screen.

1. Enter a master password (minimum 8 characters).
2. Re-enter it to confirm.
3. Click **Set Password**.

> Your master password protects your database. Write it down and store it safely — it cannot be recovered if forgotten.

### Unlocking the App

After setup (or after auto-lock), you will see the **Unlock** screen.

- Enter your master password and click **Unlock**.
- The app opens to the Dashboard.

### Sidebar Navigation

The left sidebar contains all main sections:

| Icon | Section | Purpose |
|---|---|---|
| Home | Dashboard | Net worth overview and market pulse |
| Briefcase | Portfolio | Holdings across all 8 asset classes |
| Flag | Goals | Financial goals with progress tracking |
| List | Transactions | Full transaction log |
| Credit Card | Liabilities | Loans and credit cards |
| Wallet | Income & Expenses | Budget tracking and category trends |
| Database | Data Sources | Import from Zerodha / MF Central + price refresh |
| Chart | Reports | Net worth history and capital gains |
| Cog | Settings | Security, backup, diagnostics, and preferences |

Click the **≡ / ×** button at the top of the sidebar to collapse or expand it. Collapsed mode shows only icons with tooltips on hover.

Click **Lock App** at the bottom of the sidebar to lock immediately.

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

### Market Pulse

Shows live **Nifty 50**, **Sensex**, and **USD/INR** rates.

- Click **Fetch** to pull fresh data from Yahoo Finance.
- Values show "—" until fetched or when offline.

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

### 3.8 Insurance

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
   - **Date** — transaction date
   - **Type** — buy, sell, dividend, interest, sip, redemption, deposit, withdrawal, expense, income, emi, transfer
   - **Amount** — in ₹
   - **Category** — optional (Food, Rent, EMI, Salary, Dividend, etc.)
   - **Asset Class** — optional link to an asset class
   - **Description** — free text label
   - **Notes** — any additional detail
3. Click **Add**.

### Filtering

Use the filter bar above the table to narrow results:

- **From Date** — show transactions on or after this date
- **Type** — filter by transaction type

Click the **× button** to clear filters.

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

Set monthly spending limits for any expense category.

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

> The API key and secret are stored only in your local database. They never leave your device.

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

### 8.2 MF Central CAS — Mutual Fund Import

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

### 8.3 Price Refresh

#### Equity Prices

Click **Refresh Prices** to update the current price for all your equity holdings from Yahoo Finance.

- NSE stocks: fetched as `{SYMBOL}.NS`
- BSE stocks: fetched as `{SYMBOL}.BO`

The result shows how many holdings updated and lists any failures.

#### Mutual Fund NAVs

Click **Refresh NAVs** to update the current NAV for all your MF holdings from mfapi.in (free AMFI-sourced data).

Requires the **Scheme Code** to be set on each MF holding. Holdings imported via CAS will have the scheme code populated automatically.

---

## 9. Reports

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

## 10. Settings

### Security

#### Change Master Password

1. Enter your **current password**.
2. Enter and confirm the **new password** (minimum 8 characters).
3. Click **Change Password**.

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

## 11. Security & Privacy

### Local-Only Storage

All data is stored in a single SQLite database file on your computer. Suvarix makes no network requests except when you explicitly click a price refresh, market indices fetch, or Zerodha sync button.

### Master Password

Required every time you open the app and after an auto-lock timeout. Never stored in plain text or transmitted anywhere.

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

## Quick Reference — Common Tasks

| Task | Where |
|---|---|
| Add a stock holding | Portfolio → Equity → Add Equity |
| Import equity from Zerodha | Data Sources → Zerodha → Sync Holdings |
| Add a mutual fund manually | Portfolio → Mutual Funds → Add MF |
| Import MF from MF Central CAS | Data Sources → MF Central CAS |
| Set up a SIP | Portfolio → Mutual Funds → SIP Schedules → Add SIP |
| Add a financial goal | Goals → Add Goal |
| Record an expense | Transactions → Add Transaction (Type: expense) |
| Add a home loan | Liabilities → Loans → Add Loan |
| View loan EMI schedule | Liabilities → Loans → calendar icon on the row |
| Set a monthly budget | Income & Expenses → Budget Manager → pencil icon |
| Refresh stock prices | Data Sources → Equity → Refresh Prices |
| Refresh MF NAV | Data Sources → Mutual Funds → Refresh NAVs |
| View capital gains | Reports → Capital Gains tab |
| Export gains to CSV | Reports → Capital Gains → Export CSV |
| Take net worth snapshot | Reports → Net Worth History → Take Snapshot |
| Change password | Settings → Security → Change Password |
| Back up data | Settings → Data Management → Backup |
| Export diagnostics for feedback | Settings → Diagnostics → Export |
| Lock the app | Sidebar → Lock App (bottom) |
