export type Money = string | number;
export type Row = { id: string; [key: string]: unknown };
export type Account = Row & {
  name: string;
  broker: string;
  currencies: string[];
};
export type Cycle = Row & {
  name: string;
  strategy: string;
  start_date: string;
  state: "active" | "winding_down" | "closed";
  stopped_on?: string;
  closed_on?: string;
};
export type Instrument = Row & {
  symbol: string;
  asset: string;
  currency: string;
  multiplier: Money;
  option_right?: string;
  strike?: Money;
  expiry?: string;
  underlying?: string;
  settlement: string;
};
export type Position = Row & {
  instrument_id: string;
  cycle_id: string;
  account_id: string;
  direction: string;
  name: string;
  stop_price?: Money;
  covered_by?: string;
  notes: string;
};
export type Transaction = Row & {
  position_id: string;
  date: string;
  order: number;
  action: string;
  quantity: Money;
  price: Money;
  fee: Money;
  notes: string;
  settlement_id?: string;
};
export type CashMovement = Row & {
  account_id: string;
  cycle_id: string;
  currency: string;
  date: string;
  kind: string;
  amount: Money;
  destination_cycle_id?: string;
  destination_account_id?: string;
  receive_currency?: string;
  receive_amount?: Money;
  notes: string;
};
export type MarketData = Row & {
  instrument_id: string;
  date: string;
  price: Money;
  delta?: Money;
  underlying_price?: Money;
  adjusted: boolean;
  source: string;
};
export type Portfolio = {
  dataset_id: string;
  accounts: Account[];
  cycles: Cycle[];
  positions: Position[];
  instruments: Instrument[];
  transactions: Transaction[];
  cash_movements: CashMovement[];
  market_data: MarketData[];
  reservations: Row[];
  fx_rates: Row[];
  snapshots: Row[];
  settings: {
    reporting_currency?: string;
    volatility_window: number;
    holidays: string[];
    calendar: string;
    timezone: string;
  };
};
export type LedgerRow = {
  transaction: Transaction;
  quantity_after: Money;
  basis_after: Money;
  realized: Money;
  cash_delta: Money;
};
export type PositionView = {
  position: Position;
  instrument: Instrument;
  cycle_name: string;
  account_name: string;
  quantity: Money;
  opened: Money;
  closed: Money;
  basis: Money;
  average_entry?: Money;
  realized: Money;
  unrealized?: Money;
  total_pl?: Money;
  period_pl?: Money;
  price?: Money;
  quote_date?: string;
  market_value?: Money;
  planned_risk?: Money;
  delta_exposure?: Money;
  maximum_loss?: Money;
  unbounded: boolean;
  status: string;
  stale: boolean;
  ledger: LedgerRow[];
};
export type Totals = {
  currency: string;
  cash: Money;
  market_value?: Money;
  equity?: Money;
  realized: Money;
  unrealized?: Money;
  total_pl?: Money;
  period_pl?: Money;
  contributed: Money;
  external_contributions: Money;
  income: Money;
  expenses: Money;
  gross_exposure?: Money;
  net_exposure?: Money;
  planned_risk?: Money;
  delta_exposure?: Money;
  missing_quotes: number;
  missing_stops: number;
  missing_deltas: number;
  open_positions: number;
};
export type Report = {
  allocation: {
    position_id: string;
    symbol: string;
    currency: string;
    signed_value?: Money;
    gross_share?: Money;
  }[];
  attribution: {
    currency: string;
    from?: string;
    to: string;
    investment_gain?: Money;
    period_position_pl?: Money;
    income: Money;
    expenses: Money;
    reconciliation?: Money;
    note: string;
  }[];
  positions: PositionView[];
  totals: Totals[];
  cash_balances: {
    account_id: string;
    cycle_id: string;
    currency: string;
    balance: Money;
    reserved: Money;
    available?: Money;
    reasons: string[];
  }[];
  history: {
    date: string;
    currency: string;
    equity?: Money;
    capital?: Money;
    gain?: Money;
    source: string;
  }[];
  performance: {
    currency: string;
    from?: string;
    to: string;
    investment_gain?: Money;
    twr?: number;
    twr_method: string;
    xirr?: number;
    xirr_status: string;
    volatility?: number;
    observations: number;
    missing_periods: number;
  }[];
  asset_volatility: {
    instrument_id: string;
    volatility?: number;
    observations: number;
    method: string;
  }[];
  warnings: string[];
  as_of: string;
  from?: string;
};
export type Source =
  | { kind: "sample" }
  | { kind: "local"; path: string }
  | { kind: "google"; id: string };
export type Consolidated = {
  currency: string;
  equity?: Money;
  history: Report["history"];
  performance: Report["performance"][number];
  position_pl?: Money;
  income?: Money;
  expenses?: Money;
  reconciliation?: Money;
  currency_effect?: Money;
  warnings: string[];
};
export type AppState = {
  portfolio: Portfolio;
  head: string;
  revision: number;
  source: Source;
  report: Report;
  pending: { id: string; source: Source; created_at: string }[];
  conflicts: { command_id: string; reason: string }[];
  google_connected: boolean;
  notice?: string;
  consolidated?: Consolidated;
};
export type Filter = {
  cycle_id?: string;
  account_id?: string;
  currency?: string;
  as_of: string;
  from?: string;
};
export type Change = { table: string; id: string; value: unknown | null };
export type Preview = {
  cash_before?: Report["cash_balances"];
  cash_after?: Report["cash_balances"];
  prior_records?: { table: string; record_id: string; payload: unknown }[];
  command_id: string;
  changes: { table: string; record_id: string; payload: unknown }[];
  report: Report;
};
export type Legacy = {
  portfolio: Portfolio;
  source_hash: string;
  as_of: string;
  earliest_date: string;
  unsupported: string[];
  pending_settlements: string[];
  warnings: string[];
  cash_controls: Record<string, Money>;
  reported_quantities: Record<string, Money>;
};
export type Editor = { kind: string; row?: Row; position?: PositionView };
