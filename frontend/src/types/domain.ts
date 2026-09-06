/** Money as exact decimal strings — mirrors Rust Decimal / Postgres NUMERIC. */
export type Money = string;

export type Environment = "PAPER" | "SHADOW" | "LIVE";

export type AccountType = "Simulated" | "Prop" | "Personal" | "Cex" | "Vault";

export type AccountStatus = "Active" | "Suspended" | "Halted" | "Closed";

export type OrderSide = "Buy" | "Sell";

export type OrderType = "Market" | "Limit";

export type TimeInForce = "Day" | "Gtc" | "Ioc" | "Fok";

/** Domain order state machine (crates/domain). */
export type OrderStatus =
  | "Created"
  | "PendingRisk"
  | "Approved"
  | "Rejected"
  | "Submitted"
  | "Unknown"
  | "Accepted"
  | "PartiallyFilled"
  | "Filled"
  | "CancelPending"
  | "Cancelled"
  | "Failed";

export type PositionSide = "Long" | "Short";

export type StrategyStatus =
  | "RUNNING"
  | "PAUSED"
  | "STOPPED"
  | "ERROR"
  | "SHADOW"
  | "PAPER"
  | "LIVE";

export type HealthStatus = "HEALTHY" | "DEGRADED" | "DISCONNECTED" | "ERROR";

export type RiskTone = "HEALTHY" | "WARNING" | "BREACH";

export type VenueType = "Simulated" | "Prop" | "Cex" | "Onchain";

export type TradingEventType =
  | "MarketDataReceived"
  | "StrategySignalGenerated"
  | "TradeIntentCreated"
  | "RiskCheckRequested"
  | "RiskApproved"
  | "RiskResized"
  | "RiskRejected"
  | "TradingHalted"
  | "OrderCreated"
  | "OrderSubmitted"
  | "OrderAccepted"
  | "OrderRejected"
  | "OrderPartiallyFilled"
  | "OrderFilled"
  | "OrderCancelled"
  | "OrderFailed"
  | "OrderUnknown"
  | "MarketDataUnhealthy"
  | "PositionOpened"
  | "PositionUpdated"
  | "PositionClosed"
  | "AccountUpdated"
  | "RiskLimitBreached"
  | "ReconciliationStarted"
  | "ReconciliationCompleted"
  | "ReconciliationFailed"
  | "ReconciliationAlert"
  | "SystemError";

export type Account = {
  id: string;
  name: string;
  type: AccountType;
  venueId: string;
  venueName: string;
  environment: Environment;
  balance: Money;
  equity: Money;
  availableCapital: Money;
  dailyPnl: Money;
  unrealizedPnl: Money;
  realizedPnl: Money;
  drawdownPct: number;
  openPositionCount: number;
  status: AccountStatus;
  riskStatus: RiskTone;
  connectionStatus: HealthStatus;
};

export type Venue = {
  id: string;
  name: string;
  type: VenueType;
  status: HealthStatus;
};

export type Instrument = {
  id: string;
  symbol: string;
  assetClass: string;
  /** Market-structure rules when known (mirrors Rust InstrumentSpec). */
  tickSize?: Money;
  lotSize?: Money;
  minQuantity?: Money;
  minNotional?: Money;
};

export type Strategy = {
  id: string;
  name: string;
  version: string;
  status: StrategyStatus;
  environment: Environment;
  instrumentId: string;
  signals: number;
  trades: number;
  returnPct: number;
  sharpe: number;
  maxDrawdownPct: number;
  winRatePct: number;
  profitFactor: number;
  description: string;
  /** Config keys only — never secrets. */
  configuration: { key: string; value: string }[];
  recentIntentIds: string[];
};

export type TradeIntent = {
  id: string;
  strategyId: string;
  strategyVersion: string;
  deploymentId: string;
  instrumentId: string;
  side: OrderSide;
  targetQuantity?: Money;
  entryPrice?: Money;
  stopLoss?: Money;
  confidence?: Money;
  createdAt: string;
};

