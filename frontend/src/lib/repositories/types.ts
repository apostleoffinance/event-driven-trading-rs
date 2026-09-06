import type {
  Account,
  Order,
  PipelineNode,
  PortfolioSnapshot,
  Position,
  ReconciliationResult,
  RiskDecisionRecord,
  RiskOverview,
  Strategy,
  SystemComponent,
  TradingEvent,
} from "@/types/domain";

export interface AccountRepository {
  listAccounts(): Promise<Account[]>;
  getAccount(id: string): Promise<Account>;
}

export interface PortfolioRepository {
  getSnapshot(): Promise<PortfolioSnapshot>;
}

export interface StrategyRepository {
  listStrategies(): Promise<Strategy[]>;
  getStrategy(id: string): Promise<Strategy>;
}

export interface PositionRepository {
  listPositions(): Promise<Position[]>;
}

export interface OrderRepository {
  listOrders(): Promise<Order[]>;
  getOrder(id: string): Promise<Order>;
}

export interface RiskRepository {
  getOverview(): Promise<RiskOverview>;
  listDecisions(): Promise<RiskDecisionRecord[]>;
}

export interface EventRepository {
  listEvents(limit?: number): Promise<TradingEvent[]>;
}

export interface ReconciliationRepository {
  getLatest(): Promise<ReconciliationResult>;
}

export interface SystemRepository {
  listComponents(): Promise<SystemComponent[]>;
  getPipeline(): Promise<PipelineNode[]>;
}

export type DataRepositories = {
  accounts: AccountRepository;
  portfolio: PortfolioRepository;
  strategies: StrategyRepository;
  positions: PositionRepository;
  orders: OrderRepository;
  risk: RiskRepository;
  events: EventRepository;
  reconciliation: ReconciliationRepository;
  system: SystemRepository;
};
