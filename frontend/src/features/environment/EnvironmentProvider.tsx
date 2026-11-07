"use client";

import { createContext, useContext, useEffect, useMemo, useState } from "react";

export type TradingEnvironment = "LIVE" | "DEMO" | "PAPER";
type EnvironmentContextValue = {
  environment: TradingEnvironment;
  setEnvironment: (environment: TradingEnvironment) => void;
};

const STORAGE_KEY = "edgenest.trading-environment";
const EnvironmentContext = createContext<EnvironmentContextValue | null>(null);

function isEnvironment(value: string | null): value is TradingEnvironment {
  return value === "LIVE" || value === "DEMO" || value === "PAPER";
}

export function EnvironmentProvider({ children }: { children: React.ReactNode }) {
  const [environment, setEnvironmentState] = useState<TradingEnvironment>("PAPER");

  useEffect(() => {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (isEnvironment(saved)) setEnvironmentState(saved);
  }, []);

  const setEnvironment = (next: TradingEnvironment) => {
    setEnvironmentState(next);
    window.localStorage.setItem(STORAGE_KEY, next);
    window.dispatchEvent(new CustomEvent("edgenest:environment-changed", { detail: next }));
  };

  const value = useMemo(() => ({ environment, setEnvironment }), [environment]);
  return <EnvironmentContext.Provider value={value}>{children}</EnvironmentContext.Provider>;
}

export function useTradingEnvironment() {
  const context = useContext(EnvironmentContext);
  if (!context) throw new Error("useTradingEnvironment must be used inside EnvironmentProvider");
  return context;
}
