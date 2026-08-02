<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
    buildItrDebugReport,
    ItrPasswordRequired,
    parseItrPdf,
    type FieldConfidence,
    type InvariantIssue,
    type ItrParseResult,
    type ParsedItr,
} from "@/utils/itrParser";
import { useItrStore, type ItrReturn } from "@/stores/itr";

const props = defineProps<{ visible: boolean; editing?: ItrReturn | null }>();
const emit = defineEmits<{ (e: "update:visible", v: boolean): void; (e: "saved"): void }>();

const store = useItrStore();

// "pick" → choose a file (or skip straight to manual entry), "review" → editable form
const stage = ref<"pick" | "review">("pick");
const file = ref<File | null>(null);
const password = ref("");
const needsPassword = ref(false);
const passwordAttempts = ref(0);
const parsing = ref(false);
const parseError = ref("");
const rawLines = ref<string[]>([]);
const confidence = ref<Record<string, FieldConfidence>>({});
const issues = ref<InvariantIssue[]>([]);
const showRaw = ref(false);
const overwriteAsked = ref(false);
/** Path of the parse-debug log written after the last parse (empty if unwritten). */
const debugLogPath = ref("");

function blankReturn(): ItrReturn {
    return {
        id: null,
        assessmentYear: "",
        formType: "ITR-2",
        regime: null,
        panMasked: null,
        filingDate: null,
        ackNumber: null,
        salaryIncome: 0,
        housePropertyIncome: 0,
        capitalGainsStcg: 0,
        capitalGainsLtcg: 0,
        otherSourcesIncome: 0,
        businessIncome: 0,
        grossTotalIncome: 0,
        chapterViaDeductions: 0,
        totalIncome: 0,
        taxOnTotalIncome: 0,
        surcharge: 0,
        cess: 0,
        totalTaxLiability: 0,
        tdsPaid: 0,
        advanceTaxPaid: 0,
        selfAssessmentTaxPaid: 0,
        tcsPaid: 0,
        totalTaxPaid: 0,
        refundDue: 0,
        taxPayable: 0,
        source: "manual",
    };
}

const form = ref<ItrReturn>(blankReturn());

const AMOUNT_FIELDS: Array<{ key: keyof ItrReturn; label: string; group: string }> = [
    { key: "salaryIncome", label: "Salary", group: "Income" },
    { key: "housePropertyIncome", label: "House property", group: "Income" },
    { key: "capitalGainsStcg", label: "Short-term capital gains", group: "Income" },
    { key: "capitalGainsLtcg", label: "Long-term capital gains", group: "Income" },
    { key: "otherSourcesIncome", label: "Other sources", group: "Income" },
    { key: "businessIncome", label: "Business / profession", group: "Income" },
    { key: "grossTotalIncome", label: "Gross total income", group: "Income" },
    { key: "chapterViaDeductions", label: "Chapter VI-A deductions", group: "Deductions" },
    { key: "totalIncome", label: "Total (taxable) income", group: "Deductions" },
    { key: "taxOnTotalIncome", label: "Tax on total income", group: "Tax" },
    { key: "surcharge", label: "Surcharge", group: "Tax" },
    { key: "cess", label: "Cess", group: "Tax" },
    { key: "totalTaxLiability", label: "Total tax liability", group: "Tax" },
    { key: "tdsPaid", label: "TDS", group: "Taxes paid" },
    { key: "advanceTaxPaid", label: "Advance tax", group: "Taxes paid" },
    { key: "selfAssessmentTaxPaid", label: "Self-assessment tax", group: "Taxes paid" },
    { key: "tcsPaid", label: "TCS", group: "Taxes paid" },
    { key: "totalTaxPaid", label: "Total taxes paid", group: "Taxes paid" },
    { key: "refundDue", label: "Refund due", group: "Outcome" },
    { key: "taxPayable", label: "Tax still payable", group: "Outcome" },
];

const GROUPS = ["Income", "Deductions", "Tax", "Taxes paid", "Outcome"];

function fieldsOf(group: string) {
    return AMOUNT_FIELDS.filter((f) => f.group === group);
}

