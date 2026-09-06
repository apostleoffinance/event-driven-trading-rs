"use client";

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import type { TradingEvent } from "@/types/domain";

type SimState = {
  running: boolean;
  pause: () => void;
  resume: () => void;
  reset: () => void;
  generateSignal: () => void;
  generateTrade: () => void;
};

const Ctx = createContext<SimState | null>(null);

export function SimulationProvider({
  children,
  pushEvent,
  resetEvents,
}: {
  children: React.ReactNode;
  pushEvent: (e: TradingEvent) => void;
  resetEvents: () => void;
}) {
  const [running, setRunning] = useState(true);
  const [seq, setSeq] = useState(1);

  const generateSignal = useCallback(() => {
    const n = seq + 1;
    setSeq(n);
    const intentId = `ti-sim-${n}`;
    pushEvent({
      id: `evt-sim-sig-${n}`,
      timestamp: new Date().toISOString(),
      type: "StrategySignalGenerated",
      strategyId: "btc-mean-reversion",
      instrumentId: "EURUSD",
      detail: "Simulated mean-reversion signal (PAPER)",
    });
    pushEvent({
      id: `evt-sim-intent-${n}`,
      timestamp: new Date().toISOString(),
      type: "TradeIntentCreated",
      strategyId: "btc-mean-reversion",
      instrumentId: "EURUSD",
      correlationId: intentId,
      entity: intentId,
      detail: "TradeIntent emitted — not an order",
    });
  }, [pushEvent, seq]);

  const generateTrade = useCallback(() => {
    const n = seq + 1;
    setSeq(n);
    const intentId = `ti-sim-${n}`;
    const orderId = `ord-sim-${n}`;
    const now = new Date().toISOString();
    const chain: TradingEvent[] = [
      {
        id: `evt-sim-i-${n}`,
        timestamp: now,
        type: "TradeIntentCreated",
        correlationId: intentId,
        strategyId: "btc-mean-reversion",
        instrumentId: "EURUSD",
        entity: intentId,
        detail: "Simulated intent",
      },
      {
        id: `evt-sim-r-${n}`,
        timestamp: now,
        type: "RiskApproved",
        correlationId: intentId,
        strategyId: "btc-mean-reversion",
        instrumentId: "EURUSD",
        accountId: "prop-account-001",
        detail: "Risk resized then approved (mock)",
      },
      {
        id: `evt-sim-o-${n}`,
        timestamp: now,
        type: "OrderCreated",
        correlationId: intentId,
        entity: orderId,
        accountId: "prop-account-001",
        instrumentId: "EURUSD",
        detail: "OMS created order after risk",
      },
      {
        id: `evt-sim-s-${n}`,
        timestamp: now,
        type: "OrderSubmitted",
        correlationId: intentId,
        entity: orderId,
        detail: "Submitted to PropVenue",
      },
      {
        id: `evt-sim-f-${n}`,
        timestamp: now,
        type: "OrderFilled",
        correlationId: intentId,
        entity: orderId,
        status: "Filled",
        detail: "Paper fill",
      },
      {
        id: `evt-sim-p-${n}`,
        timestamp: now,
        type: "PositionUpdated",
        accountId: "prop-account-001",
        instrumentId: "EURUSD",
        detail: "Position updated after fill",
      },
    ];
    chain.reverse().forEach(pushEvent);
  }, [pushEvent, seq]);

  const value = useMemo(
    () => ({
      running,
      pause: () => setRunning(false),
      resume: () => setRunning(true),
      reset: () => {
        resetEvents();
        setSeq(1);
        setRunning(true);
      },
      generateSignal,
      generateTrade,
    }),
    [running, generateSignal, generateTrade, resetEvents],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useSimulation(): SimState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useSimulation requires SimulationProvider");
  return ctx;
}
