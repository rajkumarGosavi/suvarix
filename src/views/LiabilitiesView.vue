<script setup lang="ts">
import { onMounted, reactive, ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useConfirm } from "primevue/useconfirm";
import { useLiabilitiesStore } from "@/stores/liabilities";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";

const store = useLiabilitiesStore();
const confirm = useConfirm();
const { formatINR } = useCurrencyFormat();

const LOAN_TYPES = ["home", "car", "personal", "education", "gold"];

// ─── Loan CRUD ────────────────────────────────────────────────
interface LoanForm {
    loanType: string;
    lenderName: string;
    accountNumber: string;
    principal: number;
    outstanding: number;
    interestRate: number;
    emiAmount: number;
    tenureMonths: number;
    disbursementDate: Date | null;
    nextEmiDate: Date | null;
}

const showLoanDialog = ref(false);
const editLoan = ref<any>(null);
const loanLoading = ref(false);

const loanForm = reactive<LoanForm>({
    loanType: "home", lenderName: "", accountNumber: "",
    principal: 0, outstanding: 0, interestRate: 0,
    emiAmount: 0, tenureMonths: 12,
    disbursementDate: null, nextEmiDate: null,
});

function resetLoanForm() {
    Object.assign(loanForm, {
        loanType: "home", lenderName: "", accountNumber: "",
        principal: 0, outstanding: 0, interestRate: 0,
        emiAmount: 0, tenureMonths: 12,
        disbursementDate: new Date(), nextEmiDate: null,
    });
}

function openAddLoan() {
    editLoan.value = null;
    resetLoanForm();
    showLoanDialog.value = true;
}

function openEditLoan(item: any) {
    editLoan.value = item;
    Object.assign(loanForm, {
        loanType: item.loanType,
        lenderName: item.lenderName,
        accountNumber: item.accountNumber ?? "",
        principal: item.principal,
        outstanding: item.outstanding,
        interestRate: item.interestRate,
        emiAmount: item.emiAmount,
        tenureMonths: item.tenureMonths,
        disbursementDate: strToDate(item.disbursementDate),
        nextEmiDate: strToDate(item.nextEmiDate),
    });
    showLoanDialog.value = true;
}

async function submitLoan() {
    loanLoading.value = true;
    try {
        const payload = {
            loanType: loanForm.loanType,
            lenderName: loanForm.lenderName,
            accountNumber: loanForm.accountNumber || null,
            principal: loanForm.principal,
            outstanding: loanForm.outstanding,
            interestRate: loanForm.interestRate,
            emiAmount: loanForm.emiAmount,
            tenureMonths: loanForm.tenureMonths,
            disbursementDate: dateToStr(loanForm.disbursementDate) ?? "",
            nextEmiDate: dateToStr(loanForm.nextEmiDate),
        };
        if (editLoan.value) {
            await store.updateLoan(editLoan.value.id, payload);
        } else {
            await store.addLoan(payload);
        }
        showLoanDialog.value = false;
    } finally {
        loanLoading.value = false;
    }
}

function confirmDeleteLoan(item: any) {
    confirm.require({
        message: `Delete loan from ${item.lenderName}?`,
        header: "Delete Loan",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => store.removeLoan(item.id),
    });
}

// ─── Credit Card CRUD ─────────────────────────────────────────
interface CardForm {
    bankName: string;
    cardName: string;
    lastFour: string;
    creditLimit: number;
    currentBalance: number;
    dueDate: number | null;
    minPayment: number | null;
}

const showCardDialog = ref(false);
const editCard = ref<any>(null);
const cardLoading = ref(false);

const cardForm = reactive<CardForm>({
    bankName: "", cardName: "", lastFour: "",
    creditLimit: 0, currentBalance: 0,
    dueDate: null, minPayment: null,
});

function resetCardForm() {
    Object.assign(cardForm, {
        bankName: "", cardName: "", lastFour: "",
        creditLimit: 0, currentBalance: 0,
        dueDate: null, minPayment: null,
    });
}

