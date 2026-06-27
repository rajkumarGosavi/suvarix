import { invoke } from "@tauri-apps/api/core";
import { APP_NAME } from "@/constants";
import { useToast } from "primevue/usetoast";
import { useNotifications } from "@/composables/useNotifications";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

interface GoalAchievement {
    id: number;
    name: string;
    targetAmount: number;
    category: string;
}

export function useGoalCheck() {
    const toast = useToast();
    const { nativeNotify } = useNotifications();
    const { formatCompact } = useCurrencyFormat();

    async function checkGoals(totalAssets: number) {
        const hits = await invoke<GoalAchievement[]>("check_goal_achievements", { totalAssets }).catch(() => []);
        for (const g of hits) {
            const detail = `You've reached your "${g.name}" goal of ${formatCompact(g.targetAmount)}! 🎉`;
            toast.add({ severity: "success", summary: "Goal achieved!", detail, life: 12000 });
            nativeNotify(`${APP_NAME} — Goal achieved!`, detail);
        }
    }

    return { checkGoals };
}
