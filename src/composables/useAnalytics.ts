import { invoke } from "@tauri-apps/api/core";

export function useAnalytics() {
    function track(event: string, properties?: Record<string, unknown>) {
        invoke("track_event", {
            name: event,
            properties: properties ? JSON.stringify(properties) : null,
        }).catch(() => {});
    }

    function trackError(type: string, message: string, stack?: string) {
        invoke("track_error", {
            errorType: type,
            message,
            stack: stack ?? null,
            context: null,
        }).catch(() => {});
    }

    function startTimer(): (metric: string) => void {
        const t = performance.now();
        return (metric: string) => {
            invoke("track_perf", {
                metricName: metric,
                valueMs: Math.round(performance.now() - t),
            }).catch(() => {});
        };
    }

    return { track, trackError, startTimer };
}