function openAddCard() {
    editCard.value = null;
    resetCardForm();
    showCardDialog.value = true;
}

function openEditCard(item: any) {
    editCard.value = item;
    Object.assign(cardForm, {
        bankName: item.bankName,
        cardName: item.cardName ?? "",
        lastFour: item.lastFour ?? "",
        creditLimit: item.creditLimit,
        currentBalance: item.currentBalance,
        dueDate: item.dueDate ?? null,
        minPayment: item.minPayment ?? null,
    });
    showCardDialog.value = true;
}

async function submitCard() {
    cardLoading.value = true;
    try {
        const payload = {
            bankName: cardForm.bankName,
            cardName: cardForm.cardName || null,
            lastFour: cardForm.lastFour || null,
            creditLimit: cardForm.creditLimit,
            currentBalance: cardForm.currentBalance,
            dueDate: cardForm.dueDate,
            minPayment: cardForm.minPayment,
        };
        if (editCard.value) {
            await store.updateCard(editCard.value.id, payload);
        } else {
            await store.addCard(payload);
        }
        showCardDialog.value = false;
    } finally {
        cardLoading.value = false;
    }
}

function confirmDeleteCard(item: any) {
    confirm.require({
        message: `Delete ${item.bankName}${item.cardName ? " " + item.cardName : ""} card?`,
        header: "Delete Credit Card",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => store.removeCard(item.id),
    });
}

// ─── Amortization Schedule ────────────────────────────────────
interface EmiRow { month: number; payment: number; principal: number; interest: number; balance: number; }

const showAmortDialog = ref(false);
const amortLoan = ref<any>(null);
const amortSchedule = ref<EmiRow[]>([]);
const amortLoading = ref(false);

const amortTotals = computed(() => {
    const rows = amortSchedule.value;
    return {
        totalPayment:   rows.reduce((s, r) => s + r.payment, 0),
        totalPrincipal: rows.reduce((s, r) => s + r.principal, 0),
        totalInterest:  rows.reduce((s, r) => s + r.interest, 0),
        months:         rows.length,
    };
});

async function openAmortization(loan: any) {
    amortLoan.value = loan;
    amortSchedule.value = [];
    showAmortDialog.value = true;
    amortLoading.value = true;
    try {
        amortSchedule.value = await invoke<EmiRow[]>("get_amortization_schedule", { loanId: loan.id });
    } finally {
        amortLoading.value = false;
    }
}

onMounted(() => store.fetchAll());
</script>

