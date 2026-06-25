<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = useRouter();
const auth = useAuthStore();

const password = ref("");
const confirm = ref("");
const error = ref("");
const loading = ref(false);

async function submit() {
    error.value = "";
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
        router.push("/dashboard");
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
                <span class="logo-text logo-brand">FinFolio</span>
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
                <Message v-if="error" severity="error">{{ error }}</Message>
                <Button type="submit" label="Create Password & Get Started" :loading="loading" class="w-full" />
            </form>
        </div>
    </div>
</template>

<style scoped>
.setup-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
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
}

h2 {
    margin: 0 0 0.5rem;
    font-size: 1.2rem;
}

.hint {
    font-size: 0.85rem;
    margin-bottom: 1.5rem;
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
</style>
