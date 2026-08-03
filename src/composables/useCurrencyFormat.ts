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

import { computed } from "vue";

import { useUiStore } from "@/stores/ui";

const MASK = "₹ ••••••";

export function useCurrencyFormat() {
    const ui = useUiStore();
    // Reading ui.hideAmounts inside each formatter makes callers reactive:
    // toggling the store re-renders every amount without touching call sites.
    const formatINR = (value: number) =>
        ui.hideAmounts ? MASK : inrFormatter.format(value);
    const formatINRDecimal = (value: number) =>
        ui.hideAmounts ? MASK : inrDecimalFormatter.format(value);
    const formatPercent = (value: number) =>
        ui.hideAmounts ? "••••" : percentFormatter.format(value / 100);
    const formatChange = (value: number) => {
        if (ui.hideAmounts) return MASK;
        const sign = value >= 0 ? "+" : "";
        return sign + inrFormatter.format(value);
    };

    const formatCompact = (value: number) => {
        if (ui.hideAmounts) return MASK;
        if (Math.abs(value) >= 1e7) return `₹${(value / 1e7).toFixed(2)}Cr`;
        if (Math.abs(value) >= 1e5) return `₹${(value / 1e5).toFixed(2)}L`;
        return inrFormatter.format(value);
    };

    /**
     * Axis-tick formatter for Chart.js, handed out as a computed *value*.
     *
     * The formatters above are reactive only because a template re-renders and
     * calls them again. A Chart.js tick callback is different: it is stored inside
     * the options object and invoked at draw time, so reading `hideAmounts` inside
     * it registers no dependency and the axis keeps whatever it drew first — the
     * mask stayed on long after privacy mode was switched off.
     *
     * Taking `chartTick.value` while building a computed options object is what
     * ties the chart to the flag: the toggle produces a new callback, hence a new
     * options object, hence a redraw.
     */
    const chartTick = computed(() => {
        const masked = ui.hideAmounts;
        return (value: unknown) => (masked ? MASK : formatCompact(Number(value)));
    });

    return { formatINR, formatINRDecimal, formatPercent, formatChange, formatCompact, chartTick };
}
