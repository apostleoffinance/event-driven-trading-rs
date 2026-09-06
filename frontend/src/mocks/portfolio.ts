import type { PortfolioSnapshot } from "@/types/domain";

export const mockPortfolio: PortfolioSnapshot = {
  totalEquity: "100482.31",
  balance: "99850.00",
  availableCapital: "65210.40",
  dailyPnl: "482.31",
  unrealizedPnl: "632.31",
  realizedPnl: "-150.00",
  cumulativeReturnPct: 4.82,
  drawdownPct: 2.31,
  maxDrawdownPct: 3.85,
  grossExposure: "35271.91",
  netExposure: "35271.91",
  exposurePct: 35.1,
  equitySeries: [
    { time: "09:00", equity: "100000.00", realizedPnl: "0.00", drawdownPct: 0 },
    { time: "10:00", equity: "100120.00", realizedPnl: "40.00", drawdownPct: 0.1 },
    { time: "11:00", equity: "99880.00", realizedPnl: "-40.00", drawdownPct: 0.24 },
    { time: "12:00", equity: "100210.00", realizedPnl: "80.00", drawdownPct: 0.35 },
    { time: "13:00", equity: "100050.00", realizedPnl: "20.00", drawdownPct: 0.9 },
    { time: "14:00", equity: "100340.00", realizedPnl: "90.00", drawdownPct: 1.2 },
    { time: "15:00", equity: "100290.00", realizedPnl: "40.00", drawdownPct: 1.8 },
    { time: "16:00", equity: "100410.00", realizedPnl: "-20.00", drawdownPct: 2.1 },
    { time: "17:00", equity: "100482.31", realizedPnl: "-150.00", drawdownPct: 2.31 },
  ],
  pnlByStrategy: [
    { strategyId: "btc-mean-reversion", pnl: "482.31" },
    { strategyId: "es-mean-reversion", pnl: "0.00" },
  ],
  pnlByInstrument: [
    { instrumentId: "EURUSD", pnl: "312.10" },
    { instrumentId: "BTCUSDT", pnl: "170.21" },
  ],
};
