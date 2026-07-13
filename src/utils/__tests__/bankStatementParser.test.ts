import { describe, expect, it } from "vitest";
import * as XLSX from "xlsx";
import { parseBankStatement } from "@/utils/bankStatementParser";

function buf(text: string): ArrayBuffer {
    return new TextEncoder().encode(text).buffer as ArrayBuffer;
}

describe("parseBankStatement", () => {
    it("parses a tab-delimited HDFC statement, skipping the preamble", () => {
        const text = [
            "HDFC BANK Ltd.\tPage No .:   1\tStatement of accounts",
            "\t\t",
            "Name\t\tAddress :Line1",
            "*****************************************",
            "Date\tNarration\tChq./Ref.No.\tValue Dt\tWithdrawal Amt.\tDeposit Amt.\tClosing Balance",
            "01/01/26\tUPI-SWIGGY-swiggy@ybl\t0000123\t01/01/26\t500.00\t\t10000.00",
            "02/01/26\tNEFT CR-MONTHLY SALARY\tN12345\t02/01/26\t\t90,000.00\t100000.00",
            "\t\tOpening Balance\t\t\t\t9500.00",
        ].join("\n");

        const { rows, headerRowIndex } = parseBankStatement("hdfc", "acct.txt", buf(text));
        expect(headerRowIndex).toBe(4);
        expect(rows).toHaveLength(2);

        expect(rows[0]).toMatchObject({
            date: "2026-01-01",
            type: "expense",
            amount: -500,
            refNo: "0000123",
            category: "Food",
            tag: "HDFC",
            include: true,
        });
        expect(rows[1]).toMatchObject({
            date: "2026-01-02",
            type: "income",
            amount: 90000,
            category: "Salary",
        });
    });

    it("parses a comma ICICI statement and drops zero-money / dateless rows", () => {
        const text = [
            "Detailed Statement",
            "S No.,Value Date,Transaction Date,Cheque Number,Transaction Remarks,Withdrawal Amount (INR),Deposit Amount (INR),Balance (INR)",
            "1,01/01/2026,01/01/2026,,UPI/AMAZON PAY,1200.50,,5000.00",
            "2,02/01/2026,02/01/2026,,IMPS CREDIT REFUND,,300.00,5300.00",
            "3,,,,,,,",
            ",,,Total,,1200.50,300.00,",
        ].join("\n");

        const { rows } = parseBankStatement("icici", "acct.csv", buf(text));
        expect(rows).toHaveLength(2);
        expect(rows[0]).toMatchObject({ date: "2026-01-01", type: "expense", amount: -1200.5, tag: "ICICI" });
        expect(rows[1]).toMatchObject({ date: "2026-01-02", type: "income", amount: 300 });
    });

    it("parses the ICICI DATE|MODE|PARTICULARS|DEPOSITS|WITHDRAWALS|BALANCE layout", () => {
        const text = [
            "DATE,MODE,PARTICULARS,DEPOSITS,WITHDRAWALS,BALANCE",
            "01/01/2026,UPI,UPI/AMAZON PAY,,1200.50,5000.00",
            "02/01/2026,NEFT,IMPS CREDIT REFUND,300.00,,5300.00",
        ].join("\n");

        const { rows } = parseBankStatement("icici", "acct.csv", buf(text));
        expect(rows).toHaveLength(2);
        expect(rows[0]).toMatchObject({ date: "2026-01-01", type: "expense", amount: -1200.5, tag: "ICICI" });
        expect(rows[1]).toMatchObject({ date: "2026-01-02", type: "income", amount: 300 });
    });

    it("parses an XLSX statement via SheetJS", () => {
        const aoa = [
            ["HDFC BANK Ltd.", "", "", "", "", "", ""],
            ["Date", "Narration", "Chq./Ref.No.", "Value Dt", "Withdrawal Amt.", "Deposit Amt.", "Closing Balance"],
            ["05/03/26", "POS HPCL FUEL", "REF9", "05/03/26", "2000", "", "3000"],
        ];
        const ws = XLSX.utils.aoa_to_sheet(aoa);
        const wb = XLSX.utils.book_new();
        XLSX.utils.book_append_sheet(wb, ws, "Sheet1");
        const out = XLSX.write(wb, { type: "array", bookType: "xlsx" }) as ArrayBuffer;

        const { rows } = parseBankStatement("hdfc", "statement.xlsx", out);
        expect(rows).toHaveLength(1);
        expect(rows[0]).toMatchObject({ date: "2026-03-05", type: "expense", amount: -2000, category: "Travel" });
    });

    it("returns rawGrid and no rows when the header can't be found", () => {
        const { rows, headerRowIndex, rawGrid } = parseBankStatement("hdfc", "junk.csv", buf("foo,bar\n1,2\n"));
        expect(headerRowIndex).toBe(-1);
        expect(rows).toHaveLength(0);
        expect(rawGrid.length).toBeGreaterThan(0);
    });
});
