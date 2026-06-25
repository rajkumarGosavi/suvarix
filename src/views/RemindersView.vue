<script setup lang="ts">
import { ref, reactive, computed, onMounted } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";
import { useRemindersStore, type BillPayload, type RecurringTxPayload, type UpcomingReminder, type RecurringTx } from "@/stores/reminders";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";

const store = useRemindersStore();
const confirm = useConfirm();
const toast = useToast();
const { formatINR } = useCurrencyFormat();

const activeTab = ref(0);
const filterDays = ref(30);

const TX_TYPES = ["income","expense","emi","sip","buy","sell","dividend","interest","redemption","deposit","withdrawal","transfer"];
const ASSET_CLASSES = ["equity","mf","fd","ppf_epf","real_estate","gold","crypto","insurance","cash","loan","credit_card"];
const CATEGORIES = ["Food","Rent","EMI","Travel","Medical","Utilities","Entertainment","Education","Shopping","Dividend","Interest","Salary","Other"];
const BILL_CATEGORIES = ["utilities","rent","subscription","insurance","emi","tax","other"];
const FREQUENCIES_BILL = ["weekly","monthly","quarterly","yearly","one_time"];
const FREQUENCIES_RECURRING = ["daily","weekly","monthly","yearly"];

// ─── Upcoming Bills ──────────────────────────────────────────────────────────

async function changeFilter(days: number) {
    filterDays.value = days;
    await store.loadUpcoming(days);
}

function urgencyClass(days: number) {
    if (days < 0) return "overdue";
    if (days <= 7) return "soon";
    return "normal";
}

function urgencySeverity(days: number): "danger" | "warn" | "success" {
    if (days < 0) return "danger";
    if (days <= 7) return "warn";
    return "success";
}

function sourceIcon(source: string) {
    if (source === "loan") return "pi pi-building";
    if (source === "credit_card") return "pi pi-credit-card";
    return "pi pi-receipt";
}

function dueDateLabel(days: number) {
    if (days < 0) return `${Math.abs(days)}d overdue`;
    if (days === 0) return "Due today";
    if (days === 1) return "Due tomorrow";
    return `${days}d left`;
}

// Mark Paid dialog
const showPayDialog = ref(false);
const payTarget = ref<UpcomingReminder | null>(null);
const payForm = reactive({ amount: 0, date: new Date(), notes: "" });
const payLoading = ref(false);

function openMarkPaid(r: UpcomingReminder) {
    payTarget.value = r;
    payForm.amount = r.amount;
    payForm.date = new Date();
    payForm.notes = "";
    showPayDialog.value = true;
}

async function submitMarkPaid() {
    if (!payTarget.value) return;
    payLoading.value = true;
    try {
        await store.markPaid(
            payTarget.value.source,
            payTarget.value.sourceId,
            payForm.amount,
            dateToStr(payForm.date) ?? new Date().toISOString().split("T")[0],
            payForm.notes || null,
        );
        toast.add({ severity: "success", summary: "Marked paid", detail: `${payTarget.value.name} recorded.`, life: 3000 });
        showPayDialog.value = false;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Failed", detail: String(e?.message ?? e), life: 4000 });
    } finally {
        payLoading.value = false;
    }
}

// ─── Bill CRUD ───────────────────────────────────────────────────────────────

const showBillDialog = ref(false);
const editBillId = ref<number | null>(null);
const billLoading = ref(false);

interface BillForm { name: string; category: string; amount: number; frequency: string; nextDueDate: Date | null; notes: string; }
const billForm = reactive<BillForm>({ name: "", category: "utilities", amount: 0, frequency: "monthly", nextDueDate: null, notes: "" });

function resetBillForm() {
    billForm.name = ""; billForm.category = "utilities"; billForm.amount = 0;
    billForm.frequency = "monthly"; billForm.nextDueDate = new Date(); billForm.notes = "";
}

function openAddBill() { editBillId.value = null; resetBillForm(); showBillDialog.value = true; }

function openEditBill(b: any) {
    editBillId.value = b.id; billForm.name = b.name; billForm.category = b.category;
    billForm.amount = b.amount; billForm.frequency = b.frequency;
    billForm.nextDueDate = strToDate(b.nextDueDate); billForm.notes = b.notes ?? "";
    showBillDialog.value = true;
}

