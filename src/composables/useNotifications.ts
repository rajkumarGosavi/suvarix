import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";

export function useNotifications() {
    async function nativeNotify(title: string, body: string) {
        try {
            let granted = await isPermissionGranted();
            if (!granted) {
                const perm = await requestPermission();
                granted = perm === "granted";
            }
            if (granted) sendNotification({ title, body });
        } catch { /* non-fatal */ }
    }
    return { nativeNotify };
}
