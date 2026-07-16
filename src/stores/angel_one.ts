import { defineBrokerStore } from "@/stores/broker";

export const useAngelOneStore = defineBrokerStore("angel_one", {
    status: "get_angel_status",
    saveConfig: "save_angel_config",
    connect: "login_angel",
    sync: "sync_angel_holdings",
    disconnect: "disconnect_angel",
});
