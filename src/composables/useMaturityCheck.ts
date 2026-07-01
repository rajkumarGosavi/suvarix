import { invoke } from "@tauri-apps/api/core";
import { APP_NAME } from "@/constants";
import { useToast } from "primevue/usetoast";
import { useNotifications } from "@/composables/useNotifications";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

interface MaturityAlert {
    source: "fd" | "bond";
    sourceId: number;
    name: string;
    principal: number;
    maturityDate: string;
    maturityAmount: number | null;
    daysUntilMaturity: number;
}

export function useMaturityCheck() {
    const toast = useToast();
    const { nativeNotify } = useNotifications();
    const { formatCompact } = useCurrencyFormat();

    async function checkMaturity(days = 30) {
        const alerts = await invoke<MaturityAlert[]>("get_maturity_alerts", { days })
            .catch(() => [] as MaturityAlert[]);

        const matured = alerts.filter(a => a.daysUntilMaturity < 0);
        const soon7   = alerts.filter(a => a.daysUntilMaturity >= 0 && a.daysUntilMaturity <= 7);
        const soon30  = alerts.filter(a => a.daysUntilMaturity > 7);

        for (const a of matured) {
            const label = a.source === "fd" ? "FD" : "Bond";
            const amt = a.maturityAmount ?? a.principal;
            const detail = `${a.name} ${label} of ${formatCompact(amt)} matured ${Math.abs(a.daysUntilMaturity)} day(s) ago. Consider reinvesting.`;
            toast.add({ severity: "warn", summary: "Investment matured", detail, life: 12000 });
            nativeNotify(`${APP_NAME} — Investment matured`, detail);
        }

        for (const a of soon7) {
            const label = a.source === "fd" ? "FD" : "Bond";
            const amt = a.maturityAmount ?? a.principal;
            const dText = a.daysUntilMaturity === 0 ? "today" : `in ${a.daysUntilMaturity} day(s)`;
            const detail = `${a.name} ${label} of ${formatCompact(amt)} matures ${dText}.`;
            toast.add({ severity: "warn", summary: "Maturity approaching", detail, life: 10000 });
            nativeNotify(`${APP_NAME} — Maturity in ${a.daysUntilMaturity}d`, detail);
        }

        for (const a of soon30) {
            const label = a.source === "fd" ? "FD" : "Bond";
            const amt = a.maturityAmount ?? a.principal;
            const detail = `${a.name} ${label} of ${formatCompact(amt)} matures on ${a.maturityDate} (${a.daysUntilMaturity} days).`;
            toast.add({ severity: "info", summary: "Upcoming maturity", detail, life: 8000 });
        }
    }

    return { checkMaturity };
}
