<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = useRouter();
const auth = useAuthStore();

const password = ref("");
const error = ref("");
const loading = ref(false);

async function unlock() {
    error.value = "";
    loading.value = true;
    try {
        const ok = await auth.unlock(password.value);
        if (ok) {
            router.push("/dashboard");
        } else {
            error.value = "Incorrect password.";
            password.value = "";
        }
    } catch {
        error.value = "An error occurred. Please try again.";
    } finally {
        loading.value = false;
    }
}
</script>

<template>
    <div class="unlock-page">
        <div class="unlock-card">
            <div class="lock-icon">
                <i class="pi pi-lock" style="font-size: 2.5rem" />
            </div>
            <h2>FinFolio is Locked</h2>
            <p class="hint">Enter your master password to continue.</p>

            <form @submit.prevent="unlock" class="unlock-form">
                <Password
                    v-model="password"
                    :feedback="false"
                    toggleMask
                    fluid
                    placeholder="Master password"
                    :inputProps="{ autocomplete: 'current-password', autofocus: true }"
                />
                <Message v-if="error" severity="error">{{ error }}</Message>
                <Button type="submit" label="Unlock" :loading="loading" class="w-full" />
            </form>
        </div>
    </div>
</template>

<style scoped>
.unlock-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
}

.unlock-card {
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 380px;
    text-align: center;
}

.lock-icon {
    margin-bottom: 1rem;
}

h2 {
    margin: 0 0 0.5rem;
    font-size: 1.3rem;
}

.hint {
    font-size: 0.875rem;
    margin-bottom: 1.5rem;
}

.unlock-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}
</style>
