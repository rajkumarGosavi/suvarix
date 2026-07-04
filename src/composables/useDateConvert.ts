/** Convert "YYYY-MM-DD" string from Rust to a local Date (avoids UTC midnight offset). */
export function strToDate(s: string | null | undefined): Date | null {
    if (!s) return null;
    const [y, m, d] = s.split("-").map(Number);
    return new Date(y, m - 1, d);
}

/** Convert a Date picked in the UI back to "YYYY-MM-DD" for the Rust backend. */
export function dateToStr(d: Date | null | undefined): string | null {
    if (!d) return null;
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
}

/** Convert a Date to "YYYY-MM-DD HH:MM:SS" for transaction timestamps (used only by TransactionsView). */
export function dateTimeToStr(d: Date | null | undefined): string | null {
    if (!d) return null;
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    const hh = String(d.getHours()).padStart(2, "0");
    const mm = String(d.getMinutes()).padStart(2, "0");
    const ss = String(d.getSeconds()).padStart(2, "0");
    return `${y}-${m}-${day} ${hh}:${mm}:${ss}`;
}

/** Parse "YYYY-MM-DD[ HH:MM:SS]" from Rust to a local Date (tolerates legacy date-only rows). */
export function strToDateTime(s: string | null | undefined): Date | null {
    if (!s) return null;
    const [datePart, timePart] = s.split(" ");
    const [y, m, d] = datePart.split("-").map(Number);
    const [hh, mm, ss] = (timePart ?? "00:00:00").split(":").map(Number);
    return new Date(y, m - 1, d, hh || 0, mm || 0, ss || 0);
}
