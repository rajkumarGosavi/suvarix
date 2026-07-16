import { defineBrokerStore } from "@/stores/broker";

export const useZerodhaStore = defineBrokerStore("zerodha", {
    status: "get_zerodha_status",
    saveConfig: "save_zerodha_config",
    connect: "start_zerodha_login",
    sync: "sync_zerodha_holdings",
    disconnect: "disconnect_zerodha",
});
