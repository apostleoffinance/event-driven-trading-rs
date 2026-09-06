import type { Position } from "@/types/domain";

export const mockPositions: Position[] = [
  {
    id: "pos-1",
    accountId: "prop-account-001",
    strategyId: "btc-mean-reversion",
    instrumentId: "EURUSD",
    side: "Long",
    quantity: "0.42",
    entryPrice: "1.08420",
    markPrice: "1.08610",
    unrealizedPnl: "318.40",
    realizedPnl: "0.00",
    exposure: "18240.00",
    riskPct: 1.14,
    openedAt: "2026-09-06T14:02:11.000Z",
  },
  {
    id: "pos-2",
    accountId: "prop-account-001",
    strategyId: "btc-mean-reversion",
    instrumentId: "BTCUSDT",
    side: "Long",
    quantity: "0.18",
    entryPrice: "94210.00",
    markPrice: "94820.00",
    unrealizedPnl: "313.91",
    realizedPnl: "-40.00",
    exposure: "17031.91",
    riskPct: 0.92,
    openedAt: "2026-09-06T15:41:03.000Z",
  },
];
