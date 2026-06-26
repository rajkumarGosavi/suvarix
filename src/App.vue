<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const router = useRouter();
const ready = ref(false);

onMounted(() => {
    ui.initTheme();
    router.isReady().then(() => { ready.value = true; });
});
</script>

<template>
  <RouterView v-if="ready" />
</template>
