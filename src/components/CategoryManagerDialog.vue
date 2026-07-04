<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";
import { useCategoriesStore } from "@/stores/categories";

const visible = defineModel<boolean>("visible", { default: false });
const store = useCategoriesStore();
const confirm = useConfirm();
const toast = useToast();

const newName = ref("");
const editingId = ref<number | null>(null);
const editingName = ref("");
const busy = ref(false);

onMounted(() => store.fetchCategories());

function showError(prefix: string, e: any) {
    toast.add({ severity: "error", summary: prefix, detail: String(e?.message ?? e), life: 5000 });
}

async function addCategory() {
    const name = newName.value.trim();
    if (!name) return;
    busy.value = true;
    try {
        await store.addCategory(name);
        newName.value = "";
    } catch (e: any) {
        showError("Couldn't add category", e);
    } finally {
        busy.value = false;
    }
}

function startEdit(cat: { id: number; name: string }) {
    editingId.value = cat.id;
    editingName.value = cat.name;
}

async function saveEdit(id: number) {
    const name = editingName.value.trim();
    editingId.value = null;
    if (!name) return;
    try {
        await store.updateCategory(id, name);
    } catch (e: any) {
        showError("Couldn't rename category", e);
    }
}

function confirmDelete(cat: { id: number; name: string }) {
    confirm.require({
        message: `Delete category "${cat.name}"?`,
        header: "Delete Category",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: async () => {
            try {
                await store.deleteCategory(cat.id);
            } catch (e: any) {
                showError("Can't delete category", e);
            }
        },
    });
}
</script>

<template>
    <Dialog v-model:visible="visible" header="Manage Categories" modal style="width:420px">
        <div class="add-row">
            <InputText v-model="newName" placeholder="New category name" class="w-full" @keyup.enter="addCategory" />
            <Button icon="pi pi-plus" :loading="busy" aria-label="Add category" @click="addCategory" />
        </div>
        <ul class="category-list">
            <li v-for="cat in store.categories" :key="cat.id" class="category-row">
                <InputText
                    v-if="editingId === cat.id"
                    v-model="editingName"
                    class="w-full"
                    autofocus
                    @keyup.enter="saveEdit(cat.id)"
                    @blur="saveEdit(cat.id)"
                />
                <span v-else class="category-name" @click="startEdit(cat)">{{ cat.name }}</span>
                <Button icon="pi pi-trash" text size="small" aria-label="Delete category" @click="confirmDelete(cat)" />
            </li>
        </ul>
        <p v-if="!store.categories.length" class="empty-hint">No categories yet.</p>
    </Dialog>
</template>

<style scoped>
.add-row { display: flex; gap: 0.5rem; margin-bottom: 1rem; }
.category-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.4rem; max-height: 320px; overflow-y: auto; }
.category-row { display: flex; align-items: center; gap: 0.5rem; padding: 0.4rem 0.5rem; border-radius: 8px; }
.category-row:hover { background: color-mix(in srgb, var(--p-primary-color) 6%, transparent); }
.category-name { flex: 1; cursor: pointer; font-size: 0.9rem; }
.empty-hint { font-size: 0.85rem; color: var(--p-text-muted-color); text-align: center; padding: 1rem 0; }
</style>
