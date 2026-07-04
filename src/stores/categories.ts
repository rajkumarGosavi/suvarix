import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface Category {
    id: number;
    name: string;
    createdAt: string;
}

export const useCategoriesStore = defineStore("categories", {
    state: () => ({
        categories: [] as Category[],
        loading: false,
    }),
    getters: {
        names: (state) => state.categories.map((c) => c.name),
    },
    actions: {
        async fetchCategories() {
            this.loading = true;
            try {
                this.categories = await invoke<Category[]>("list_categories");
            } finally {
                this.loading = false;
            }
        },
        async addCategory(name: string) {
            await invoke("add_category", { payload: { name } });
            await this.fetchCategories();
        },
        async updateCategory(id: number, name: string) {
            await invoke("update_category", { id, payload: { name } });
            await this.fetchCategories();
        },
        async deleteCategory(id: number) {
            await invoke("delete_category", { id });
            await this.fetchCategories();
        },
    },
});
