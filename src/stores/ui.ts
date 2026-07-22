import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export type Theme = "light" | "dark" | "system";

let _mq: MediaQueryList | null = null;
let _mqListener: ((e: MediaQueryListEvent) => void) | null = null;

function applyDarkClass(dark: boolean) {
    document.documentElement.classList.toggle("app-dark", dark);
}

function detachSystemWatcher() {
    if (_mq && _mqListener) {
        _mq.removeEventListener("change", _mqListener);
        _mqListener = null;
    }
}

export const useUiStore = defineStore("ui", {
    state: () => ({
        sidebarCollapsed: false,
        theme: "system" as Theme,
        // Privacy: hide all monetary amounts by default every session.
        // Intentionally NOT persisted — resets to hidden on each app open/unlock.
        hideAmounts: true,
    }),

    actions: {
        toggleSidebar() {
            this.sidebarCollapsed = !this.sidebarCollapsed;
        },

        toggleHideAmounts() {
            this.hideAmounts = !this.hideAmounts;
        },

        async initTheme() {
            try {
                const saved = await invoke<string>("get_setting", { key: "theme" });
                if (saved === "light" || saved === "dark" || saved === "system") {
                    this._applyTheme(saved);
                } else {
                    this._applyTheme("system");
                }
            } catch {
                this._applyTheme("system");
            }
        },

        async setTheme(theme: Theme) {
            this._applyTheme(theme);
            await invoke("set_setting", { key: "theme", value: theme });
        },

        _applyTheme(theme: Theme) {
            this.theme = theme;
            detachSystemWatcher();
            if (theme === "dark") {
                applyDarkClass(true);
            } else if (theme === "light") {
                applyDarkClass(false);
            } else {
                _mq = window.matchMedia("(prefers-color-scheme: dark)");
                applyDarkClass(_mq.matches);
                _mqListener = (e) => applyDarkClass(e.matches);
                _mq.addEventListener("change", _mqListener);
            }
        },
    },
});
