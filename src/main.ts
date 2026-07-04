import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from "pinia";
import router from "./router";
import { useAnalytics } from "@/composables/useAnalytics";

import "./style.css";
import PrimeVue from "primevue/config";
import Aura from "@primeuix/themes/aura";
import { definePreset } from "@primeuix/themes";
import "primeicons/primeicons.css";
import ToastService from "primevue/toastservice";
import ConfirmationService from "primevue/confirmationservice";
import DialogService from "primevue/dialogservice";

const app = createApp(App);

app.use(createPinia());
app.use(router);

const SuvarixPreset = definePreset(Aura, {
    semantic: {
        primary: {
            50: "#eefbfb",
            100: "#d3f3f4",
            200: "#a8e7e9",
            300: "#71d4d8",
            400: "#3bb6bc",
            500: "#0d7377",
            600: "#0a5e62",
            700: "#094c4f",
            800: "#0a3d40",
            900: "#0b3335",
            950: "#041d1e",
        },
    },
});

app.use(PrimeVue, {
    theme: {
        preset: SuvarixPreset,
        options: { darkModeSelector: ".app-dark" },
    },
});
app.use(ToastService);
app.use(ConfirmationService);
app.use(DialogService);

app.config.errorHandler = (err, _vm, info) => {
    const { trackError } = useAnalytics();
    trackError("vue_error", String(err), info ?? undefined);
    console.error(err);
};

app.mount("#app");
