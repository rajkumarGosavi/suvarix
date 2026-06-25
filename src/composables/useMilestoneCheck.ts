import { invoke } from "@tauri-apps/api/core";
import { useToast } from "primevue/usetoast";
import { useNotifications } from "@/composables/useNotifications";

interface MilestoneHit { id: number; amount: number; label: string; }

export function useMilestoneCheck() {
    const toast = useToast();
    const { nativeNotify } = useNotifications();

    async function checkMilestones(netWorth: number) {
        const hits = await invoke<MilestoneHit[]>("check_milestones", { netWorth }).catch(() => []);
        for (const m of hits) {
            toast.add({
                severity: "success",
                summary: "Milestone reached!",
                detail: `Your net worth crossed ${m.label} 🎉`,
                life: 12000,
            });
            nativeNotify("FinFolio — Milestone reached!", `Your net worth crossed ${m.label}! 🎉`);
        }
    }

    return { checkMilestones };
}
