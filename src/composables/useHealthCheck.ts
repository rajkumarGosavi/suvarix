import { useFinancialHealthStore } from "@/stores/financialHealth";
import { useGamificationSafe } from "@/composables/useGamification";

// Max XP a single day's score improvement can earn — keeps rewards improvement-only
// and grind-resistant (you can't farm XP by toggling data back and forth).
const MAX_IMPROVEMENT_XP = 15;

/**
 * Refreshes the Financial Health Score, records today's daily snapshot, and —
 * only when gamification is enabled — awards improvement-only XP and health
 * milestone badges. Safe to call on every Dashboard mount: XP is granted once
 * per day and only when the score genuinely rose versus the last recorded day.
 */
export function useHealthCheck() {
    const health = useFinancialHealthStore();
    const gamification = useGamificationSafe();

    async function runHealthCheck() {
        await health.fetch();
        const score = health.score;
        if (!score) return;

        const snap = await health.recordSnapshot(score.overall);

        // Improvement-only XP: first record of the day, and strictly higher than the
        // last recorded day. previousScore is null on the very first run (no reward).
        if (
            snap &&
            !snap.alreadyRecordedToday &&
            snap.previousScore != null &&
            score.overall > snap.previousScore
        ) {
            const gained = Math.min(
                Math.round(score.overall - snap.previousScore),
                MAX_IMPROVEMENT_XP,
            );
            if (gained > 0) {
                await gamification.awardXP("health_improved", gained);
            }
        }

        // Milestone badges — flags set only when the real threshold is crossed.
        const emergency = score.pillars.find((p) => p.key === "emergency");
        const debt = score.pillars.find((p) => p.key === "debt");
        await gamification.checkBadges({
            checkHealthA: score.overall >= 70,
            checkHealthAplus: score.overall >= 85,
            checkEmergencyReady: (emergency?.score ?? 0) >= 100,
            checkDebtLight: (debt?.score ?? 0) >= 100,
        });
    }

    return { runHealthCheck };
}
