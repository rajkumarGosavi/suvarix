# FinFolio — User Guide

FinFolio is a privacy-first personal finance desktop app for Indian investors. All data is stored locally on your device in an encrypted database — nothing is ever sent to the cloud.

---

## Table of Contents

1. [Getting Started](#1-getting-started)
2. [Dashboard](#2-dashboard)
3. [Portfolio](#3-portfolio)
4. [Transactions](#4-transactions)
5. [Liabilities](#5-liabilities)
6. [Income & Expenses](#6-income--expenses)
7. [Data Sources](#7-data-sources)
8. [Reports](#8-reports)
9. [Settings](#9-settings)
10. [Security & Privacy](#10-security--privacy)

---

## 1. Getting Started

### First Launch — Set Master Password

When you open FinFolio for the first time you will see the **Setup** screen.

1. Enter a master password (minimum 8 characters).
2. Re-enter it to confirm.
3. Click **Set Password**.

> Your master password encrypts your database. Write it down and store it safely — it cannot be recovered if forgotten.

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
| List | Transactions | Full transaction log |
| Credit Card | Liabilities | Loans and credit cards |
| Wallet | Income & Expenses | Budget tracking and category trends |
| Database | Data Sources | Price refresh and market indices |
| Chart | Reports | Net worth history and capital gains |
| Cog | Settings | Security, backup, and preferences |

Click the **≡ / ×** button at the top of the sidebar to collapse or expand it. Collapsed mode shows only icons.

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

## 4. Transactions

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

### Editing and Deleting

Click the pencil icon to edit any transaction. Click the trash icon to delete (with confirmation).

### Sign Convention

Credit-type transactions (income, dividend, interest, sell, redemption, deposit) are shown with a **+** prefix. All others show **−**.

---

## 5. Liabilities

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

#### Viewing the Amortization Schedule

Click the **calendar icon** on any loan row to open the amortization schedule.

The schedule shows:
- **Summary row** — total months remaining, total payment, total interest, total principal
- **Monthly breakdown table** — for each month: EMI paid, principal component, interest component, remaining balance

This helps you understand exactly how much interest you are paying and when the loan will be paid off.

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

## 6. Income & Expenses

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

A 12-month bar chart showing income vs. expenses side by side. The most recent month is on the right. This helps identify spending trends over the year.

### Category Breakdown

Two columns showing spending totals by category:

- **Income** — Salary, Dividend, Interest, and other income sources
- **Expenses** — Food, Rent, EMI, Travel, Medical, Utilities, Entertainment, etc.

### Budget Manager

Set monthly spending limits for any expense category.

**To set a budget:**
1. Click the **pencil icon** next to any category, or click **Set Budget** at the bottom.
2. Select the category and enter the monthly limit (₹).
3. Click **Save**.

Each category shows a **progress bar** (capped at 100%) indicating how much of the budget has been used. The bar turns amber when over 75% and red when the budget is exceeded.

---

## 7. Data Sources

Fetches live prices and NAVs from public APIs.

### Market Pulse

Displays **Nifty 50**, **Sensex**, and **USD/INR**.

Click **Fetch** to pull the latest values from Yahoo Finance. If the network is unavailable the values show "—" without crashing.

### Price Refresh

#### Equity Prices

Click **Refresh Prices** to update the `current_price` field for all your equity holdings.

- NSE stocks are fetched as `{SYMBOL}.NS` from Yahoo Finance.
- BSE stocks are fetched as `{SYMBOL}.BO`.

The result shows how many holdings were updated and lists any failures (e.g. symbol not found).

#### Mutual Fund NAVs

Click **Refresh NAVs** to update the `current_nav` field for all your MF holdings.

- Data is fetched from **mfapi.in** (free AMFI-sourced NAV data).
- Requires the **Scheme Code** to be set on each MF holding.

### Import (Coming Soon)

Three import modes are planned but not yet available:

| Mode | Description |
|---|---|
| CSV Import | Import transactions from any CSV with column mapping |
| Bank Statement (PDF) | Auto-parse PDF statements from HDFC, SBI, ICICI, Axis |
| MF Central CAS | Import consolidated account statement from MF Central |

---

## 8. Reports

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

Click **Export CSV** to save the net worth history to a `.csv` file. A save-file dialog will prompt you to choose the location.

---

### Capital Gains

Calculates realised gains from equity and mutual fund sell transactions using **FIFO or LIFO** cost basis.

#### Selecting the Financial Year and Method

- **Financial Year** — dropdown lists the current FY and the previous 4 FYs (India FY: April 1 to March 31).
- **Method** — FIFO (First In, First Out) or LIFO (Last In, First Out). FIFO is the default and is required for Indian tax computation.

#### Gain Summary Cards

- **STCG (Short-Term Capital Gains)** — assets held for less than 12 months. Taxed at 20%.
- **LTCG (Long-Term Capital Gains)** — assets held for 12 months or more. Taxed at 12.5% on gains exceeding ₹1.25 lakh (Budget 2024 rates).

#### Tax Estimate Panel

Below the summary cards, an estimated tax breakdown shows:

| Line | Calculation |
|---|---|
| STCG Tax | STCG × 20% |
| LTCG Exempt | Min(LTCG, ₹1,25,000) |
| LTCG Tax | Max(0, LTCG − ₹1,25,000) × 12.5% |
| **Total Estimated Tax** | STCG Tax + LTCG Tax |

> This is an indicative estimate. Surcharge, cess, and other income are not included. Consult a CA for your final ITR computation.

#### Transactions Table

Each row represents one matched lot:

| Column | Meaning |
|---|---|
| Symbol | Stock or MF name |
| Class | equity or mf |
| Buy Date | Date the lot was purchased |
| Sell Date | Date it was sold |
| Days | Holding period in days |
| Qty | Units matched in this lot |
| Buy ₹ | Cost per unit |
| Sell ₹ | Sale price per unit |
| Gain / Loss | Net gain or loss for this lot |
| Type | STCG or LTCG tag |

#### Exporting to CSV

Click **Export CSV** to save the full transactions table (including all columns above) to a `.csv` file.

---

## 9. Settings

### Security

#### Change Master Password

1. Enter your **current password**.
2. Enter and confirm the **new password** (minimum 8 characters).
3. Click **Change Password**.

The new password takes effect immediately.

#### Auto-lock

Choose how long the app stays unlocked when idle:

| Option | Behaviour |
|---|---|
| 5 minutes | Locks after 5 minutes of no mouse/keyboard activity |
| 15 minutes | Default — good balance for most users |
| 30 minutes | Relaxed — suitable for trusted environments |
| 1 hour | For long working sessions |
| Disabled | App never auto-locks (not recommended) |

Click **Save** after changing. The new timeout takes effect immediately.

---

### Data Management

#### Backup Database

Click **Backup** to save a full copy of your database to any location you choose.

- A native save-file dialog opens.
- The default filename includes today's date (e.g. `finfolio-backup-2026-06-25.db`).
- Keep backups on an external drive or cloud storage folder.

> Recommended: take a backup before updating the app and after entering large amounts of data.

#### Restore Database

Click **Restore** to replace all current data with a previously saved backup.

- You will see a confirmation prompt before proceeding.
- After restoring, restart the app to ensure all views reflect the restored data.

> Warning: Restore overwrites all current data. Take a fresh backup before restoring if you want to preserve current data.

#### Wipe All Data

Permanently deletes all portfolio, transaction, liability, budget, and snapshot records.

1. Click **Wipe Data**.
2. In the confirmation dialog, type `DELETE` exactly.
3. Click **Wipe All Data**.

Your master password and app settings are not affected — only financial data is deleted.

---

### About

Shows:
- App name and version
- Data directory path — where the database file is stored on your computer
- Privacy statement

---

## 10. Security & Privacy

### Local-only Storage

All data is stored in a single SQLite database file on your computer. FinFolio makes no network requests except when you explicitly click a price refresh or market indices button.

### Master Password

The master password is required every time you open the app and after an auto-lock timeout. It is never stored in plain text or transmitted anywhere.

### Database Location

The database file is located in your operating system's application data directory:

| OS | Path |
|---|---|
| Windows | `C:\Users\<you>\AppData\Roaming\com.finfolio.app\` |
| macOS | `~/Library/Application Support/com.finfolio.app/` |
| Linux | `~/.local/share/com.finfolio.app/` |

The exact path is shown in **Settings → About → Data directory**.

### Backup Recommendation

FinFolio does not automatically back up your data. Set a reminder to use **Settings → Backup Database** regularly, especially:
- After adding new holdings
- Before and after app updates
- Monthly as a routine habit

---

## Quick Reference — Common Tasks

| Task | Where |
|---|---|
| Add a stock holding | Portfolio → Equity → Add Equity |
| Add a mutual fund | Portfolio → Mutual Funds → Add MF |
| Set up a SIP | Portfolio → Mutual Funds → SIP Schedules → Add SIP |
| Record an expense | Transactions → Add Transaction (Type: expense) |
| Add a home loan | Liabilities → Loans → Add Loan |
| View loan EMI schedule | Liabilities → Loans → calendar icon on the row |
| Set a monthly budget | Income & Expenses → Budget Manager → pencil icon |
| Refresh stock prices | Data Sources → Equity Holdings → Refresh Prices |
| Refresh MF NAV | Data Sources → Mutual Fund NAV → Refresh NAVs |
| View capital gains | Reports → Capital Gains tab |
| Export gains to CSV | Reports → Capital Gains → Export CSV |
| Take net worth snapshot | Reports → Net Worth History → Take Snapshot |
| Change password | Settings → Security → Change Password |
| Back up data | Settings → Data Management → Backup |
| Lock the app | Sidebar → Lock App (bottom) |
