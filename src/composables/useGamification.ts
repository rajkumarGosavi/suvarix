import { invoke } from "@tauri-apps/api/core";
import { useToast } from "primevue/usetoast";
import { useGamificationStore } from "@/stores/gamification";
import type { Badge } from "@/stores/gamification";

export interface BadgeCheckContext {
    checkFirstInvestment?: boolean;
    checkFirstGoal?: boolean;
    checkFirstMilestone?: boolean;
    checkDiversification?: boolean;
    checkDebtDestroyer?: boolean;
    checkCroreClub?: boolean;
    checkHealthA?: boolean;
    checkHealthAplus?: boolean;
    checkEmergencyReady?: boolean;
    checkDebtLight?: boolean;
    checkFirstLakh?: boolean;
    checkTenLakh?: boolean;
    checkSavingsStar?: boolean;
}

const NOOP = async () => {};
const NOOP_SYNC = () => {};

export function useGamificationSafe() {
    if (import.meta.env.VITE_GAMIFICATION !== "true") {
        return {
            awardXP: NOOP as (reason: string, amount: number) => Promise<void>,
            updateStreak: NOOP as (type: "bill_payment" | "transaction") => Promise<void>,
            checkBadges: NOOP as (context: BadgeCheckContext) => Promise<void>,
            celebrate: NOOP_SYNC,
        };
    }
    return useGamification();
}

export function useGamification() {
    const store = useGamificationStore();
    const toast = useToast();

    function celebrate() {
        import("canvas-confetti").then(({ default: confetti }) => {
            confetti({
                particleCount: 120,
                spread: 80,
                origin: { y: 0.6 },
                colors: ["#22c55e", "#3b82f6", "#f59e0b", "#ec4899", "#8b5cf6"],
            });
        });
    }

    function showBadgeToast(badge: Badge) {
        toast.add({
            severity: "info",
            summary: `Badge Earned: ${badge.name}`,
            detail: `${badge.icon} ${badge.description}`,
            life: 7000,
        });
    }

    async function awardXP(reason: string, amount: number) {
        try {
            const result = await store.awardXpAndRefresh(reason, amount);
            if (result.levelChanged) {
                toast.add({
                    severity: "success",
                    summary: "Level Up!",
                    detail: `You are now a ${result.newLevel}! Keep going!`,
                    life: 8000,
                });
                celebrate();
            }
            for (const badge of result.newBadges) {
                showBadgeToast(badge);
            }
        } catch {
            // non-fatal
        }
    }

    async function updateStreak(type: "bill_payment" | "transaction") {
        try {
            const result = await store.updateStreakAndRefresh(type);
            const milestones = [7, 14, 30, 60, 90];
            if (result.currentCount > 0 && milestones.includes(result.currentCount)) {
                const label = type === "bill_payment" ? "Bill Payment" : "Transaction";
                toast.add({
                    severity: "success",
                    summary: `${result.currentCount}-Day Streak!`,
                    detail: `${label} streak milestone hit! +${result.streakBonusXp > 0 ? result.streakBonusXp : 0} XP bonus!`,
                    life: 7000,
                });
                if (result.currentCount === 7 || result.currentCount === 30) {
                    celebrate();
                }
            }
        } catch {
            // non-fatal
        }
    }

    async function checkBadges(context: BadgeCheckContext) {
        try {
            const newBadges = await invoke<Badge[]>("check_and_award_badges", {
                context: {
                    checkFirstInvestment: context.checkFirstInvestment ?? false,
                    checkFirstGoal: context.checkFirstGoal ?? false,
                    checkFirstMilestone: context.checkFirstMilestone ?? false,
                    checkDiversification: context.checkDiversification ?? false,
                    checkDebtDestroyer: context.checkDebtDestroyer ?? false,
                    checkCroreClub: context.checkCroreClub ?? false,
                    checkHealthA: context.checkHealthA ?? false,
                    checkHealthAplus: context.checkHealthAplus ?? false,
                    checkEmergencyReady: context.checkEmergencyReady ?? false,
                    checkDebtLight: context.checkDebtLight ?? false,
                    checkFirstLakh: context.checkFirstLakh ?? false,
                    checkTenLakh: context.checkTenLakh ?? false,
                    checkSavingsStar: context.checkSavingsStar ?? false,
                },
            });
            await store.fetch();
            for (const badge of newBadges) {
                showBadgeToast(badge);
                celebrate();
            }
        } catch {
            // non-fatal
        }
    }

    return { awardXP, updateStreak, celebrate, checkBadges };
}
