/** Presentation-only numeric formatters. Keep raw decimal strings in persistence and calculations. */
export function formatNumber(value: string | number | null | undefined, maximumFractionDigits = 8): string {
  if (value == null || value === "") return "—";
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return typeof value === "string" ? value : "—";
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: Math.max(0, Math.min(12, maximumFractionDigits)),
    minimumFractionDigits: 0,
    useGrouping: true,
  }).format(n);
}

export function formatPrice(value: string | number | null | undefined): string {
  return formatNumber(value, 8);
}

export function formatQuantity(value: string | number | null | undefined): string {
  return formatNumber(value, 8);
}

export function formatMoney(value: string | number, currency = "USD"): string {
  const n = Number(value);
  if (!Number.isFinite(n)) return String(value);
  try {
    return new Intl.NumberFormat("en-US", { style: "currency", currency, minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(n);
  } catch {
    return `${currency} ${formatNumber(n, 2)}`;
  }
}

export function formatSignedMoney(value: string | number, currency = "USD"): string {
  const n = Number(value);
  if (!Number.isFinite(n)) return String(value);
  const abs = formatMoney(Math.abs(n), currency);
  if (n > 0) return `+${abs}`;
  if (n < 0) return `-${abs}`;
  return abs;
}

export function formatPct(value: number, digits = 2): string {
  if (!Number.isFinite(value)) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(digits)}%`;
}

export function formatR(value: string | number | null | undefined, digits = 2): string {
  if (value == null || value === "") return "—";
  const n = Number(value);
  return Number.isFinite(n) ? `${n.toFixed(digits)}R` : String(value);
}

export function formatTime(iso: string): string {
  const d = new Date(iso);
  if (!Number.isFinite(d.getTime())) return "—";
  return d.toLocaleTimeString("en-GB", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit", fractionalSecondDigits: 3 } as Intl.DateTimeFormatOptions);
}

export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (!Number.isFinite(d.getTime())) return "—";
  return d.toLocaleString("en-GB", { hour12: false, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

/** Display helper only — never use Number for monetary arithmetic in domain logic. */
export function moneyToNumber(value: string): number {
  return Number(value);
}