/**
 * Badge shown next to a field. `parsed` gets none — a plain read from the PDF is
 * the unremarkable case, and a tag on every row would drown the ones that matter.
 */
const CONFIDENCE_TAG: Partial<
    Record<FieldConfidence, { value: string; severity: string; title: string }>
> = {
    missing: {
        value: "not found",
        severity: "warn",
        title: "No row in the PDF matched this field.",
    },
    derived: {
        value: "derived",
        severity: "info",
        title: "Not found in the PDF — computed from the surrounding Part B figures.",
    },
    confirmed: {
        value: "checked",
        severity: "success",
        title: "Read from the PDF and consistent with the Part B arithmetic.",
    },
    conflict: {
        value: "check this",
        severity: "danger",
        title: "This figure breaks the Part B arithmetic — verify it against your return.",
    },
};

function tagOf(key: string) {
    return CONFIDENCE_TAG[confidence.value[key] ?? "parsed"];
}

const missingCount = computed(
    () => Object.values(confidence.value).filter((c) => c === "missing").length,
);

const REGIME_OPTIONS = [
    { label: "Old regime", value: "old" },
    { label: "New regime", value: "new" },
];

const isEditing = computed(() => !!props.editing?.id);

const dialogTitle = computed(() =>
    isEditing.value
        ? `Edit AY ${props.editing?.assessmentYear}`
        : stage.value === "pick" ? "Import ITR PDF" : "Review before saving",
);

const canSave = computed(() => /^\d{4}-\d{2}$/.test(form.value.assessmentYear.trim()));

const willOverwrite = computed(
    () => !isEditing.value && store.hasYear(form.value.assessmentYear.trim()),
);

function reset() {
    stage.value = "pick";
    file.value = null;
    password.value = "";
    needsPassword.value = false;
    passwordAttempts.value = 0;
    parseError.value = "";
    rawLines.value = [];
    confidence.value = {};
    issues.value = [];
    showRaw.value = false;
    overwriteAsked.value = false;
    debugLogPath.value = "";
    form.value = blankReturn();
}

// Re-seed whenever the dialog opens: edit mode prefills, import mode starts clean.
watch(
    () => props.visible,
    (open) => {
        if (!open) return;
        reset();
        if (props.editing?.id) {
            form.value = { ...props.editing };
            stage.value = "review";
        }
    },
    { immediate: true },
);

function onFileSelect(event: any) {
    file.value = event.files?.[0] ?? null;
    parseError.value = "";
}

function applyParsed(data: ParsedItr, formType: string | null, assessmentYear: string | null) {
    const next = blankReturn();
    next.source = "pdf";
    next.formType = formType ?? "ITR-2";
    next.assessmentYear = assessmentYear ?? "";
    next.regime = data.regime ?? null;
    next.panMasked = data.panMasked ?? null;
    next.filingDate = data.filingDate ?? null;
    next.ackNumber = data.ackNumber ?? null;
    for (const { key } of AMOUNT_FIELDS) {
        const v = (data as Record<string, unknown>)[key as string];
        if (typeof v === "number") (next as unknown as Record<string, unknown>)[key as string] = v;
    }
    form.value = next;
}

// Writes the Part B raw lines + what each rule made of them to a log file, so the
// parser patterns can be tuned against a real return. Never blocks the import.
async function writeDebugLog(result: ItrParseResult) {
    try {
        debugLogPath.value = await invoke<string>("write_itr_debug_log", {
            content: buildItrDebugReport(result, file.value?.name ?? "(unknown)"),
        });
    } catch {
        debugLogPath.value = "";
    }
}

