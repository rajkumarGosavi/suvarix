<script setup lang="ts">
import { ref } from "vue";
import { APP_NAME } from "@/constants";
import { EULA_TEXT } from "@/eulaText";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = useRouter();
const auth = useAuthStore();

const accepting = ref(false);

async function accept() {
    accepting.value = true;
    try {
        await auth.acceptEula();
        router.push("/dashboard");
    } finally {
        accepting.value = false;
    }
}

async function decline() {
    await auth.lock();
    router.push("/unlock");
}
</script>

<template>
    <div class="eula-page">
        <div class="eula-card fin-card">
            <div class="eula-header">
                <span class="logo-text logo-brand">{{ APP_NAME }}</span>
                <h2>Terms Have Been Updated</h2>
                <p class="hint">
                    The End User License Agreement has changed since you last agreed to it.
                    Please review and accept the updated terms to continue.
                </p>
            </div>

            <pre class="eula-text">{{ EULA_TEXT }}</pre>

            <div class="eula-actions">
                <Button label="Decline & Lock" severity="secondary" outlined @click="decline" />
                <Button label="I Agree, Continue" :loading="accepting" @click="accept" />
            </div>
        </div>
    </div>
</template>

<style scoped>
.eula-page {
    min-height: 100dvh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
}

.eula-card {
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 600px;
}

.eula-header {
    text-align: center;
    margin-bottom: 1.5rem;
}

.logo-text {
    font-size: 1.75rem;
    font-weight: 800;
}

h2 {
    margin: 0.75rem 0 0.5rem;
    font-size: 1.15rem;
}

.hint {
    font-size: 0.875rem;
    margin: 0;
    color: var(--p-text-muted-color);
}

.eula-text {
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.85rem;
    line-height: 1.5;
    max-height: 40vh;
    overflow-y: auto;
    margin: 0 0 1.5rem;
    padding: 1rem;
    border: 1px solid var(--p-content-border-color);
    border-radius: 8px;
}

.eula-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
}
</style>