export type RiskDecisionKind = "APPROVED" | "RESIZED" | "REJECTED" | "HALTED";

export type RiskDecisionRecord = {
  id: string;
  time: string;
  tradeIntentId: string;
  strategyId: string;
  instrumentId: string;
  accountId: string;
  requestedQuantity?: Money;
  approvedQuantity?: Money;
  decision: RiskDecisionKind;
  reason: string;
};

export type Order = {
  id: string;
  clientOrderId: string;
  accountId: string;
  venueId: string;
  strategyId: string;
  tradeIntentId?: string;
  instrumentId: string;
  side: OrderSide;
  orderType: OrderType;
  timeInForce: TimeInForce;
  quantity: Money;
  price?: Money;
  filledQuantity: Money;
  averageFillPrice?: Money;
  status: OrderStatus;
  createdAt: string;
  submittedAt?: string;
  updatedAt: string;
};

export type Fill = {
  id: string;
  orderId: string;
  instrumentId: string;
  price: Money;
  quantity: Money;
  fee: Money;
  timestamp: string;
};

export type Position = {
  id: string;
  accountId: string;
  strategyId: string;
  instrumentId: string;
  side: PositionSide;
  quantity: Money;
  entryPrice: Money;
  markPrice: Money;
  unrealizedPnl: Money;
  realizedPnl: Money;
  exposure: Money;
  riskPct: number;
  openedAt: string;
};

export type EquityPoint = {
  time: string;
  equity: Money;
  realizedPnl: Money;
  drawdownPct: number;
};

export type PortfolioSnapshot = {
  totalEquity: Money;
  balance: Money;
  availableCapital: Money;
  dailyPnl: Money;
  unrealizedPnl: Money;
  realizedPnl: Money;
  cumulativeReturnPct: number;
  drawdownPct: number;
  maxDrawdownPct: number;
  grossExposure: Money;
  netExposure: Money;
  exposurePct: number;
  equitySeries: EquityPoint[];
  pnlByStrategy: { strategyId: string; pnl: Money }[];
  pnlByInstrument: { instrumentId: string; pnl: Money }[];
};

export type RiskOverview = {
  engineStatus: HealthStatus;
  dailyLossPct: number;
  dailyLossLimitPct: number;
  maxDrawdownPct: number;
  maxDrawdownLimitPct: number;
  openRiskPct: number;
  exposurePct: number;
  killSwitch: boolean;
  tone: RiskTone;
  hierarchy: {
    id: string;
    label: string;
    scope: string;
    status: HealthStatus;
    tone: RiskTone;
    utilizationPct: number;
    limitLabel: string;
    note: string;
  }[];
};

export type TradingEvent = {
  id: string;
  timestamp: string;
  type: TradingEventType;
  entity?: string;
  accountId?: string;
  strategyId?: string;
  instrumentId?: string;
  correlationId?: string;
  status?: string;
  detail: string;
  payload?: Record<string, string>;
};

export type ReconciliationLine = {
  field: string;
  internal: string;
  venue: string;
  status: "MATCH" | "MISMATCH";
  difference?: string;
};

export type ReconciliationResult = {
  accountId: string;
  venueId: string;
  status: "HEALTHY" | "ALERT";
  lines: ReconciliationLine[];
  alerts: { title: string; detail: string; action: "ALERT_ONLY" }[];
  lastRunAt: string;
};

export type SystemComponent = {
  id: string;
  name: string;
  status: HealthStatus;
  uptime: string;
  lastHeartbeat: string;
  lastEvent?: string;
  latencyMs: number;
  errorCount: number;
  eventsPerSec?: number;
};

export type PipelineNode = {
  id: string;
  label: string;
  status: HealthStatus;
  lastActivity: string;
  metric: string;
  href: string;
};