async function saveBill() {
    billLoading.value = true;
    try {
        const payload: BillPayload = {
            name: billForm.name, category: billForm.category, amount: billForm.amount,
            frequency: billForm.frequency,
            nextDueDate: dateToStr(billForm.nextDueDate) ?? new Date().toISOString().split("T")[0],
            notes: billForm.notes || null,
        };
        if (editBillId.value) { await store.updateBill(editBillId.value, payload); }
        else { await store.addBill(payload); }
        await store.loadUpcoming(filterDays.value);
        toast.add({ severity: "success", summary: "Saved", detail: "Bill updated.", life: 2500 });
        showBillDialog.value = false;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Error", detail: String(e?.message ?? e), life: 4000 });
    } finally { billLoading.value = false; }
}

function deleteBill(id: number, name: string) {
    confirm.require({
        message: `Delete bill "${name}"?`,
        header: "Delete Bill",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", severity: "secondary", outlined: true },
        acceptProps: { label: "Delete", severity: "danger" },
        accept: async () => {
            await store.deleteBill(id);
            await store.loadUpcoming(filterDays.value);
            toast.add({ severity: "success", summary: "Deleted", life: 2000 });
        },
    });
}

// ─── Recurring Transactions ──────────────────────────────────────────────────

const showRecurDialog = ref(false);
const editRecurId = ref<number | null>(null);
const recurLoading = ref(false);
const showApplyDialog = ref(false);
const selectedDueIds = ref<number[]>([]);
const applyLoading = ref(false);

interface RecurForm {
    name: string; type: string; amount: number; category: string; assetClass: string;
    description: string; notes: string; frequency: string; nextDueDate: Date | null;
}
const recurForm = reactive<RecurForm>({
    name: "", type: "expense", amount: 0, category: "Other", assetClass: "",
    description: "", notes: "", frequency: "monthly", nextDueDate: null,
});

function resetRecurForm() {
    Object.assign(recurForm, { name: "", type: "expense", amount: 0, category: "Other",
        assetClass: "", description: "", notes: "", frequency: "monthly", nextDueDate: new Date() });
}

function openAddRecur() { editRecurId.value = null; resetRecurForm(); showRecurDialog.value = true; }

function openEditRecur(r: RecurringTx) {
    editRecurId.value = r.id; recurForm.name = r.name; recurForm.type = r.type;
    recurForm.amount = r.amount; recurForm.category = r.category;
    recurForm.assetClass = r.assetClass ?? ""; recurForm.description = r.description ?? "";
    recurForm.notes = r.notes ?? ""; recurForm.frequency = r.frequency;
    recurForm.nextDueDate = strToDate(r.nextDueDate);
    showRecurDialog.value = true;
}

async function saveRecur() {
    recurLoading.value = true;
    try {
        const payload: RecurringTxPayload = {
            name: recurForm.name, type: recurForm.type, amount: recurForm.amount,
            category: recurForm.category, assetClass: recurForm.assetClass || null,
            description: recurForm.description || null, notes: recurForm.notes || null,
            frequency: recurForm.frequency,
            nextDueDate: dateToStr(recurForm.nextDueDate) ?? new Date().toISOString().split("T")[0],
        };
        if (editRecurId.value) { await store.updateRecurring(editRecurId.value, payload); }
        else { await store.addRecurring(payload); }
        toast.add({ severity: "success", summary: "Saved", life: 2500 });
        showRecurDialog.value = false;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Error", detail: String(e?.message ?? e), life: 4000 });
    } finally { recurLoading.value = false; }
}

function deleteRecur(id: number, name: string) {
    confirm.require({
        message: `Delete recurring "${name}"?`,
        header: "Delete",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", severity: "secondary", outlined: true },
        acceptProps: { label: "Delete", severity: "danger" },
        accept: async () => {
            await store.deleteRecurring(id);
            toast.add({ severity: "success", summary: "Deleted", life: 2000 });
        },
    });
}

async function toggleRecur(id: number) {
    await store.toggleRecurring(id);
}

