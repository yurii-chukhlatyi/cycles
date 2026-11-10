use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Money = Decimal;
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
pub fn date(s: &str) -> crate::Result<NaiveDate> {
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| crate::Error::Invalid(format!("Invalid date: {s}")))?;
    if s.len() != 10 || d.to_string() != s {
        return Err(crate::Error::Invalid(format!(
            "Use an ISO date YYYY-MM-DD: {s}"
        )));
    }
    Ok(d)
}
fn one() -> Decimal {
    Decimal::ONE
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    Stock,
    Option,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
}
impl Direction {
    pub fn sign(&self) -> Decimal {
        if *self == Self::Long {
            Decimal::ONE
        } else {
            -Decimal::ONE
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptionRight {
    Call,
    Put,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CycleState {
    #[default]
    Active,
    WindingDown,
    Closed,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Settlement {
    #[default]
    Physical,
    Cash,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Open,
    Close,
    Expiry,
    Assignment,
    Exercise,
    Basis,
    Snapshot,
    Split,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CashKind {
    Initial,
    Deposit,
    Withdrawal,
    CycleTransfer,
    AccountTransfer,
    Fx,
    Dividend,
    Interest,
    Expense,
    Baseline,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub broker: String,
    pub currencies: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cycle {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub start_date: String,
    #[serde(default)]
    pub state: CycleState,
    #[serde(default)]
    pub stopped_on: Option<String>,
    #[serde(default)]
    pub closed_on: Option<String>,
    #[serde(default)]
    pub stop_snapshot_id: Option<String>,
}
impl Cycle {
    pub fn new(id: &str, name: &str, date: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            strategy: String::new(),
            start_date: date.into(),
            state: CycleState::Active,
            stopped_on: None,
            closed_on: None,
            stop_snapshot_id: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub id: String,
    pub symbol: String,
    pub asset: Asset,
    pub currency: String,
    #[serde(default = "one")]
    pub multiplier: Decimal,
    #[serde(default)]
    pub option_right: Option<OptionRight>,
    #[serde(default)]
    pub strike: Option<Decimal>,
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub underlying: Option<String>,
    #[serde(default)]
    pub settlement: Settlement,
    #[serde(default)]
    pub deliverable: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub instrument_id: String,
    pub account_id: String,
    pub cycle_id: String,
    pub direction: Direction,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub stop_price: Option<Decimal>,
    #[serde(default)]
    pub covered_by: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pending_lifecycle: bool,
    #[serde(default)]
    pub settlement_reconciled_on: Option<String>,
    #[serde(default)]
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub position_id: String,
    #[serde(default)]
    pub cycle_id: Option<String>,
    pub date: String,
    #[serde(default)]
    pub order: i64,
    pub action: Action,
    pub quantity: Decimal,
    pub price: Decimal,
    #[serde(default)]
    pub basis_total: Option<Decimal>,
    #[serde(default)]
    pub fee: Decimal,
    #[serde(default)]
    pub settlement_id: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub source: String,
}
impl Transaction {
    #[allow(clippy::too_many_arguments)]
    pub fn trade(
        id: &str,
        p: &str,
        date: &str,
        order: i64,
        action: Action,
        q: Decimal,
        price: Decimal,
        fee: Decimal,
    ) -> Self {
        Self {
            id: id.into(),
            position_id: p.into(),
            cycle_id: None,
            date: date.into(),
            order,
            action,
            quantity: q,
            price,
            basis_total: None,
            fee,
            settlement_id: None,
            notes: String::new(),
            source: String::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashMovement {
    pub id: String,
    pub account_id: String,
    pub cycle_id: String,
    pub currency: String,
    pub date: String,
    #[serde(default)]
    pub order: i64,
    pub kind: CashKind,
    pub amount: Decimal,
    #[serde(default)]
    pub destination_cycle_id: Option<String>,
    #[serde(default)]
    pub destination_account_id: Option<String>,
    #[serde(default)]
    pub receive_currency: Option<String>,
    #[serde(default)]
    pub receive_amount: Option<Decimal>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub source: String,
}
impl CashMovement {
    pub fn funding(id: &str, a: &str, c: &str, ccy: &str, date: &str, amount: Decimal) -> Self {
        Self {
            id: id.into(),
            account_id: a.into(),
            cycle_id: c.into(),
            currency: ccy.into(),
            date: date.into(),
            order: 0,
            kind: CashKind::Initial,
            amount,
            destination_cycle_id: None,
            destination_account_id: None,
            receive_currency: None,
            receive_amount: None,
            notes: String::new(),
            source: String::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub id: String,
    pub account_id: String,
    pub cycle_id: String,
    pub currency: String,
    pub amount: Decimal,
    pub start_date: String,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub position_id: Option<String>,
    pub notes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub id: String,
    pub instrument_id: String,
    pub date: String,
    pub price: Decimal,
    #[serde(default)]
    pub delta: Option<Decimal>,
    #[serde(default)]
    pub underlying_price: Option<Decimal>,
    #[serde(default)]
    pub adjusted: bool,
    #[serde(default)]
    pub source: String,
}
impl MarketData {
    pub fn quote(id: &str, i: &str, date: &str, price: Decimal) -> Self {
        Self {
            id: id.into(),
            instrument_id: i.into(),
            date: date.into(),
            price,
            delta: None,
            underlying_price: None,
            adjusted: false,
            source: "Manual".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxRate {
    pub id: String,
    pub from: String,
    pub to: String,
    pub date: String,
    pub rate: Decimal,
    #[serde(default)]
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub date: String,
    #[serde(default)]
    pub cycle_id: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    pub currency: String,
    pub equity: Decimal,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub input_head: String,
    #[serde(default)]
    pub quote_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub reporting_currency: Option<String>,
    pub volatility_window: usize,
    pub holidays: Vec<String>,
    pub calendar: String,
    pub timezone: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            reporting_currency: None,
            volatility_window: 60,
            holidays: vec![],
            calendar: "Weekdays; explicit holidays".into(),
            timezone: "America/Vancouver".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Portfolio {
    pub schema_version: u32,
    pub dataset_id: String,
    pub created_at: String,
    pub policy_version: u32,
    pub settings: Settings,
    pub accounts: Vec<Account>,
    pub cycles: Vec<Cycle>,
    pub instruments: Vec<Instrument>,
    pub positions: Vec<Position>,
    pub transactions: Vec<Transaction>,
    pub cash_movements: Vec<CashMovement>,
    pub reservations: Vec<Reservation>,
    pub market_data: Vec<MarketData>,
    pub fx_rates: Vec<FxRate>,
    pub snapshots: Vec<Snapshot>,
}
impl Default for Portfolio {
    fn default() -> Self {
        Self {
            schema_version: 1,
            dataset_id: new_id(),
            created_at: Utc::now().to_rfc3339(),
            policy_version: 1,
            settings: Settings::default(),
            accounts: vec![],
            cycles: vec![],
            instruments: vec![],
            positions: vec![],
            transactions: vec![],
            cash_movements: vec![],
            reservations: vec![],
            market_data: vec![],
            fx_rates: vec![],
            snapshots: vec![],
        }
    }
}
impl Portfolio {
    pub fn example() -> Self {
        let mut p = Self::default();
        p.accounts.push(Account {
            id: "account".into(),
            name: "Brokerage".into(),
            broker: String::new(),
            currencies: vec!["USD".into(), "CAD".into()],
        });
        p.cycles
            .push(Cycle::new("cycle", "2026 · Core strategy", "2026-01-01"));
        p.instruments.push(Instrument {
            id: "stock".into(),
            symbol: "TEST".into(),
            asset: Asset::Stock,
            currency: "USD".into(),
            multiplier: Decimal::ONE,
            option_right: None,
            strike: None,
            expiry: None,
            underlying: None,
            settlement: Settlement::Physical,
            deliverable: String::new(),
        });
        p.positions.push(Position {
            id: "pos".into(),
            instrument_id: "stock".into(),
            account_id: "account".into(),
            cycle_id: "cycle".into(),
            direction: Direction::Long,
            name: String::new(),
            stop_price: None,
            covered_by: None,
            notes: String::new(),
            pending_lifecycle: false,
            settlement_reconciled_on: None,
            source: String::new(),
        });
        p
    }
    pub fn account(&self, id: &str) -> crate::Result<&Account> {
        self.accounts
            .iter()
            .find(|x| x.id == id)
            .ok_or_else(|| crate::Error::Invalid(format!("Unknown account: {id}")))
    }
    pub fn cycle(&self, id: &str) -> crate::Result<&Cycle> {
        self.cycles
            .iter()
            .find(|x| x.id == id)
            .ok_or_else(|| crate::Error::Invalid(format!("Unknown cycle: {id}")))
    }
    pub fn position(&self, id: &str) -> crate::Result<&Position> {
        self.positions
            .iter()
            .find(|x| x.id == id)
            .ok_or_else(|| crate::Error::Invalid(format!("Unknown position: {id}")))
    }
    pub fn instrument(&self, id: &str) -> crate::Result<&Instrument> {
        self.instruments
            .iter()
            .find(|x| x.id == id)
            .ok_or_else(|| crate::Error::Invalid(format!("Unknown instrument: {id}")))
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportFilter {
    pub cycle_id: Option<String>,
    pub account_id: Option<String>,
    pub currency: Option<String>,
    pub as_of: String,
    pub from: Option<String>,
}
impl ReportFilter {
    pub fn at(date: &str) -> Self {
        Self {
            as_of: date.into(),
            ..Self::default()
        }
    }
    pub fn matches(&self, p: &Position, currency: &str) -> bool {
        self.cycle_id.as_ref().is_none_or(|x| x == &p.cycle_id)
            && self.account_id.as_ref().is_none_or(|x| x == &p.account_id)
            && self.currency.as_ref().is_none_or(|x| x == currency)
    }
}