async function runParse() {
    if (!file.value) return;
    parsing.value = true;
    parseError.value = "";
    try {
        const result = await parseItrPdf(await file.value.arrayBuffer(), password.value.trim());
        await writeDebugLog(result);
        if (!result.formType && !result.assessmentYear) {
            parseError.value =
                "This does not look like an ITR PDF — no form type or assessment year found. " +
                "Use the filed ITR-2 PDF downloaded from the e-filing portal.";
            return;
        }
        if (result.formType && result.formType !== "ITR-2") {
            parseError.value =
                `Detected ${result.formType}. Only ITR-2 layouts are tuned, so check every ` +
                "field below before saving.";
        }
        needsPassword.value = false;
        rawLines.value = result.rawLines;
        confidence.value = result.confidence;
        issues.value = result.issues;
        applyParsed(result.data, result.formType, result.assessmentYear);
        stage.value = "review";
    } catch (e: any) {
        if (e instanceof ItrPasswordRequired) {
            needsPassword.value = true;
            passwordAttempts.value += 1;
            parseError.value =
                passwordAttempts.value >= 3
                    ? "Password rejected three times. Close and try the PDF again, or enter the year manually."
                    : "This PDF is password protected. It is usually your PAN in lowercase followed by your date of birth as DDMMYYYY.";
        } else {
            parseError.value = e?.message ?? "Could not read this PDF.";
        }
    } finally {
        parsing.value = false;
    }
}

function startManual() {
    form.value = blankReturn();
    confidence.value = {};
    issues.value = [];
    rawLines.value = [];
    stage.value = "review";
}

async function onSave() {
    if (!canSave.value) return;
    if (willOverwrite.value && !overwriteAsked.value) {
        overwriteAsked.value = true;
        return;
    }
    try {
        await store.save({ ...form.value, assessmentYear: form.value.assessmentYear.trim() });
        emit("saved");
        emit("update:visible", false);
    } catch {
        // store.error is rendered inline below
    }
}

function close() {
    emit("update:visible", false);
}
</script>

