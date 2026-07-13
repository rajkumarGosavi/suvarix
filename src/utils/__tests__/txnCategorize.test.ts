import { describe, expect, it } from "vitest";
import { categorize } from "@/utils/txnCategorize";

describe("categorize", () => {
    it("maps income-side narrations", () => {
        expect(categorize("NEFT CR-ACME PVT LTD-MONTHLY SALARY")).toBe("Salary");
        expect(categorize("CREDIT INTEREST CAPITALISED")).toBe("Interest");
        expect(categorize("ACH C- DIVIDEND INFOSYS")).toBe("Dividend");
    });

    it("maps common merchants", () => {
        expect(categorize("UPI-SWIGGY-swiggy@ybl")).toBe("Food");
        expect(categorize("UPI-AMAZON PAY INDIA")).toBe("Shopping");
        expect(categorize("POS HPCL FUEL STATION")).toBe("Travel");
        expect(categorize("BILLPAY AIRTEL RECHARGE")).toBe("Utilities");
        expect(categorize("UPI-APOLLO PHARMACY")).toBe("Medical");
        expect(categorize("NETFLIX SUBSCRIPTION")).toBe("Entertainment");
        expect(categorize("ACH D- HOME LOAN EMI")).toBe("EMI");
    });

    it("falls back to Other for unknown narrations", () => {
        expect(categorize("SOME RANDOM MERCHANT XYZ")).toBe("Other");
        expect(categorize("")).toBe("Other");
    });
});
