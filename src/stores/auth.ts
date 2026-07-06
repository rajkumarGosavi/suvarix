import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { enable as enableAutostart } from "@tauri-apps/plugin-autostart";
import { EULA_VERSION } from "@/eulaText";

export const useAuthStore = defineStore("auth", {
    state: () => ({
        isPasswordSet: false,
        isUnlocked: false,
        onboardingComplete: false,
        _onboardingSeen: false,
        _initialized: false,
        _lastActivity: Date.now(),
        eulaVersion: null as string | null,
    }),

    getters: {
        // Null = pre-EULA install upgrading (setting never existed) or never accepted;
        // stale = terms changed since accept. Both cases correctly gate to /eula.
        eulaCurrent: (state) => state.eulaVersion === EULA_VERSION,
    },

    actions: {
        async init() {
            if (this._initialized) return;
            this.isPasswordSet = await invoke<boolean>("is_password_set");
            // onboardingComplete/eulaVersion are loaded after unlock() — DB is locked here
            this._initialized = true;
        },

        async unlock(password: string): Promise<boolean> {
            const ok = await invoke<boolean>("verify_master_password", { password });
            if (ok) {
                this.isUnlocked = true;
                this._lastActivity = Date.now();
                try {
                    const val = await invoke<string>("get_setting", { key: "onboarding_complete" });
                    this.onboardingComplete = val === "true";
                } catch { /* new install — stays false, onboarding will run */ }
                try {
                    this.eulaVersion = await invoke<string>("get_setting", { key: "eula_version" });
                } catch { this.eulaVersion = null; /* pre-EULA install — gate will prompt */ }
            }
            return ok;
        },

        async setup(password: string) {
            await invoke("setup_master_password", { password });
            this.isPasswordSet = true;
            this.isUnlocked = true;
            // onboardingComplete stays false — correct, new user needs onboarding
            // SetupView only lets this run after its own EULA checkbox is ticked, so
            // accept immediately — the user has already agreed to the current terms.
            await this.acceptEula();
            // Default on so bill/maturity reminders can notify even when the app
            // isn't open — user can turn this off any time in Settings.
            try { await enableAutostart(); } catch { /* non-critical */ }
        },

        async acceptEula() {
            await invoke("set_setting", { key: "eula_version", value: EULA_VERSION });
            this.eulaVersion = EULA_VERSION;
        },

        async completeOnboarding() {
            await invoke("set_setting", { key: "onboarding_complete", value: "true" });
            this.onboardingComplete = true;
            this._onboardingSeen = true;
        },

        async lock() {
            // Drops the Rust-side pool/password and stops the background
            // reminder scheduler — without this the scheduler keeps running
            // (and could keep querying the pool) after the UI thinks it's locked.
            await invoke("lock");
            this.isUnlocked = false;
        },

        refreshActivity() {
            this._lastActivity = Date.now();
        },

        checkAutoLock(timeoutMs: number): boolean {
            if (!this.isUnlocked || timeoutMs <= 0) return false;
            if (Date.now() - this._lastActivity >= timeoutMs) {
                this.lock();
                return true;
            }
            return false;
        },
    },
});
