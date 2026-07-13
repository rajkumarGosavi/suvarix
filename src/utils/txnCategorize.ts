// Rule-based categorization of a bank-statement narration into one of the
// seeded default categories (see DEFAULT_CATEGORIES_SEED in migrations.rs):
//   Food, Rent, EMI, Travel, Medical, Utilities, Entertainment, Education,
//   Shopping, Dividend, Interest, Salary, Other.
// Fallback is "Other" (already seeded → no new category needs creating).
// Rules are ordered: income-side signals first, then merchant/keyword matches.
// The user can override any row's category in the import preview before commit.

interface Rule {
    category: string;
    // matched case-insensitively against the narration
    pattern: RegExp;
}

const RULES: Rule[] = [
    // ── income-side ───────────────────────────────────────────────
    { category: "Salary", pattern: /\bSALARY\b|\bSAL\b|SAL[\s.]?CR|NEFT.*SALARY|MONTHLY SAL/i },
    { category: "Interest", pattern: /\bINT(?:EREST)?\b|CREDIT INTEREST|INT[\s.]?PD|SB INT|FD INT/i },
    { category: "Dividend", pattern: /\bDIVIDEND\b|\bDIV\b|IDCW/i },
    // ── merchants / spend ─────────────────────────────────────────
    { category: "Food", pattern: /SWIGGY|ZOMATO|DOMINO|MCDONALD|KFC|RESTAURANT|CAFE|FOODPANDA|EATCLUB|BLINKIT|ZEPTO/i },
    { category: "Shopping", pattern: /AMAZON|FLIPKART|MYNTRA|AJIO|MEESHO|NYKAA|RELIANCE(?:\s*(?:RETAIL|TRENDS|DIGITAL))?|DMART|BIGBASKET|SHOPP?ING/i },
    { category: "Travel", pattern: /\bUBER\b|\bOLA\b|RAPIDO|IRCTC|MAKEMYTRIP|GOIBIBO|REDBUS|INDIGO|SPICEJET|VISTARA|\bFUEL\b|PETROL|DIESEL|HPCL|IOCL|BPCL|IND(?:IAN)?\s*OIL|TRAVEL/i },
    { category: "Utilities", pattern: /ELECTRIC|\bBSES\b|\bMSEB\b|RECHARGE|\bJIO\b|AIRTEL|\bVI\b|VODAFONE|\bBSNL\b|BROADBAND|\bGAS\b|WATER BILL|BILLPAY|\bBBPS\b|\bDTH\b|TATA POWER|ADANI/i },
    { category: "Medical", pattern: /PHARMAC|APOLLO|MEDPLUS|HOSPITAL|CLINIC|\bMEDIC|DIAGNOSTIC|PRACTO|1MG|NETMEDS|LAB\b/i },
    { category: "Entertainment", pattern: /NETFLIX|SPOTIFY|HOTSTAR|PRIME VIDEO|\bZEE5\b|SONYLIV|BOOKMYSHOW|\bPVR\b|INOX|YOUTUBE PREMIUM|GAMING/i },
    { category: "Education", pattern: /\bFEES?\b|SCHOOL|COLLEGE|UNIVERSITY|TUITION|UDEMY|COURSERA|BYJU|UNACADEMY|VEDANTU/i },
    { category: "EMI", pattern: /\bEMI\b|\bLOAN\b|\bACH\b.*(?:EMI|LOAN)|BAJAJ FIN|HDFC LOAN|HOME LOAN|CAR LOAN/i },
    { category: "Rent", pattern: /\bRENT\b|HOUSE RENT|NOBROKER|RENTPAY/i },
];

/**
 * Best-effort category for a bank-statement narration.
 * Returns the first matching rule's category, else "Other".
 */
export function categorize(narration: string): string {
    const text = (narration || "").toUpperCase();
    for (const rule of RULES) {
        if (rule.pattern.test(text)) return rule.category;
    }
    return "Other";
}