<template>
    <div class="liabilities-view">
        <h1 class="page-title">Liabilities</h1>

        <div v-if="store.isLoading" class="loading"><ProgressSpinner /></div>

        <template v-else>
            <!-- Loans -->
            <div class="section-header">
                <h2>Loans</h2>
                <Button icon="pi pi-plus" label="Add Loan" size="small" @click="openAddLoan" />
            </div>
            <DataTable :value="store.loans" stripedRows emptyMessage="No loans added.">
                <Column field="loanType" header="Type" style="width:100px" />
                <Column field="lenderName" header="Lender" />
                <Column field="principal" header="Principal">
                    <template #body="{ data }">{{ formatINR(data.principal) }}</template>
                </Column>
                <Column field="outstanding" header="Outstanding">
                    <template #body="{ data }">{{ formatINR(data.outstanding) }}</template>
                </Column>
                <Column field="emiAmount" header="EMI / mo">
                    <template #body="{ data }">{{ formatINR(data.emiAmount) }}</template>
                </Column>
                <Column field="interestRate" header="Rate">
                    <template #body="{ data }">{{ data.interestRate }}%</template>
                </Column>
                <Column field="nextEmiDate" header="Next EMI" />
                <Column header="" style="width:120px">
                    <template #body="{ data }">
                        <Button icon="pi pi-calendar" text size="small" @click="openAmortization(data)" v-tooltip="'View amortization schedule'" />
                        <Button icon="pi pi-pencil" text size="small" @click="openEditLoan(data)" />
                        <Button icon="pi pi-trash" text size="small" @click="confirmDeleteLoan(data)" />
                    </template>
                </Column>
            </DataTable>

            <!-- Credit Cards -->
            <div class="section-header">
                <h2>Credit Cards</h2>
                <Button icon="pi pi-plus" label="Add Card" size="small" @click="openAddCard" />
            </div>
            <DataTable :value="store.creditCards" stripedRows emptyMessage="No credit cards added.">
                <Column field="bankName" header="Bank" />
                <Column field="cardName" header="Card" />
                <Column field="lastFour" header="Number">
                    <template #body="{ data }">{{ data.lastFour ? `•••• ${data.lastFour}` : '—' }}</template>
                </Column>
                <Column field="currentBalance" header="Balance">
                    <template #body="{ data }">{{ formatINR(data.currentBalance) }}</template>
                </Column>
                <Column field="creditLimit" header="Limit">
                    <template #body="{ data }">{{ formatINR(data.creditLimit) }}</template>
                </Column>
                <Column field="dueDate" header="Due Day">
                    <template #body="{ data }">{{ data.dueDate ?? '—' }}</template>
                </Column>
                <Column header="" style="width:90px">
                    <template #body="{ data }">
                        <Button icon="pi pi-pencil" text size="small" @click="openEditCard(data)" />
                        <Button icon="pi pi-trash" text size="small" @click="confirmDeleteCard(data)" />
                    </template>
                </Column>
            </DataTable>
        </template>
    </div>

    <!-- Loan Dialog -->
    <Dialog v-model:visible="showLoanDialog" :header="editLoan ? 'Edit Loan' : 'Add Loan'" modal style="width:520px">
        <form @submit.prevent="submitLoan" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Loan Type *</label>
                    <Select v-model="loanForm.loanType" :options="LOAN_TYPES" class="w-full" required />
                </div>
                <div class="field">
                    <label>Lender Name *</label>
                    <InputText v-model="loanForm.lenderName" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Account Number</label>
                <InputText v-model="loanForm.accountNumber" class="w-full" placeholder="Optional" />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Principal (₹) *</label>
                    <InputNumber v-model="loanForm.principal" :min="0" :minFractionDigits="2" class="w-full" required />
                </div>
                <div class="field">
                    <label>Outstanding (₹) *</label>
                    <InputNumber v-model="loanForm.outstanding" :min="0" :minFractionDigits="2" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Interest Rate (%) *</label>
                    <InputNumber v-model="loanForm.interestRate" :min="0" :max="100" :minFractionDigits="2" class="w-full" required />
                </div>
                <div class="field">
                    <label>EMI Amount (₹) *</label>
                    <InputNumber v-model="loanForm.emiAmount" :min="0" :minFractionDigits="2" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Tenure (months) *</label>
                    <InputNumber v-model="loanForm.tenureMonths" :min="1" class="w-full" required />
                </div>
                <div class="field">
                    <label>Disbursement Date *</label>
                    <DatePicker v-model="loanForm.disbursementDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Next EMI Date</label>
                <DatePicker v-model="loanForm.nextEmiDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" />
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="showLoanDialog = false" />
                <Button type="submit" :label="editLoan ? 'Update' : 'Add Loan'" :loading="loanLoading" />
            </div>
        </form>
    </Dialog>

    <!-- Credit Card Dialog -->
    <Dialog v-model:visible="showCardDialog" :header="editCard ? 'Edit Credit Card' : 'Add Credit Card'" modal style="width:480px">
        <form @submit.prevent="submitCard" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Bank Name *</label>
                    <InputText v-model="cardForm.bankName" class="w-full" required />
                </div>
                <div class="field">
                    <label>Card Name</label>
                    <InputText v-model="cardForm.cardName" class="w-full" placeholder="e.g. HDFC Regalia" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Last 4 Digits</label>
                    <InputText v-model="cardForm.lastFour" class="w-full" maxlength="4" placeholder="1234" />
                </div>
                <div class="field">
                    <label>Due Date (day of month)</label>
                    <InputNumber v-model="cardForm.dueDate" :min="1" :max="31" class="w-full" placeholder="e.g. 15" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Credit Limit (₹) *</label>
                    <InputNumber v-model="cardForm.creditLimit" :min="0" :minFractionDigits="2" class="w-full" required />
                </div>
                <div class="field">
                    <label>Current Balance (₹) *</label>
                    <InputNumber v-model="cardForm.currentBalance" :min="0" :minFractionDigits="2" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Minimum Payment (₹)</label>
                <InputNumber v-model="cardForm.minPayment" :min="0" :minFractionDigits="2" class="w-full" />
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="showCardDialog = false" />
                <Button type="submit" :label="editCard ? 'Update' : 'Add Card'" :loading="cardLoading" />
            </div>
        </form>
    </Dialog>

    <!-- Amortization Schedule Dialog -->
    <Dialog
        v-model:visible="showAmortDialog"
        :header="`Amortization Schedule — ${amortLoan?.lenderName ?? ''}`"
        modal
        style="width:720px; max-width:95vw"
    >
        <div v-if="amortLoading" class="amort-loading"><ProgressSpinner /></div>

        <template v-else-if="amortSchedule.length">
            <!-- Summary row -->
            <div class="amort-summary">
                <div class="amort-stat">
                    <span class="amort-stat-label">Months remaining</span>
                    <span class="amort-stat-value">{{ amortTotals.months }}</span>
                </div>
                <div class="amort-stat">
                    <span class="amort-stat-label">Total payment</span>
                    <span class="amort-stat-value">{{ formatINR(amortTotals.totalPayment) }}</span>
                </div>
                <div class="amort-stat">
                    <span class="amort-stat-label">Total interest</span>
                    <span class="amort-stat-value">{{ formatINR(amortTotals.totalInterest) }}</span>
                </div>
                <div class="amort-stat">
                    <span class="amort-stat-label">Total principal</span>
                    <span class="amort-stat-value">{{ formatINR(amortTotals.totalPrincipal) }}</span>
                </div>
            </div>

            <DataTable
                :value="amortSchedule"
                scrollable
                scrollHeight="380px"
                stripedRows
                size="small"
                class="amort-table"
            >
                <Column field="month" header="Month" style="width:80px" />
                <Column field="payment" header="EMI">
                    <template #body="{ data }">{{ formatINR(data.payment) }}</template>
                </Column>
                <Column field="principal" header="Principal">
                    <template #body="{ data }">{{ formatINR(data.principal) }}</template>
                </Column>
                <Column field="interest" header="Interest">
                    <template #body="{ data }">{{ formatINR(data.interest) }}</template>
                </Column>
                <Column field="balance" header="Balance">
                    <template #body="{ data }">{{ formatINR(data.balance) }}</template>
                </Column>
            </DataTable>
        </template>

        <p v-else class="amort-empty">No schedule data available for this loan.</p>
    </Dialog>

    <ConfirmDialog />
</template>

<style scoped>
.liabilities-view { max-width: 1000px; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0 0 1.5rem; }
.section-header { display: flex; justify-content: space-between; align-items: center; margin: 1.5rem 0 0.75rem; }
.section-header:first-of-type { margin-top: 0; }
.section-header h2 { margin: 0; font-size: 1.1rem; }
.loading { display: flex; justify-content: center; padding: 4rem; }
.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field-row { display: flex; gap: 1rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }

/* Amortization */
.amort-loading { display: flex; justify-content: center; padding: 2.5rem; }
.amort-summary {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1rem;
    margin-bottom: 1.25rem;
}
.amort-stat { display: flex; flex-direction: column; gap: 0.25rem; }
.amort-stat-label { font-size: 0.75rem; font-weight: 500; }
.amort-stat-value { font-size: 1.05rem; font-weight: 700; font-variant-numeric: tabular-nums; }
.amort-table { font-size: 0.85rem; }
.amort-empty { text-align: center; padding: 2rem; }
</style>