function openApplyDialog() {
    selectedDueIds.value = store.dueRecurring.map(r => r.id);
    showApplyDialog.value = true;
}

async function applySelected() {
    if (!selectedDueIds.value.length) return;
    applyLoading.value = true;
    try {
        await store.applyDue(selectedDueIds.value);
        toast.add({ severity: "success", summary: "Applied", detail: `${selectedDueIds.value.length} transaction(s) recorded.`, life: 3000 });
        showApplyDialog.value = false;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Error", detail: String(e?.message ?? e), life: 4000 });
    } finally { applyLoading.value = false; }
}

function isOverdue(dateStr: string) {
    return dateStr < new Date().toISOString().split("T")[0];
}

// ─── Milestones ──────────────────────────────────────────────────────────────

const showMilestoneDialog = ref(false);
const milestoneForm = reactive({ amount: 0, label: "" });
const milestoneLoading = ref(false);

const achievedMilestones = computed(() => store.milestones.filter(m => m.achievedAt));
const upcomingMilestones = computed(() => store.milestones.filter(m => !m.achievedAt));

function openAddMilestone() {
    milestoneForm.amount = 0;
    milestoneForm.label = "";
    showMilestoneDialog.value = true;
}

async function saveMilestone() {
    milestoneLoading.value = true;
    try {
        await store.addMilestone(milestoneForm.amount, milestoneForm.label);
        toast.add({ severity: "success", summary: "Milestone added", life: 2500 });
        showMilestoneDialog.value = false;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Error", detail: String(e?.message ?? e), life: 4000 });
    } finally { milestoneLoading.value = false; }
}

function deleteMilestone(id: number, label: string) {
    confirm.require({
        message: `Delete custom milestone "${label}"?`,
        header: "Delete Milestone",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", severity: "secondary", outlined: true },
        acceptProps: { label: "Delete", severity: "danger" },
        accept: async () => {
            await store.deleteMilestone(id);
            toast.add({ severity: "success", summary: "Deleted", life: 2000 });
        },
    });
}

onMounted(async () => {
    await Promise.all([
        store.loadUpcoming(30),
        store.fetchBills(),
        store.fetchRecurring(),
        store.loadDue(),
        store.fetchMilestones(),
    ]);
});
</script>

