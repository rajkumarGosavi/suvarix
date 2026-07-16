import { defineBrokerStore } from "@/stores/broker";

export const useUpstoxStore = defineBrokerStore("upstox", {
    status: "get_upstox_status",
    saveConfig: "save_upstox_config",
    connect: "start_upstox_login",
    sync: "sync_upstox_holdings",
    disconnect: "disconnect_upstox",
});
