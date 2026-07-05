<script setup lang="ts">
import { ref } from "vue";
import { APP_NAME } from "@/constants";
import { EULA_TEXT } from "@/eulaText";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = useRouter();
const auth = useAuthStore();

const password = ref("");
const confirm = ref("");
const error = ref("");
const loading = ref(false);
const eulaAccepted = ref(false);
const showEula = ref(false);

async function submit() {
    error.value = "";
    if (!eulaAccepted.value) {
        error.value = "You must agree to the End User License Agreement to continue.";
        return;
    }
    if (password.value.length < 8) {
        error.value = "Password must be at least 8 characters.";
        return;
    }
    if (password.value !== confirm.value) {
        error.value = "Passwords do not match.";
        return;
    }
    loading.value = true;
    try {
        await auth.setup(password.value);
        router.push("/onboarding");
    } catch (e: any) {
        error.value = e?.message?.message ?? "Setup failed.";
    } finally {
        loading.value = false;
    }
}
</script>

<template>
    <div class="setup-page">
        <div class="setup-card fin-card">
            <div class="setup-logo">
                <span class="logo-text logo-brand">{{ APP_NAME }}</span>
                <p class="tagline">Your finances, on your machine.</p>
            </div>

            <h2>Create Master Password</h2>
            <p class="hint">This password encrypts your data locally. There is no recovery option — keep it safe.</p>

            <form @submit.prevent="submit" class="setup-form">
                <div class="field">
                    <label for="password">Password</label>
                    <Password
                        id="password"
                        v-model="password"
                        :feedback="true"
                        toggleMask
                        fluid
                        placeholder="Enter password"
                        :inputProps="{ autocomplete: 'new-password' }"
                    />
                </div>
                <div class="field">
                    <label for="confirm">Confirm Password</label>
                    <Password
                        id="confirm"
                        v-model="confirm"
                        :feedback="false"
                        toggleMask
                        fluid
                        placeholder="Confirm password"
                        :inputProps="{ autocomplete: 'new-password' }"
                    />
                </div>
                <div class="field eula-field">
                    <Checkbox v-model="eulaAccepted" inputId="eula" binary />
                    <label for="eula" class="eula-label">
                        I agree to the
                        <a href="#" @click.prevent="showEula = true">End User License Agreement</a>
                    </label>
                </div>
                <Message v-if="error" severity="error">{{ error }}</Message>
                <Button type="submit" label="Create Password & Get Started" :loading="loading" :disabled="!eulaAccepted" class="w-full" />
            </form>
        </div>

        <Dialog v-model:visible="showEula" header="End User License Agreement" modal style="width:600px; max-width:90vw">
            <pre class="eula-text">{{ EULA_TEXT }}</pre>
            <template #footer>
                <Button label="Close" @click="showEula = false" />
            </template>
        </Dialog>
    </div>
</template>

<style scoped>
.setup-page {
    min-height: 100dvh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
}

.setup-card {
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 420px;
}

.setup-logo {
    text-align: center;
    margin-bottom: 1.5rem;
}

.logo-text {
    font-size: 2rem;
    font-weight: 800;
}

.tagline {
    margin-top: 0.25rem;
    font-size: 0.9rem;
    color: var(--p-text-muted-color);
}

h2 {
    margin: 0 0 0.5rem;
    font-size: 1.2rem;
}

.hint {
    font-size: 0.85rem;
    margin-bottom: 1.5rem;
    color: var(--p-text-muted-color);
}

.setup-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
}

label {
    font-size: 0.875rem;
    font-weight: 500;
}

.eula-field {
    flex-direction: row;
    align-items: center;
    gap: 0.6rem;
}

.eula-label {
    font-weight: 400;
    font-size: 0.85rem;
}

.eula-label a {
    color: var(--p-primary-color);
    text-decoration: underline;
}

.eula-text {
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.85rem;
    line-height: 1.5;
    max-height: 60vh;
    overflow-y: auto;
    margin: 0;
}
</style>
