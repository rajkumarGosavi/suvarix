<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { useUiStore } from "@/stores/ui";
import { useAppUpdater } from "@/composables/useAppUpdater";

const ui = useUiStore();
const router = useRouter();
const ready = ref(false);
const { checkForUpdate } = useAppUpdater();

onMounted(() => {
    ui.initTheme();
    router.isReady().then(() => {
        ready.value = true;
        setTimeout(checkForUpdate, 3000);
    });
});
</script>

<template>
  <Toast />
  <ConfirmDialog />
  <RouterView v-if="ready" />
</template>
