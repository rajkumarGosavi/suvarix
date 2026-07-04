# Testing the Suvarix debug APK on an Android emulator (Windows)

Prerequisites: Android SDK installed with `platform-tools` and `emulator` (Android Studio's default install), at least one AVD already created, and WSL2 set up per [Building the debug APK](#building-the-debug-apk) below (one-time setup).

## Quick flow (day-to-day)

1. Open your emulator in Android Studio's Device Manager (or launch it manually — see step 3 below).
2. Build the APK — in a **WSL Ubuntu terminal**:
   ```bash
   ~/build-apk.sh
   ```
   Syncs your latest source from Windows, builds the debug APK, and copies it to `C:\Users\Rajkumar\Desktop\suvarix-debug.apk`.
3. Install it — in **PowerShell**:
   ```powershell
   & "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe" install -r "C:\Users\Rajkumar\Desktop\suvarix-debug.apk"
   ```
4. Tap the app icon in the emulator.

That's the whole loop: 1 command in WSL, 1 command in Windows. The detailed steps below cover first-time setup, launching an emulator from scratch, and troubleshooting.

## 1. Locate your Android SDK tools

Default path: `C:\Users\<you>\AppData\Local\Android\Sdk`
Needed subfolders: `platform-tools\adb.exe` and `emulator\emulator.exe`

## 2. Check for existing AVDs (virtual devices)

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\emulator\emulator.exe" -list-avds
```

If nothing's listed, create one first via Android Studio's Device Manager (or `avdmanager create avd ...`), pick any recent API level (30+).

## 3. Launch the emulator

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\emulator\emulator.exe" -avd <AVD_NAME>
```

Leave this running in its own window. First cold boot can take 1-3 minutes.

## 4. Wait for it to fully boot

```powershell
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
& $adb wait-for-device
& $adb shell getprop sys.boot_completed
```

Repeat the last line until it prints `1`.

## 5. Install the APK

```powershell
& $adb install -r "C:\path\to\suvarix-debug.apk"
```

`-r` reinstalls over an existing copy (keeps app data) - drop it for a clean install.

## 6. Launch the app

```powershell
& $adb shell monkey -p com.rajkumar.suvarix -c android.intent.category.LAUNCHER 1
```

(Or just tap the app icon in the emulator UI.)

## 7. Confirm it's alive (no crash)

```powershell
& $adb shell pidof com.rajkumar.suvarix
```

A number = running. Empty = it crashed on launch.

## 8. Check logs if something's wrong

```powershell
& $adb logcat -c        # clear old logs first
# ...reproduce the issue...
& $adb logcat -d *:E    # dump errors since clear
```

Look for `FATAL EXCEPTION`, `AndroidRuntime`, or the package name (`suvarix`) / Rust panic output.

## 9. Grab a screenshot for visual verification

```powershell
& $adb shell screencap -p /sdcard/screen.png
& $adb pull /sdcard/screen.png ".\screen.png"
```

## 10. Uninstall / reset if needed

```powershell
& $adb uninstall com.rajkumar.suvarix
```

Useful when testing the first-run "Create Master Password" flow again from scratch, since app data (including the SQLCipher DB) persists across `-r` reinstalls otherwise.

## Building the debug APK

Local Android builds on Windows must run inside WSL2 — Windows-native builds fail on `openssl-sys`/Perl toolchain issues (rusqlite's vendored OpenSSL cross-compile). WSL2 sidesteps this entirely by giving a real Linux userspace, mirroring the working CI recipe in [`.github/workflows/android-build.yml`](../.github/workflows/android-build.yml).

### One-time setup (already done on this machine)

Inside WSL2 Ubuntu:
- Java 17, Rust (stable, with `x86_64-linux-android`/`aarch64-linux-android`/etc. targets), Node (latest LTS via nvm), pnpm 9 (via corepack)
- Android SDK cmdline-tools + `platform-tools` + `platforms;android-34` + `build-tools;35.0.0` + NDK `27.0.12077973` (pinned to match CI), all under `~/android-sdk`
- Repo copied to WSL2's native filesystem at `~/projects/portfolio-tracker` (not `/mnt/c/...` — `pnpm install` throws `EPERM ... futime` on the Windows-mounted DrvFs path)
- A wrapper script at `~/build-apk.sh` that handles env vars, syncs source, builds, and copies the APK out

### Day-to-day build

```bash
~/build-apk.sh
```

Contents of `~/build-apk.sh`, for reference/editing:

```bash
#!/bin/bash
set -e

export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
nvm use default >/dev/null

export ANDROID_HOME="$HOME/android-sdk"
export NDK_HOME="$ANDROID_HOME/ndk/27.0.12077973"
export PATH="$HOME/.cargo/bin:$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"

echo "Syncing latest source from Windows..."
rsync -a --exclude=node_modules --exclude='src-tauri/target' --exclude='src-tauri/gen' /mnt/c/Users/Rajkumar/projects/portfolio-tracker/ ~/projects/portfolio-tracker/

cd ~/projects/portfolio-tracker
pnpm tauri android build --debug --target x86_64

cp src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk /mnt/c/Users/Rajkumar/Desktop/suvarix-debug.apk
echo "Done. APK copied to C:\Users\Rajkumar\Desktop\suvarix-debug.apk"
```

Swap `--target x86_64` for `--target aarch64` if testing on an arm64 emulator or a physical device.

### Known non-issues

- A dismissible **"Android App Compatibility" 16KB page size** warning shows on launch (Android 15+ native-lib alignment notice). Cosmetic only — doesn't block install or runtime, and doesn't matter until an actual Play Store submission. Safe to tap OK and ignore for local testing.