<template>
    <div class="reminders-view">
        <div class="page-header">
            <h1 class="page-title">Reminders</h1>
        </div>

        <Tabs v-model:value="activeTab">
            <TabList>
                <Tab :value="0">Upcoming Bills</Tab>
                <Tab :value="1">Recurring Transactions</Tab>
                <Tab :value="2">Milestones</Tab>
            </TabList>

            <TabPanels>
                <!-- ── Tab 1: Upcoming Bills ─────────────────────────────── -->
                <TabPanel :value="0">
                    <div class="tab-toolbar">
                        <div class="filter-btns">
                            <Button label="7 days" :severity="filterDays === 7 ? 'primary' : 'secondary'" size="small" @click="changeFilter(7)" />
                            <Button label="30 days" :severity="filterDays === 30 ? 'primary' : 'secondary'" size="small" @click="changeFilter(30)" />
                            <Button label="90 days" :severity="filterDays === 90 ? 'primary' : 'secondary'" size="small" @click="changeFilter(90)" />
                        </div>
                        <Button label="Add Custom Bill" icon="pi pi-plus" size="small" @click="openAddBill" />
                    </div>

                    <div v-if="store.upcomingReminders.length === 0" class="empty-state">
                        <i class="pi pi-check-circle empty-icon" />
                        <p>No upcoming bills in the next {{ filterDays }} days.</p>
                        <Button label="Add a bill" text icon="pi pi-plus" @click="openAddBill" />
                    </div>

                    <div v-else class="reminder-list">
                        <div
                            v-for="r in store.upcomingReminders"
                            :key="`${r.source}-${r.sourceId}`"
                            class="reminder-card"
                            :class="urgencyClass(r.daysUntilDue)"
                        >
                            <div class="reminder-left">
                                <i :class="[sourceIcon(r.source), 'reminder-icon']" />
                                <div class="reminder-info">
                                    <div class="reminder-name">{{ r.name }}</div>
                                    <div class="reminder-meta">
                                        <Tag :value="r.category" severity="secondary" />
                                        <span class="reminder-date">{{ r.dueDate }}</span>
                                    </div>
                                </div>
                            </div>
                            <div class="reminder-right">
                                <span class="reminder-amount">{{ formatINR(r.amount) }}</span>
                                <Tag :value="dueDateLabel(r.daysUntilDue)" :severity="urgencySeverity(r.daysUntilDue)" />
                                <Button label="Mark Paid" icon="pi pi-check" size="small" outlined @click="openMarkPaid(r)" />
                                <Button
                                    v-if="r.source === 'bill'"
                                    icon="pi pi-pencil"
                                    text
                                    size="small"
                                    @click="openEditBill(store.bills.find(b => b.id === r.sourceId))"
                                />
                                <Button
                                    v-if="r.source === 'bill'"
                                    icon="pi pi-trash"
                                    text
                                    severity="danger"
                                    size="small"
                                    @click="deleteBill(r.sourceId, r.name)"
                                />
                            </div>
                        </div>
                    </div>
                </TabPanel>

                <!-- ── Tab 2: Recurring Transactions ────────────────────────── -->
                <TabPanel :value="1">
                    <div class="tab-toolbar">
                        <div class="due-notice" v-if="store.dueRecurring.length > 0">
                            <Message severity="warn" :closable="false">
                                {{ store.dueRecurring.length }} recurring transaction{{ store.dueRecurring.length > 1 ? 's' : '' }} due — apply them to your ledger.
                            </Message>
                        </div>
                        <div class="toolbar-actions">
                            <Button
                                v-if="store.dueRecurring.length > 0"
                                :label="`Apply Due (${store.dueRecurring.length})`"
                                icon="pi pi-play"
                                severity="warn"
                                size="small"
                                @click="openApplyDialog"
                            />
                            <Button label="Add Recurring" icon="pi pi-plus" size="small" @click="openAddRecur" />
                        </div>
                    </div>

                    <div v-if="store.recurringList.length === 0" class="empty-state">
                        <i class="pi pi-refresh empty-icon" />
                        <p>No recurring transactions yet.</p>
                        <Button label="Add one" text icon="pi pi-plus" @click="openAddRecur" />
                    </div>

                    <DataTable v-else :value="store.recurringList" :row-class="(r: any) => isOverdue(r.nextDueDate) && r.isActive ? 'row-overdue' : ''" size="small">
                        <Column field="name" header="Name" />
                        <Column header="Type">
                            <template #body="{ data }">
                                <Tag :value="data.type" severity="secondary" />
                            </template>
                        </Column>
                        <Column header="Amount">
                            <template #body="{ data }">{{ formatINR(data.amount) }}</template>
                        </Column>
                        <Column field="frequency" header="Frequency" />
                        <Column header="Next Due">
                            <template #body="{ data }">
                                <span :class="{ 'text-danger': isOverdue(data.nextDueDate) && data.isActive }">
                                    {{ data.nextDueDate }}
                                </span>
                            </template>
                        </Column>
                        <Column header="Status">
                            <template #body="{ data }">
                                <Tag :value="data.isActive ? 'Active' : 'Paused'" :severity="data.isActive ? 'success' : 'secondary'" />
                            </template>
                        </Column>
                        <Column header="Actions" style="width:140px">
                            <template #body="{ data }">
                                <div class="action-btns">
                                    <Button icon="pi pi-pencil" text size="small" @click="openEditRecur(data)" />
                                    <Button
                                        :icon="data.isActive ? 'pi pi-pause' : 'pi pi-play'"
                                        text
                                        size="small"
                                        :severity="data.isActive ? 'secondary' : 'success'"
                                        v-tooltip="data.isActive ? 'Pause' : 'Resume'"
                                        @click="toggleRecur(data.id)"
                                    />
                                    <Button icon="pi pi-trash" text size="small" severity="danger" @click="deleteRecur(data.id, data.name)" />
                                </div>
                            </template>
                        </Column>
                    </DataTable>
                </TabPanel>

                <!-- ── Tab 3: Milestones ────────────────────────────────────── -->
                <TabPanel :value="2">
                    <div class="tab-toolbar">
                        <span class="milestone-hint">Notifications fire automatically when your net worth crosses a milestone on the Dashboard.</span>
                        <Button label="Add Custom" icon="pi pi-plus" size="small" @click="openAddMilestone" />
                    </div>

                    <!-- Achieved -->
                    <div v-if="achievedMilestones.length > 0" class="milestone-section">
                        <h3 class="milestone-heading achieved-heading">
                            <i class="pi pi-check-circle" /> Achieved
                        </h3>
                        <div class="milestone-grid">
                            <div v-for="m in achievedMilestones" :key="m.id" class="milestone-card milestone-achieved">
                                <div class="milestone-icon">🏆</div>
                                <div class="milestone-info">
                                    <div class="milestone-label">{{ m.label }}</div>
                                    <div class="milestone-date">Crossed on {{ m.achievedAt }}</div>
                                </div>
                                <Button v-if="m.isCustom" icon="pi pi-trash" text size="small" severity="danger" @click="deleteMilestone(m.id, m.label)" />
                            </div>
                        </div>
                    </div>

                    <!-- Upcoming -->
                    <div v-if="upcomingMilestones.length > 0" class="milestone-section">
                        <h3 class="milestone-heading">
                            <i class="pi pi-flag" /> Upcoming
                        </h3>
                        <div class="milestone-grid">
                            <div v-for="m in upcomingMilestones" :key="m.id" class="milestone-card milestone-upcoming">
                                <div class="milestone-icon">🎯</div>
                                <div class="milestone-info">
                                    <div class="milestone-label">{{ m.label }}</div>
                                    <div class="milestone-amount">{{ formatINR(m.amount) }}</div>
                                </div>
                                <Button v-if="m.isCustom" icon="pi pi-trash" text size="small" severity="danger" @click="deleteMilestone(m.id, m.label)" />
                            </div>
                        </div>
                    </div>

                    <div v-if="store.milestones.length === 0" class="empty-state">
                        <i class="pi pi-flag empty-icon" />
                        <p>No milestones found.</p>
                    </div>
                </TabPanel>
            </TabPanels>
        </Tabs>
    </div>

    <!-- ── Mark Paid Dialog ─────────────────────────────────────────────────── -->
    <Dialog v-model:visible="showPayDialog" modal :header="`Mark Paid — ${payTarget?.name}`" style="width:380px">
        <div class="form-grid">
            <div class="field">
                <label>Amount (₹)</label>
                <InputNumber v-model="payForm.amount" :min="0" :max-fraction-digits="2" fluid />
            </div>
            <div class="field">
                <label>Payment Date</label>
                <DatePicker v-model="payForm.date" fluid date-format="dd/mm/yy" />
            </div>
            <div class="field">
                <label>Notes (optional)</label>
                <InputText v-model="payForm.notes" fluid placeholder="e.g. paid via UPI" />
            </div>
        </div>
        <template #footer>
            <Button label="Cancel" severity="secondary" outlined @click="showPayDialog = false" />
            <Button label="Record Payment" icon="pi pi-check" :loading="payLoading" @click="submitMarkPaid" />
        </template>
    </Dialog>

    <!-- ── Add/Edit Bill Dialog ─────────────────────────────────────────────── -->
    <Dialog v-model:visible="showBillDialog" modal :header="editBillId ? 'Edit Bill' : 'Add Bill'" style="width:420px">
        <div class="form-grid">
            <div class="field">
                <label>Name</label>
                <InputText v-model="billForm.name" fluid placeholder="e.g. Netflix, Rent" />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Category</label>
                    <Select v-model="billForm.category" :options="BILL_CATEGORIES" fluid />
                </div>
                <div class="field">
                    <label>Frequency</label>
                    <Select v-model="billForm.frequency" :options="FREQUENCIES_BILL" fluid />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Amount (₹)</label>
                    <InputNumber v-model="billForm.amount" :min="0" :max-fraction-digits="2" fluid />
                </div>
                <div class="field">
                    <label>Next Due Date</label>
                    <DatePicker v-model="billForm.nextDueDate" fluid date-format="dd/mm/yy" />
                </div>
            </div>
            <div class="field">
                <label>Notes (optional)</label>
                <Textarea v-model="billForm.notes" rows="2" fluid auto-resize />
            </div>
        </div>
        <template #footer>
            <Button label="Cancel" severity="secondary" outlined @click="showBillDialog = false" />
            <Button :label="editBillId ? 'Update' : 'Add Bill'" icon="pi pi-check" :loading="billLoading" @click="saveBill" />
        </template>
    </Dialog>

    <!-- ── Add/Edit Recurring Dialog ───────────────────────────────────────── -->
    <Dialog v-model:visible="showRecurDialog" modal :header="editRecurId ? 'Edit Recurring' : 'Add Recurring Transaction'" style="width:480px">
        <div class="form-grid">
            <div class="field">
                <label>Name</label>
                <InputText v-model="recurForm.name" fluid placeholder="e.g. Monthly Salary" />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Type</label>
                    <Select v-model="recurForm.type" :options="TX_TYPES" fluid />
                </div>
                <div class="field">
                    <label>Category</label>
                    <Select v-model="recurForm.category" :options="CATEGORIES" fluid />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Amount (₹)</label>
                    <InputNumber v-model="recurForm.amount" :min="0" :max-fraction-digits="2" fluid />
                </div>
                <div class="field">
                    <label>Frequency</label>
                    <Select v-model="recurForm.frequency" :options="FREQUENCIES_RECURRING" fluid />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Next Due Date</label>
                    <DatePicker v-model="recurForm.nextDueDate" fluid date-format="dd/mm/yy" />
                </div>
                <div class="field">
                    <label>Asset Class (optional)</label>
                    <Select v-model="recurForm.assetClass" :options="['', ...ASSET_CLASSES]" fluid />
                </div>
            </div>
            <div class="field">
                <label>Description (optional)</label>
                <InputText v-model="recurForm.description" fluid />
            </div>
            <div class="field">
                <label>Notes (optional)</label>
                <Textarea v-model="recurForm.notes" rows="2" fluid auto-resize />
            </div>
        </div>
        <template #footer>
            <Button label="Cancel" severity="secondary" outlined @click="showRecurDialog = false" />
            <Button :label="editRecurId ? 'Update' : 'Add'" icon="pi pi-check" :loading="recurLoading" @click="saveRecur" />
        </template>
    </Dialog>

    <!-- ── Apply Due Dialog ─────────────────────────────────────────────────── -->
    <Dialog v-model:visible="showApplyDialog" modal header="Apply Due Transactions" style="width:480px">
        <p class="apply-hint">Select transactions to record into your ledger for today:</p>
        <div v-for="r in store.dueRecurring" :key="r.id" class="apply-row">
            <Checkbox v-model="selectedDueIds" :value="r.id" :inputId="`due-${r.id}`" />
            <label :for="`due-${r.id}`" class="apply-label">
                <span class="apply-name">{{ r.name }}</span>
                <span class="apply-meta">{{ formatINR(r.amount) }} · {{ r.frequency }} · due {{ r.nextDueDate }}</span>
            </label>
        </div>
        <template #footer>
            <Button label="Cancel" severity="secondary" outlined @click="showApplyDialog = false" />
            <Button
                :label="`Apply ${selectedDueIds.length} selected`"
                icon="pi pi-check"
                :loading="applyLoading"
                :disabled="!selectedDueIds.length"
                @click="applySelected"
            />
        </template>
    </Dialog>

    <!-- ── Add Custom Milestone Dialog ─────────────────────────────────────── -->
    <Dialog v-model:visible="showMilestoneDialog" modal header="Add Custom Milestone" style="width:360px">
        <div class="form-grid">
            <div class="field">
                <label>Label</label>
                <InputText v-model="milestoneForm.label" fluid placeholder="e.g. Dream House Fund" />
            </div>
            <div class="field">
                <label>Amount (₹)</label>
                <InputNumber v-model="milestoneForm.amount" :min="1" :max-fraction-digits="0" fluid />
            </div>
        </div>
        <template #footer>
            <Button label="Cancel" severity="secondary" outlined @click="showMilestoneDialog = false" />
            <Button label="Add" icon="pi pi-check" :loading="milestoneLoading" @click="saveMilestone" />
        </template>
    </Dialog>
