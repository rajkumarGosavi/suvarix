# Suvarix — Privacy Policy

Last updated: 2026-07-15

Suvarix is an offline-first personal finance tracker for Indian investors. This
policy explains what data the app handles and where it goes. In plain terms:
**your financial data stays on your device. The developer never receives it and
has no server that could.**

## 1. Who is responsible

Suvarix is developed and distributed by an independent developer ("the
developer"). There is no company-hosted backend, account system, or analytics
server. Contact: princegosavi12@gmail.com.

## 2. What data the app stores, and where

All data you enter — holdings, transactions, goals, budgets, reminders, broker
credentials, and everything else — is stored **locally on your device** in a
single encrypted database file. The database is encrypted at rest with
SQLCipher (AES-256); the encryption key is derived from your master password,
which is never stored anywhere and is not recoverable if forgotten.

Broker API keys, secrets, and session tokens receive an **additional layer of
field-level AES-256-GCM encryption** on top of the database encryption, keyed by
your master password.

The developer does not have access to your master password, your database, or
any data derived from it.

## 3. What data leaves your device

Suvarix has **no telemetry and no app-hosted backend.** Usage analytics and
error logs, if any, are written **only to the local database** and never
transmitted. Data leaves your device only in the following cases, all initiated
by you:

- **Broker connections (optional).** If you connect a broker (Zerodha, Upstox,
  Angel One), the app talks directly to that broker's official API to fetch your
  holdings. Your credentials are sent only to the broker, never to the
  developer. This data flow is governed by **the broker's own privacy policy and
  API terms**, not this one.
- **Cloud folder sync (optional, off by default).** If you enable auto-sync, the
  app writes an **encrypted** snapshot file (`.svbak`, AES-256-GCM, protected by
  a separate sync password you choose) into a folder you own (e.g. Dropbox,
  Google Drive, OneDrive). Your cloud provider's client software then propagates
  that file between your own devices. The developer never sees this file and
  cannot decrypt it. Broker credentials are **not** included in the sync file.
- **App updates (optional).** The in-app updater checks a public GitHub Releases
  URL for a newer version. This is a standard file download and contains no
  personal data.

## 4. What the app does NOT do

- No advertising, no ad identifiers, no tracking pixels.
- No selling or sharing of personal data with third parties.
- No cloud account, no sign-up, no email collection by the app.
- No background transmission of your financial data to anyone.

## 5. Your rights and controls

Because all data is local and under your control, you can at any time:

- **Export** your data (Settings → Data Management) as a backup or CSV.
- **Delete** all data permanently (Settings → Data Management → Wipe), which
  irreversibly destroys the local database.
- **Disconnect** any broker, which deletes its stored credentials.
- **Disable** cloud sync, after which no further snapshot files are written.

Uninstalling the app removes the application. Your database file and any backup
or sync files you created remain until you delete them yourself (see the
uninstall notes in the README).

## 6. Data protection posture (India DPDP Act 2023)

Suvarix is designed so that the developer is not a "Data Fiduciary" processing
your personal data: there is no server-side processing, no telemetry, and no
collection of your financial data by the developer. Residual exposure is limited
to files you deliberately place in your own cloud folder (encrypted) and to data
you send to brokers/price sources at your initiative under their terms.

## 7. Children

Suvarix is intended for adult investors and is not directed at children.

## 8. Changes to this policy

This policy may be updated as the app evolves. Material changes will accompany an
app release and be reflected in the "Last updated" date above.

## 9. Contact

Questions about privacy: princegosavi12@gmail.com.
