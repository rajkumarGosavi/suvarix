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
