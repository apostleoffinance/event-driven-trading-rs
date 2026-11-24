"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { getSessionUser } from "@/lib/auth/session";
import { usePathname } from "next/navigation";

export type PerformanceDisplayMode = "currency" | "percentage" | "r" | "hidden";
type PerformanceDisplayContextValue = {
  mode: PerformanceDisplayMode;
  setMode: (mode: PerformanceDisplayMode) => void;
  ready: boolean;
};
const STORAGE_PREFIX = "edgenest.performance-display.v1:";
const PerformanceDisplayContext = createContext<PerformanceDisplayContextValue | null>(null);

export function PerformanceDisplayProvider({ children }: { children: React.ReactNode }) {
  const [mode, setModeState] = useState<PerformanceDisplayMode>("currency");
  const [ready, setReady] = useState(false);
  const [scope, setScope] = useState("anonymous");
  const pathname = usePathname();

  useEffect(() => {
    const userScope = encodeURIComponent(getSessionUser()?.email ?? "anonymous");
    setScope(userScope);
    try {
      const saved = window.localStorage.getItem(STORAGE_PREFIX + userScope);
      if (saved === "currency" || saved === "percentage" || saved === "r" || saved === "hidden") setModeState(saved);
    } catch { /* Preferences remain usable for this session if storage is unavailable. */ }
    setReady(true);
  }, [pathname]);

  const setMode = useCallback((next: PerformanceDisplayMode) => {
    setModeState(next);
    try { window.localStorage.setItem(STORAGE_PREFIX + scope, next); } catch { /* Keep the in-memory preference. */ }
  }, [scope]);

  const value = useMemo(() => ({ mode, setMode, ready }), [mode, setMode, ready]);
  return <PerformanceDisplayContext.Provider value={value}>{children}</PerformanceDisplayContext.Provider>;
}

export function usePerformanceDisplay() {
  const value = useContext(PerformanceDisplayContext);
  if (!value) throw new Error("usePerformanceDisplay must be used within PerformanceDisplayProvider");
  return value;
}

export function formatPerformanceValue(
  value: string | number | null | undefined,
  currency: string,
  mode: PerformanceDisplayMode,
  alternatives: { percentage?: number | null; rMultiple?: number | string | null } = {},
) {
  if (value == null || value === "") return "—";
  const amount = Number(value);
  if (!Number.isFinite(amount)) return "—";
  if (mode === "hidden") return "••••••";
  if (mode === "percentage") return alternatives.percentage == null || !Number.isFinite(alternatives.percentage)
    ? "—" : `${alternatives.percentage > 0 ? "+" : ""}${alternatives.percentage.toFixed(2)}%`;
  if (mode === "r") {
    const r = alternatives.rMultiple == null ? NaN : Number(alternatives.rMultiple);
    return Number.isFinite(r) ? `${r > 0 ? "+" : ""}${r.toFixed(2)}R` : "—";
  }
  try {
    return new Intl.NumberFormat("en-US", { style: "currency", currency, minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(amount);
  } catch {
    return `${currency} ${amount.toFixed(2)}`;
  }
}
