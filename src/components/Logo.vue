<script setup lang="ts">
import { APP_NAME } from "@/constants";
import markUrl from "@/assets/brand/mark.svg";

withDefaults(defineProps<{ showWordmark?: boolean; size?: number }>(), {
    showWordmark: true,
    size: 28,
});
</script>

<template>
    <span class="logo-lockup">
        <img :src="markUrl" class="logo-mark" :style="{ width: size + 'px', height: size + 'px' }" alt="" />
        <span v-if="showWordmark" class="brand-name logo-brand">{{ APP_NAME }}</span>
    </span>
</template>

<style scoped>
.logo-lockup {
    display: inline-flex;
    align-items: center;
    gap: 8px;
}

.logo-mark {
    flex-shrink: 0;
}
</style>
