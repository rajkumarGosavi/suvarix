const inrFormatter = new Intl.NumberFormat("en-IN", {
    style: "currency",
    currency: "INR",
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
});

const inrDecimalFormatter = new Intl.NumberFormat("en-IN", {
    style: "currency",
    currency: "INR",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
});

const percentFormatter = new Intl.NumberFormat("en-IN", {
    style: "percent",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
});

export function useCurrencyFormat() {
    const formatINR = (value: number) => inrFormatter.format(value);
    const formatINRDecimal = (value: number) => inrDecimalFormatter.format(value);
    const formatPercent = (value: number) => percentFormatter.format(value / 100);
    const formatChange = (value: number) => {
        const sign = value >= 0 ? "+" : "";
        return sign + formatINR(value);
    };

    const formatCompact = (value: number) => {
        if (Math.abs(value) >= 1e7) return `₹${(value / 1e7).toFixed(2)}Cr`;
        if (Math.abs(value) >= 1e5) return `₹${(value / 1e5).toFixed(2)}L`;
        return formatINR(value);
    };

    return { formatINR, formatINRDecimal, formatPercent, formatChange, formatCompact };
}
