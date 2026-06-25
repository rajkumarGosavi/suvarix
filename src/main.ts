import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from "pinia";
import router from "./router";
import { useAnalytics } from "@/composables/useAnalytics";

import "./style.css";
import PrimeVue from "primevue/config";
import Aura from "@primeuix/themes/aura";
import "primeicons/primeicons.css";
import ToastService from "primevue/toastservice";
import ConfirmationService from "primevue/confirmationservice";
import DialogService from "primevue/dialogservice";

const app = createApp(App);

app.use(createPinia());
app.use(router);

app.use(PrimeVue, {
    theme: {
        preset: Aura,
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
