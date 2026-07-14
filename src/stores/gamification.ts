import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface Badge {
    id: string;
    name: string;
    description: string;
    icon: string;
    xpReward: number;
    earnedAt: string | null;
}

export interface StreakInfo {
    streakType: string;
    currentCount: number;
    bestCount: number;
    lastActivityDate: string | null;
}

export interface GamificationStats {
    totalXp: number;
    level: string;
    levelProgressPct: number;
    nextLevelXpNeeded: number;
    badges: Badge[];
    streaks: StreakInfo[];
}

export interface XpAwardResult {
    newXp: number;
    levelChanged: boolean;
    newLevel: string;
    newBadges: Badge[];
}

export interface StreakUpdateResult {
    currentCount: number;
    bestCount: number;
    isNewBest: boolean;
    streakBonusXp: number;
}

export interface SavingsStreak {
    currentStreak: number;
    bestStreak: number;
    thisMonthSaved: number;
    thisMonthPositive: boolean;
    xpAwarded: number;
}

export const useGamificationStore = defineStore("gamification", {
    state: () => ({
        stats: null as GamificationStats | null,
        savingsStreak: null as SavingsStreak | null,
        isLoading: false,
    }),
    getters: {
        earnedBadges: (state): Badge[] =>
            state.stats?.badges.filter((b) => b.earnedAt != null) ?? [],
        recentBadges: (state): Badge[] => {
            const earned = state.stats?.badges.filter((b) => b.earnedAt != null) ?? [];
            return earned.slice(0, 3);
        },
        txStreak: (state): StreakInfo | null =>
            state.stats?.streaks.find((s) => s.streakType === "transaction") ?? null,
    },
    actions: {
        async fetch() {
            this.isLoading = true;
            try {
                await invoke("bootstrap_gamification").catch(() => {});
                this.stats = await invoke<GamificationStats>("get_gamification_stats");
                // Computed behaviour streak (consecutive positive-savings months).
                this.savingsStreak = await invoke<SavingsStreak>("get_savings_streak").catch(() => null);
            } catch {
                // non-fatal
            } finally {
                this.isLoading = false;
            }
        },
        async awardXpAndRefresh(reason: string, amount: number): Promise<XpAwardResult> {
            const result = await invoke<XpAwardResult>("award_xp", { reason, amount });
            await this.fetch();
            return result;
        },
        async updateStreakAndRefresh(streakType: string): Promise<StreakUpdateResult> {
            const result = await invoke<StreakUpdateResult>("update_streak", { streakType });
            await this.fetch();
            return result;
        },
    },
});
