import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export const useAuthStore = defineStore("auth", {
    state: () => ({
        isPasswordSet: false,
        isUnlocked: false,
        _initialized: false,
        _lastActivity: Date.now(),
    }),

    actions: {
        async init() {
            if (this._initialized) return;
            this.isPasswordSet = await invoke<boolean>("is_password_set");
            this._initialized = true;
        },

        async unlock(password: string): Promise<boolean> {
            const ok = await invoke<boolean>("verify_master_password", { password });
            if (ok) {
                this.isUnlocked = true;
                this._lastActivity = Date.now();
            }
            return ok;
        },

        async setup(password: string) {
            await invoke("setup_master_password", { password });
            this.isPasswordSet = true;
            this.isUnlocked = true;
        },

        lock() {
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
