import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { OcrLine } from "@/utils/receiptParser";

export interface ScanResult {
    cancelled: boolean;
    fullText: string;
    lines: OcrLine[];
}

// Platform support is a fact of the running binary — probe once per app run
// and share across components. Desktop stub answers false; a missing plugin
// (older binary) rejects, which we also treat as unsupported.
const supported = ref(false);
let probed = false;

export function useReceiptScan() {
    const scanning = ref(false);

    async function probeSupport() {
        if (probed) return;
        probed = true;
        try {
            supported.value = await invoke<boolean>("plugin:receipt-ocr|is_supported");
        } catch {
            supported.value = false;
        }
    }

    /** Opens camera/gallery, runs on-device OCR. Resolves null when the user cancels. */
    async function scan(source: "camera" | "gallery"): Promise<ScanResult | null> {
        scanning.value = true;
        try {
            const res = await invoke<ScanResult>("plugin:receipt-ocr|scan_receipt", { source });
            return res.cancelled ? null : res;
        } finally {
            scanning.value = false;
        }
    }

    return { supported, scanning, probeSupport, scan };
}