<template>
    <Dialog
        :visible="visible"
        modal
        :header="dialogTitle"
        :style="{ width: '46rem' }"
        :breakpoints="{ '639px': '95vw' }"
        @update:visible="close"
    >
        <!-- Stage 1: pick a PDF -->
        <div v-if="stage === 'pick'" class="pick">
            <p class="hint">
                Select the ITR-2 PDF you downloaded from the income-tax e-filing portal.
                It is parsed on this device only — nothing leaves the app.
            </p>

            <FileUpload
                mode="basic"
                accept="application/pdf"
                :auto="false"
                choose-label="Choose ITR PDF"
                custom-upload
                @select="onFileSelect"
            />

            <div v-if="needsPassword" class="pw">
                <label for="itr-pw">PDF password</label>
                <Password
                    id="itr-pw"
                    v-model="password"
                    :feedback="false"
                    toggle-mask
                    fluid
                />
            </div>

            <Message v-if="parseError" severity="warn" :closable="false">{{ parseError }}</Message>

            <div class="pick-actions">
                <Button
                    label="Parse PDF"
                    icon="pi pi-file-pdf"
                    :disabled="!file || parsing || passwordAttempts >= 3"
                    :loading="parsing"
                    @click="runParse"
                />
                <Button label="Enter manually" severity="secondary" text @click="startManual" />
            </div>
        </div>

        <!-- Stage 2: review / edit -->
        <div v-else class="review">
            <Message v-if="parseError" severity="warn" :closable="false">{{ parseError }}</Message>
            <Message v-if="issues.length" severity="error" :closable="false">
                <p class="msg-lead">
                    These figures do not add up. At least one was read from the wrong row —
                    check them against your return before saving.
                </p>
                <ul class="issues">
                    <li v-for="(iss, i) in issues" :key="i">
                        {{ iss.equation }} — expected
                        <strong>{{ iss.expected.toLocaleString("en-IN") }}</strong>, read
                        <strong>{{ iss.actual.toLocaleString("en-IN") }}</strong>
                    </li>
                </ul>
            </Message>
            <Message v-if="missingCount" severity="info" :closable="false">
                {{ missingCount }} field(s) could not be found in the PDF — they are marked below.
                Fill them in from your return before saving.
            </Message>

            <div class="grid-2">
                <div class="field">
                    <label for="ay">Assessment year</label>
                    <InputText id="ay" v-model="form.assessmentYear" placeholder="2024-25" fluid />
                    <small v-if="!canSave" class="err">Use the format 2024-25.</small>
                </div>
                <div class="field">
                    <label for="ft">Form</label>
                    <InputText id="ft" v-model="form.formType" fluid />
                </div>
                <div class="field">
                    <label for="rg">Regime</label>
                    <Select
                        id="rg"
                        v-model="form.regime"
                        :options="REGIME_OPTIONS"
                        option-label="label"
                        option-value="value"
                        show-clear
                        fluid
                    />
                </div>
                <div class="field">
                    <label for="fd">Date of filing</label>
                    <InputText id="fd" v-model="form.filingDate" placeholder="2024-07-20" fluid />
                </div>
                <div class="field">
                    <label for="pan">PAN (masked)</label>
                    <InputText id="pan" v-model="form.panMasked" fluid />
                </div>
                <div class="field">
                    <label for="ack">Acknowledgement number</label>
                    <InputText id="ack" v-model="form.ackNumber" fluid />
                </div>
            </div>

            <template v-for="group in GROUPS" :key="group">
                <h4 class="group-title">{{ group }}</h4>
                <div class="grid-2">
                    <div v-for="f in fieldsOf(group)" :key="String(f.key)" class="field">
                        <label :for="String(f.key)">
                            {{ f.label }}
                            <Tag
                                v-if="tagOf(String(f.key))"
                                :severity="tagOf(String(f.key))!.severity"
                                :value="tagOf(String(f.key))!.value"
                                :title="tagOf(String(f.key))!.title"
                            />
                        </label>
                        <InputNumber
                            :input-id="String(f.key)"
                            :model-value="(form[f.key] as number)"
                            mode="decimal"
                            :min-fraction-digits="0"
                            :max-fraction-digits="2"
                            fluid
                            @update:model-value="(v: number) => ((form[f.key] as number) = v ?? 0)"
                        />
                    </div>
                </div>
            </template>

            <Message v-if="debugLogPath" severity="secondary" :closable="false">
                Parse debug log written to <code>{{ debugLogPath }}</code> — it contains your
                actual figures in plain text. Delete it once the parser is tuned.
            </Message>

            <div v-if="rawLines.length" class="raw">
                <Button
                    :label="showRaw ? 'Hide extracted text' : 'Show extracted text'"
                    text
                    size="small"
                    @click="showRaw = !showRaw"
                />
                <pre v-if="showRaw" class="raw-lines">{{ rawLines.join("\n") }}</pre>
            </div>

            <Message v-if="overwriteAsked && willOverwrite" severity="warn" :closable="false">
                AY {{ form.assessmentYear }} is already stored. Saving again overwrites it —
                press Save once more to confirm.
            </Message>
            <Message v-if="store.error" severity="error" :closable="false">{{ store.error }}</Message>
        </div>

        <template #footer>
            <Button label="Cancel" severity="secondary" text @click="close" />
            <Button
                v-if="stage === 'review'"
                label="Save"
                icon="pi pi-check"
                :disabled="!canSave || store.isSaving"
                :loading="store.isSaving"
                @click="onSave"
            />
        </template>
    </Dialog>
</template>

<style scoped>
.hint {
    color: var(--p-text-muted-color);
    font-size: 0.85rem;
    margin: 0 0 0.75rem;
}
.pick {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.pick-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
}
.pw {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
}
.review {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
}
.field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
}
.field label {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
    display: flex;
    align-items: center;
    gap: 0.4rem;
}
.group-title {
    margin: 0.5rem 0 0;
    font-size: 0.9rem;
}
.err {
    color: var(--p-red-400);
}
.msg-lead {
    margin: 0 0 0.35rem;
}
.issues {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.8rem;
}
.raw-lines {
    max-height: 14rem;
    overflow: auto;
    font-size: 0.72rem;
    background: color-mix(in srgb, var(--p-content-background) 92%, var(--p-text-color) 8%);
    padding: 0.5rem;
    border-radius: 6px;
    white-space: pre-wrap;
}
@media (max-width: 639px) {
    .grid-2 {
        grid-template-columns: 1fr;
    }
}
</style>