</template>

<style scoped>
.reminders-view { max-width: 900px; }

.page-header { margin-bottom: 1.25rem; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }

.tab-toolbar {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 1rem 0 0.75rem;
    flex-wrap: wrap;
}

.filter-btns { display: flex; gap: 0.4rem; }
.toolbar-actions { display: flex; gap: 0.5rem; align-items: center; }
.due-notice { flex: 1; }

.reminder-list { display: flex; flex-direction: column; gap: 0.6rem; }

.reminder-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.9rem 1rem;
    border-radius: 8px;
    border: 1px solid var(--p-content-border-color);
    background: var(--p-surface-card);
    gap: 1rem;
    flex-wrap: wrap;
}

.reminder-card.overdue { border-color: var(--p-red-400); background: color-mix(in srgb, var(--p-red-400) 6%, var(--p-surface-card)); }
.reminder-card.soon    { border-color: var(--p-orange-400); background: color-mix(in srgb, var(--p-orange-400) 5%, var(--p-surface-card)); }

.reminder-left  { display: flex; align-items: center; gap: 0.75rem; }
.reminder-icon  { font-size: 1.25rem; color: var(--p-primary-color); }
.reminder-name  { font-weight: 600; font-size: 0.9rem; }
.reminder-meta  { display: flex; align-items: center; gap: 0.5rem; margin-top: 0.2rem; }
.reminder-date  { font-size: 0.8rem; color: var(--p-text-muted-color); }

