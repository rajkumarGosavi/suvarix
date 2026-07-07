const PREFIX_MESSAGES: [string, string][] = [
    ["Database error:", "Something went wrong accessing your data. Try again."],
    ["IO error:", "Something went wrong reading or writing a file. Try again."],
    ["External API error:", "Couldn't reach the price data service. Check your connection and try again."],
    ["Parse error:", "Received unexpected data. Try again."],
];

export function friendlyError(e: unknown, fallback: string): string {
    const anyE = e as any;
    const raw = String(anyE?.message?.message ?? anyE?.message ?? e ?? "").trim();
    if (!raw) return fallback;

    for (const [prefix, friendly] of PREFIX_MESSAGES) {
        if (raw.startsWith(prefix)) return friendly;
    }

    if (raw.startsWith("Invalid input:")) return raw.slice("Invalid input:".length).trim();

    if (raw === "Wrong password" || raw === "Authentication required") return raw;

    return fallback;
}
