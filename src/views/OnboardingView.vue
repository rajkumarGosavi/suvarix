<script setup lang="ts">
import { ref } from "vue";
import { APP_NAME } from "@/constants";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";
import { useUiStore, type Theme } from "@/stores/ui";

const router = useRouter();
const auth = useAuthStore();
const ui = useUiStore();

const TOTAL_STEPS = 4;
const step = ref(1);
const finishing = ref(false);

const THEME_OPTIONS: { label: string; value: Theme; icon: string }[] = [
    { label: "Light",  value: "light",  icon: "pi pi-sun" },
    { label: "System", value: "system", icon: "pi pi-desktop" },
    { label: "Dark",   value: "dark",   icon: "pi pi-moon" },
];

const FEATURES = [
    { icon: "pi pi-briefcase", label: "Portfolio Tracking" },
    { icon: "pi pi-flag",      label: "Goals" },
    { icon: "pi pi-receipt",   label: "Transactions" },
    { icon: "pi pi-credit-card", label: "Liabilities" },
    { icon: "pi pi-wallet",    label: "Income & Expenses" },
    { icon: "pi pi-link",      label: "Zerodha, Upstox & Angel One" },
    { icon: "pi pi-file-import", label: "CSV & XLSX Import" },
    { icon: "pi pi-file-pdf",  label: "MF CAS Import" },
    { icon: "pi pi-bell",      label: "Reminders" },
    { icon: "pi pi-calendar",  label: "Calendar View" },
];

function next() {
    if (step.value < TOTAL_STEPS) step.value++;
}

function back() {
    if (step.value > 1) step.value--;
}

async function finish() {
    finishing.value = true;
    try {
        await auth.completeOnboarding();
        router.push("/dashboard");
    } catch {
        router.push("/dashboard");
    } finally {
        finishing.value = false;
    }
}

async function skip() {
    try { await auth.completeOnboarding(); } catch { /* non-fatal */ }
    router.push("/dashboard");
}
</script>

<template>
    <div class="onboarding-page">
        <div class="onboarding-card fin-card">

            <!-- Skip -->
            <div class="skip-row">
                <button class="skip-btn" @click="skip" type="button">Skip</button>
            </div>

            <!-- Progress -->
            <div class="progress-track"
                 role="progressbar"
                 :aria-valuenow="step"
                 aria-valuemin="1"
                 :aria-valuemax="TOTAL_STEPS"
                 :aria-label="`Step ${step} of ${TOTAL_STEPS}`">
                <div class="progress-fill" :style="{ width: `${(step / TOTAL_STEPS) * 100}%` }" />
            </div>
            <p class="step-label">Step {{ step }} of {{ TOTAL_STEPS }}</p>

            <!-- Step 1: Welcome -->
            <div v-if="step === 1" class="step-content">
                <div class="step-logo">
                    <span class="logo-text logo-brand">{{ APP_NAME }}</span>
                    <p class="tagline">Your finances, on your machine.</p>
                </div>
                <h2>Welcome to {{ APP_NAME }}</h2>
                <p class="step-desc">
                    Built for Indian investors. Track your stocks, mutual funds, FDs,
                    gold, PPF, and more — all stored locally, with complete privacy.
                    No cloud. No subscriptions.
                </p>
                <div class="step-actions">
                    <Button label="Get Started" icon="pi pi-arrow-right" iconPos="right"
                            class="w-full" @click="next" />
                </div>
            </div>

            <!-- Step 2: Theme -->
            <div v-else-if="step === 2" class="step-content">
                <div class="step-icon" aria-hidden="true">
                    <i class="pi pi-palette" style="font-size:2rem;color:var(--p-primary-color)" />
                </div>
                <h2>Choose Your Theme</h2>
                <p class="step-desc">Pick the look that works best for you. Change anytime in Settings.</p>
                <div class="theme-picker">
                    <SelectButton
                        :modelValue="ui.theme"
                        @update:modelValue="ui.setTheme($event as Theme)"
                        :options="THEME_OPTIONS"
                        optionLabel="label"
                        optionValue="value"
                        aria-label="Theme"
                    >
                        <template #option="{ option }">
                            <i :class="option.icon" aria-hidden="true" style="margin-right:0.4rem" />{{ option.label }}
                        </template>
                    </SelectButton>
                </div>
                <div class="step-actions">
                    <Button label="Back" outlined @click="back" />
                    <Button label="Next" icon="pi pi-arrow-right" iconPos="right" @click="next" />
                </div>
            </div>

            <!-- Step 3: Features -->
            <div v-else-if="step === 3" class="step-content">
                <h2>Everything You Need</h2>
                <p class="step-desc">{{ APP_NAME }} covers every corner of your financial life.</p>
                <div class="feature-grid" role="list">
                    <div
                        v-for="f in FEATURES"
                        :key="f.label"
                        class="feature-chip"
                        role="listitem"
                    >
                        <i :class="f.icon" class="feature-icon" aria-hidden="true" />
                        <span class="feature-label">{{ f.label }}</span>
                    </div>
                </div>
                <div class="step-actions">
                    <Button label="Back" outlined @click="back" />
                    <Button label="Next" icon="pi pi-arrow-right" iconPos="right" @click="next" />
                </div>
            </div>

            <!-- Step 4: Done -->
            <div v-else-if="step === 4" class="step-content">
                <div class="step-icon" aria-hidden="true">
                    <i class="pi pi-check-circle" style="font-size:3rem;color:var(--p-green-500)" />
                </div>
                <h2>You're All Set!</h2>
                <p class="step-desc">
                    Your data stays on this device — always. Start by adding your first
                    holding, or explore the dashboard to get a feel for the app.
                </p>
                <div class="step-actions">
                    <Button label="Back" outlined @click="back" />
                    <Button label="Go to Dashboard" icon="pi pi-home" iconPos="right"
                            :loading="finishing" class="cta-btn" @click="finish" />
                </div>
            </div>

        </div>
    </div>