.reminder-right { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
.reminder-amount { font-weight: 700; font-size: 1rem; }

.action-btns { display: flex; gap: 0.2rem; }

.empty-state {
    text-align: center;
    padding: 3rem 1rem;
    color: var(--p-text-muted-color);
}
.empty-icon { font-size: 2.5rem; display: block; margin-bottom: 0.75rem; }

.form-grid { display: flex; flex-direction: column; gap: 0.85rem; padding: 0.25rem 0; }
.field { display: flex; flex-direction: column; gap: 0.3rem; }
.field label { font-size: 0.85rem; font-weight: 500; color: var(--p-text-muted-color); }
.field-row { display: flex; gap: 0.75rem; }
.field-row .field { flex: 1; }

.apply-hint { margin: 0 0 1rem; color: var(--p-text-muted-color); font-size: 0.875rem; }
.apply-row { display: flex; align-items: flex-start; gap: 0.6rem; padding: 0.5rem 0; border-bottom: 1px solid var(--p-content-border-color); }
.apply-row:last-child { border-bottom: none; }
.apply-label { display: flex; flex-direction: column; gap: 0.15rem; cursor: pointer; }
.apply-name { font-weight: 600; font-size: 0.875rem; }
.apply-meta { font-size: 0.8rem; color: var(--p-text-muted-color); }

.text-danger { color: var(--p-red-500); font-weight: 600; }

:deep(.row-overdue td) { background: color-mix(in srgb, var(--p-orange-400) 8%, transparent) !important; }

.milestone-hint { font-size: 0.8rem; color: var(--p-text-muted-color); align-self: center; }

.milestone-section { margin-top: 1.25rem; }
.milestone-heading {
    font-size: 0.85rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--p-text-muted-color);
    margin: 0 0 0.6rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
}
.achieved-heading { color: var(--p-green-600); }

.milestone-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 0.65rem;
}

.milestone-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    border: 1px solid var(--p-content-border-color);
    background: var(--p-surface-card);
}

.milestone-achieved {
    border-color: var(--p-green-400);
    background: color-mix(in srgb, var(--p-green-400) 6%, var(--p-surface-card));
}

.milestone-icon { font-size: 1.4rem; flex-shrink: 0; }
.milestone-info { flex: 1; min-width: 0; }
.milestone-label { font-weight: 600; font-size: 0.875rem; }
.milestone-date { font-size: 0.775rem; color: var(--p-text-muted-color); margin-top: 0.1rem; }
.milestone-amount { font-size: 0.8rem; color: var(--p-text-muted-color); margin-top: 0.1rem; }
</style>
