<!--
GitHub Pages landing page for Suvarix.
To publish: repo Settings → Pages → Source = "Deploy from a branch",
Branch = main, Folder = /docs. Site goes live at
https://rajkumargosavi.github.io/suvarix/
Drop screenshots into docs/screenshots/ (see the Screenshots section below).
-->

<p align="center">
  <img src="https://raw.githubusercontent.com/rajkumarGosavi/suvarix/main/src-tauri/icons/128x128%402x.png" alt="Suvarix" width="96" height="96">
</p>

<h1 align="center">Suvarix</h1>

<p align="center"><strong>Your money, on your machine.</strong><br>
An offline-first personal finance tracker for Indian investors — every rupee stays on your device, encrypted.</p>

<p align="center">
  <a href="https://github.com/rajkumarGosavi/suvarix/releases/latest"><strong>⬇️ Download latest</strong></a> ·
  <a href="https://github.com/rajkumarGosavi/suvarix/blob/main/USER_GUIDE.md">User Guide</a> ·
  <a href="https://github.com/rajkumarGosavi/suvarix/blob/main/PRIVACY.md">Privacy</a> ·
  <a href="https://github.com/rajkumarGosavi/suvarix/blob/main/EULA.md">Licence / EULA</a>
</p>

---

## What it is

Suvarix tracks your whole net worth in one place — **9 asset classes** (equity, mutual funds, FDs, PPF/EPF, gold, crypto, bonds, real estate, insurance), income & expenses, loans & credit cards — and does the India-specific maths for you: **STCG / LTCG capital gains**, net-worth history, allocation, and a 0–100 **Financial Health Score**.

- 🇮🇳 **Built for India** — ₹ Cr/L formatting, Indian tax rules, Indian brokers.
- 🔌 **Pull your holdings** — connect Zerodha, Upstox, or Angel One; import MF Central CAS / Groww CSV / bank statements (HDFC, ICICI).
- 🎯 **Stay on track** — goals, budgets, bill & FD-maturity reminders, savings streaks.
- ☁️ **Optional sync** — you own the folder (Dropbox / Drive / OneDrive); the snapshot is encrypted end-to-end.

## Offline & encrypted — the whole point

- **No account. No cloud backend. No telemetry.** There is no server to send your data to — because there isn't one.
- **Encrypted at rest** with SQLCipher (AES-256). Your master password is the key; it's never stored and never leaves the device.
- **Broker credentials get a second AES-256-GCM layer** on top, keyed by your master password.
- **Your data leaves only when you tell it to** — to a broker's own API, to fetch public prices, or into your own cloud folder as an encrypted file the developer can't read.

Full details: **[Privacy Policy →](https://github.com/rajkumarGosavi/suvarix/blob/main/PRIVACY.md)**

## Download

| Platform | File | Get it |
|---|---|---|
| **Windows 10/11** | `.msi` installer | [Download](https://github.com/rajkumarGosavi/suvarix/releases/latest) |
| **macOS** | `.dmg` (Apple Silicon / Intel) | [Download](https://github.com/rajkumarGosavi/suvarix/releases/latest) |
| **Linux** | `.AppImage` / `.deb` | [Download](https://github.com/rajkumarGosavi/suvarix/releases/latest) |
| **Android** | `.apk` (sideload) | [Download](https://github.com/rajkumarGosavi/suvarix/releases/latest) |

> The desktop app auto-updates itself once installed — every release is signed with the developer's update key and verified before it's applied.
> For Android you will need to install new updates manually as of now.

## "Is this safe? Windows warned me."

Short answer: **yes**, and here's the honest why.

Windows **SmartScreen** (and macOS Gatekeeper) show an "unknown publisher" warning for any app that hasn't yet paid for an OS code-signing certificate. It's a *reputation* prompt, **not** a virus detection. New independent apps all start here until trust builds up.

**To install on Windows:** on the SmartScreen dialog click **More info → Run anyway**.

**To install on macOS:** because the app isn't notarized yet, Gatekeeper blocks the first launch. Either:

- **Right-click** (or Control-click) the app in Applications → **Open** → **Open** again in the dialog; or
- if macOS says the app is *"damaged / can't be opened"*, clear the download quarantine flag once in Terminal:

  ```bash
  xattr -dr com.apple.quarantine /Applications/Suvarix.app
  ```

  Then open it normally. This only tells macOS you trust *this* file — it doesn't disable Gatekeeper.

Why you can trust it anyway:

- **Nothing phones home.** No telemetry, no analytics server, no account. Your finances never touch the internet unless *you* connect a broker or enable sync.
- **Every update is cryptographically signed** with a minisign key and verified by the app before installing — a tampered update is rejected automatically.
- **Verify your download** against the checksums published on each [release](https://github.com/rajkumarGosavi/suvarix/releases/latest) if you want to be sure the file wasn't altered in transit.
- **OS code signing is on the roadmap**, which will remove the SmartScreen prompt entirely.

## Screenshots

<!-- Add PNGs to docs/screenshots/ then uncomment. Recommended: 1280×800. -->
<!--
<p align="center">
  <img src="screenshots/dashboard.png" alt="Dashboard — net worth & allocation" width="45%">
  <img src="screenshots/health-score.png" alt="Financial Health Score" width="45%">
  <img src="screenshots/holdings.png" alt="Holdings across asset classes" width="45%">
  <img src="screenshots/reports.png" alt="Capital gains & reports" width="45%">
</p>
-->

*Screenshots coming soon.*

## FAQ

**Do you see my data?** No. There's no backend. Everything is local and encrypted; even the optional cloud-sync file is encrypted with a password only you hold.

**What if I forget my master password?** It can't be recovered — not even by the developer. Keep a backup and remember it.

**Is it free?** Yes, free to use. It is not open-source; source is proprietary. See the [EULA](https://github.com/rajkumarGosavi/suvarix/blob/main/EULA.md).

**Investment advice?** None. Suvarix shows *your* numbers for information only. The developer is **not** a SEBI-registered adviser.

---

<p align="center"><sub>Suvarix is provided "as is", without warranty. Not financial, tax, or investment advice. © 2026.</sub></p>