</template>

<style scoped>
.onboarding-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
}

.onboarding-card {
    width: 100%;
    max-width: 480px;
    padding: 2.5rem;
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
}

/* Progress */
.progress-track {
    height: 4px;
    background: var(--p-content-border-color);
    border-radius: 2px;
    overflow: hidden;
}
.progress-fill {
    height: 100%;
    background: var(--p-primary-color);
    border-radius: 2px;
    transition: width 0.3s ease;
}
.step-label {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
    margin: 0;
}

/* Step content */
.step-content {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

/* Step 1 */
.step-logo { text-align: center; }
.logo-text { font-size: 2rem; font-weight: 800; }
.tagline { font-size: 0.9rem; color: var(--p-text-muted-color); margin: 0.25rem 0 0; }

/* Step icon */
.step-icon { text-align: center; }

h2 { margin: 0; font-size: 1.3rem; font-weight: 700; }
.step-desc { margin: 0; font-size: 0.9rem; color: var(--p-text-muted-color); line-height: 1.5; }

/* Theme picker */
.theme-picker { display: flex; justify-content: center; }

/* Feature grid */
.feature-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    max-height: 300px;
    overflow-y: auto;
}
.feature-chip {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.75rem;
    border-radius: 8px;
    background: var(--p-surface-ground);
    border: 1px solid var(--p-content-border-color);
    font-size: 0.82rem;
    font-weight: 500;
}
.feature-icon {
    color: var(--p-primary-color);
    font-size: 1rem;
    flex-shrink: 0;
}
.feature-label { line-height: 1.3; }

/* Actions */
.step-actions {
    display: flex;
    gap: 0.75rem;
    justify-content: flex-end;
    margin-top: 0.25rem;
}
.cta-btn { flex: 1; }

/* Skip */
.skip-row {
    display: flex;
    justify-content: flex-end;
}
.skip-btn {
    background: none;
    border: none;
    padding: 0;
    font-size: 0.82rem;
    color: var(--p-text-muted-color);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
}
.skip-btn:hover {
    color: var(--p-text-color);
}
.skip-btn:focus-visible {
    outline: 2px solid var(--p-primary-color);
    outline-offset: 2px;
    border-radius: 2px;
}

@media (max-width: 639px) {
    .onboarding-card { padding: 1.5rem; }
    .feature-grid { grid-template-columns: 1fr; max-height: 240px; }
    .step-actions { flex-direction: column-reverse; }
    .step-actions :deep(.p-button) { width: 100%; }
    .cta-btn { flex: none; }
}
</style>
