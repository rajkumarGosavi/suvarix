import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { usePortfolioStore } from "@/stores/portfolio";

export function useHoldingCrud(addCmd: string, updateCmd: string, deleteCmd: string, fetchFn: () => Promise<void>) {
    const showDialog = ref(false);
    const editItem = ref<any>(null);
    const loading = ref(false);
    const portfolio = usePortfolioStore();

    function openAdd() {
        editItem.value = null;
        showDialog.value = true;
    }

    function openEdit(item: any) {
        editItem.value = { ...item };
        showDialog.value = true;
    }

    function close() {
        showDialog.value = false;
        editItem.value = null;
    }

    async function save(payload: object) {
        loading.value = true;
        try {
            if (editItem.value?.id) {
                await invoke(updateCmd, { id: editItem.value.id, payload });
            } else {
                await invoke(addCmd, { payload });
            }
            await fetchFn();
            await portfolio.fetchNetWorth();
            await portfolio.fetchAllocation();
            close();
        } finally {
            loading.value = false;
        }
    }

    async function remove(id: number) {
        loading.value = true;
        try {
            await invoke(deleteCmd, { id });
            await fetchFn();
            await portfolio.fetchNetWorth();
            await portfolio.fetchAllocation();
        } finally {
            loading.value = false;
        }
    }

    return { showDialog, editItem, loading, openAdd, openEdit, close, save, remove };
}
