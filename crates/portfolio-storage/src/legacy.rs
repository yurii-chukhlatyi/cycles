use crate::{fingerprint, Error, Result};
use calamine::{Data, DataType, Reader, Xlsx};
use portfolio_core::*;
use rust_decimal::{Decimal as D, RoundingStrategy};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyPreview {
    pub portfolio: Portfolio,
    pub source_hash: String,
    pub as_of: String,
    pub earliest_date: String,
    pub unsupported: Vec<String>,
    pub pending_settlements: Vec<String>,
    pub warnings: Vec<String>,
    pub reported_quantities: BTreeMap<String, D>,
    pub cash_controls: BTreeMap<String, D>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningCash {
    pub account_id: String,
    pub cycle_id: String,
    pub currency: String,
    pub amount: D,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportChoices {
    pub position_cycles: BTreeMap<String, String>,
    pub cycles: Vec<Cycle>,
    pub cash: Vec<OpeningCash>,
    pub acknowledge_exclusions: bool,
    pub reconcile_historical_settlements: bool,
}
fn text(row: &[Data], col: usize) -> String {
    row.get(col)
        .map(|x| match x {
            Data::String(s) => s.clone(),
            Data::Empty => String::new(),
            _ => x.to_string(),
        })
        .unwrap_or_default()
}
fn amount(row: &[Data], col: usize) -> Result<D> {
    let s = text(row, col);
    if s.is_empty() {
        return Ok(D::ZERO);
    }
    D::from_str(&s)
        .or_else(|_| D::from_scientific(&s))
        .map(|x| x.round_dp_with_strategy(12, RoundingStrategy::MidpointAwayFromZero))
        .map_err(|_| Error::Corrupt(format!("Invalid numeric input in column {}", col + 1)))
}
fn optional(row: &[Data], col: usize) -> Result<Option<D>> {
    if text(row, col).is_empty() {
        Ok(None)
    } else {
        Ok(Some(amount(row, col)?))
    }
}
fn legacy_date(cell: &Data, is_1904: bool) -> Result<String> {
    if let Data::DateTime(t) = cell {
        if let Some(d) = t.as_datetime() {
            return Ok(d.date().to_string());
        }
    }
    if let Some(s) = cell.get_string() {
        let value = s.get(..10).unwrap_or(s);
        date(value)?;
        return Ok(value.into());
    }
    let n = cell
        .as_f64()
        .ok_or_else(|| Error::Corrupt("Missing or invalid legacy transaction date".into()))?;
    if !n.is_finite() || n < if is_1904 { 0.0 } else { 1.0 } || n > 2_900_000.0 || n.fract() != 0.0
    {
        return Err(Error::Corrupt("Unsupported Excel date".into()));
    }
    if is_1904 {
        return Ok((date("1904-01-01")? + chrono::Duration::days(n as i64)).to_string());
    }
    let base = date(if n < 60.0 { "1899-12-31" } else { "1899-12-30" })?;
    if n == 60.0 {
        return Err(Error::Corrupt(
            "Excel's fictitious 1900-02-29 cannot be imported".into(),
        ));
    }
    Ok((base + chrono::Duration::days(n as i64)).to_string())
}
pub fn preview_legacy(path: &Path, as_of: &str) -> Result<LegacyPreview> {
    date(as_of)?;
    if std::fs::metadata(path)?.len() > 100 * 1024 * 1024 {
        return Err(Error::Corrupt(
            "Legacy workbook exceeds the 100 MB import limit".into(),
        ));
    }
    let is_1904 = workbook_epoch(path)?;
    let source_hash = fingerprint(path)?;
    let source = format!("legacy:{source_hash}");
    let mut book: Xlsx<_> = calamine::open_workbook(path)
        .map_err(|e: calamine::XlsxError| Error::Corrupt(e.to_string()))?;
    let positions = book
        .worksheet_range("Positions")
        .map_err(|_| Error::Corrupt("Legacy Positions sheet not found".into()))?;
    let transactions = book
        .worksheet_range("Transactions")
        .map_err(|_| Error::Corrupt("Legacy Transactions sheet not found".into()))?;
    for (name, columns) in [
        ("Transactions", vec![0u32, 1, 2, 3, 4, 5, 6]),
        (
            "Positions",
            vec![0u32, 1, 2, 4, 5, 7, 8, 12, 13, 26, 27, 28, 29, 30],
        ),
    ] {
        let formulas = book
            .worksheet_formula(name)
            .map_err(|e| Error::Corrupt(e.to_string()))?;
        let (row0, col0) = formulas.start().unwrap_or((0, 0));
        for (row, col, formula) in formulas.cells() {
            let row = row + row0 as usize;
            let col = col + col0 as usize;
            let source_range = if name == "Positions" {
                &positions
            } else {
                &transactions
            };
            let populated = source_range
                .get_value((row as u32, 0))
                .is_some_and(|x| !x.to_string().is_empty());
            if populated && !formula.is_empty() && columns.contains(&(col as u32)) {
                return Err(Error::Corrupt(format!("{name} row {}: a required raw input contains a formula. Paste its verified raw value before importing.",row+1)));
            }
        }
    }
    let pr: Vec<_> = positions.rows().collect();
    let tr: Vec<_> = transactions.rows().collect();
    let ph = pr
        .iter()
        .position(|r| text(r, 0) == "Position ID")
        .ok_or_else(|| Error::Corrupt("Unrecognized legacy position headers".into()))?;
    let th = tr
        .iter()
        .position(|r| text(r, 0) == "Transaction ID")
        .ok_or_else(|| Error::Corrupt("Unrecognized legacy transaction headers".into()))?;
    let mut p = Portfolio::default();
    let mut unsupported = vec![];
    let mut reported = BTreeMap::new();
    let mut accounts = BTreeMap::<String, Account>::new();
    for row in pr.iter().skip(ph + 1) {
        let id = text(row, 0);
        if id.is_empty() {
            continue;
        }
        let asset = match text(row, 2).as_str() {
            "Stock" => Asset::Stock,
            "Option" => Asset::Option,
            other => {
                unsupported.push(format!("{id} · {other} (and its transactions)"));
                continue;
            }
        };
        let currency = text(row, 29).trim().to_string();
        if currency.is_empty() {
            return Err(Error::Corrupt(format!(
                "{id}: currency is missing; restore the raw report input before import"
            )));
        }
        if !["long", "short"].contains(&text(row, 4).to_lowercase().as_str()) {
            return Err(Error::Corrupt(format!(
                "{id}: unknown long/short direction"
            )));
        }
        if asset == Asset::Option
            && !["call", "put"].contains(&text(row, 5).to_lowercase().as_str())
        {
            return Err(Error::Corrupt(format!("{id}: unknown option right")));
        }
        let account_name = if text(row, 27).is_empty() {
            "Imported account".into()
        } else {
            text(row, 27)
        };
        let reference = text(row, 28);
        let key = format!("{account_name}|{reference}");
        let account_id = format!("import-account-{}", &crate::hash(&key)?[..12]);
        let a = accounts.entry(key).or_insert(Account {
            id: account_id.clone(),
            name: account_name,
            broker: if reference.is_empty() {
                String::new()
            } else {
                format!(
                    "…{}",
                    reference
                        .chars()
                        .rev()
                        .take(4)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                )
            },
            currencies: vec![],
        });
        if !a.currencies.contains(&currency) {
            a.currencies.push(currency.clone())
        }
        let instrument_id = format!("instrument:{id}");
        let ticker = text(row, 1);
        let symbol = if asset == Asset::Option && !text(row, 30).is_empty() {
            text(row, 30)
        } else {
            ticker.clone()
        };
        let multiplier =
            optional(row, 26)?
                .filter(|x| *x > D::ZERO)
                .unwrap_or(if asset == Asset::Option {
                    D::from(100)
                } else {
                    D::ONE
                });
        let expiry = if asset == Asset::Option {
            Some(legacy_date(row.get(8).unwrap_or(&Data::Empty), is_1904)?)
        } else {
            None
        };
        let ins = Instrument {
            id: instrument_id.clone(),
            symbol,
            asset: asset.clone(),
            currency: currency.clone(),
            multiplier,
            option_right: if asset == Asset::Option {
                Some(if text(row, 5).eq_ignore_ascii_case("put") {
                    OptionRight::Put
                } else {
                    OptionRight::Call
                })
            } else {
                None
            },
            strike: if asset == Asset::Option {
                Some(amount(row, 7)?)
            } else {
                None
            },
            expiry,
            underlying: if asset == Asset::Option {
                Some(ticker)
            } else {
                None
            },
            settlement: Settlement::Physical,
            deliverable: String::new(),
        };
        p.instruments.push(ins);
        p.positions.push(Position {
            id: id.clone(),
            instrument_id: instrument_id.clone(),
            account_id,
            cycle_id: "import-cycle".into(),
            direction: if text(row, 4).eq_ignore_ascii_case("short") {
                Direction::Short
            } else {
                Direction::Long
            },
            name: text(row, 3),
            stop_price: optional(row, 13)?,
            covered_by: None,
            notes: text(row, 22),
            pending_lifecycle: false,
            settlement_reconciled_on: None,
            source: source.clone(),
        });
        if !text(row, 32).is_empty() {
            reported.insert(id, amount(row, 32)?.abs());
        }
        if let Some(price) = optional(row, 12)? {
            p.market_data.push(MarketData {
                id: new_id(),
                instrument_id,
                date: as_of.into(),
                price,
                delta: optional(row, 20)?,
                underlying_price: None,
                adjusted: false,
                source: format!("Legacy report {as_of}"),
            });
        }
    }
    p.accounts = accounts.into_values().collect();
    let mut pending = vec![];
    for (n, row) in tr.iter().enumerate().skip(th + 1) {
        let id = text(row, 0);
        let pos_id = text(row, 1);
        if id.is_empty() || !p.positions.iter().any(|p| p.id == pos_id) {
            continue;
        }
        let mut action = match text(row, 3).as_str() {
            "OPEN" => {
                if text(row, 21).to_lowercase().contains("snapshot")
                    || text(row, 21).to_lowercase().contains("carry")
                {
                    Action::Snapshot
                } else {
                    Action::Open
                }
            }
            "CLOSE" => Action::Close,
            "BASIS" => Action::Basis,
            "ASSIGNMENT" => Action::Assignment,
            "EXERCISE" => Action::Exercise,
            "EXPIRATION" => Action::Expiry,
            other => {
                return Err(Error::Corrupt(format!(
                    "Unsupported legacy action {other} in row {}",
                    n + 1
                )))
            }
        };
        let position = p.position(&pos_id)?;
        let instrument = p.instrument(&position.instrument_id)?;
        if instrument.asset == Asset::Stock
            && matches!(action, Action::Assignment | Action::Exercise)
        {
            action = Action::Close;
        }
        let d = legacy_date(row.get(2).unwrap_or(&Data::Empty), is_1904)?;
        let mut t = Transaction::trade(
            &id,
            &pos_id,
            &d,
            n as i64,
            action.clone(),
            amount(row, 4)?,
            amount(row, 5)?,
            amount(row, 6)?,
        );
        t.notes = text(row, 8);
        t.source = format!(
            "{source}|{}|{}|{}|{}",
            text(row, 21),
            text(row, 20),
            text(row, 22),
            text(row, 3)
        );
        if matches!(action, Action::Assignment | Action::Exercise) {
            pending.push(pos_id.clone());
            if let Some(x) = p.positions.iter_mut().find(|p| p.id == pos_id) {
                x.pending_lifecycle = true;
            }
        }
        p.transactions.push(t);
    }
    let option_events: Vec<_> = p
        .transactions
        .iter()
        .filter(|t| matches!(t.action, Action::Assignment | Action::Exercise))
        .cloned()
        .collect();
    for event in option_events {
        let owner = p.position(&event.position_id)?;
        let instrument = p.instrument(&owner.instrument_id)?;
        let delivery = if (instrument.option_right == Some(OptionRight::Call))
            == (owner.direction == Direction::Long)
        {
            D::ONE
        } else {
            -D::ONE
        };
        let candidates: Vec<_> = p
            .transactions
            .iter()
            .filter(|t| {
                t.date == event.date
                    && t.action == Action::Close
                    && (t.source.ends_with("|ASSIGNMENT") || t.source.ends_with("|EXERCISE"))
                    && t.settlement_id.is_none()
            })
            .filter(|t| {
                p.position(&t.position_id).ok().is_some_and(|up| {
                    p.instrument(&up.instrument_id).ok().is_some_and(|ui| {
                        up.account_id == owner.account_id
                            && ui.asset == Asset::Stock
                            && ui.symbol == instrument.underlying.as_deref().unwrap_or_default()
                            && ui.currency == instrument.currency
                            && t.price == instrument.strike.unwrap_or_default()
                            && -up.direction.sign() * t.quantity
                                == delivery * event.quantity * instrument.multiplier
                    })
                })
            })
            .map(|t| t.id.clone())
            .collect();
        if candidates.len() == 1 {
            p.transactions
                .iter_mut()
                .find(|t| t.id == candidates[0])
                .expect("candidate")
                .settlement_id = Some(event.id);
            pending.retain(|id| id != &event.position_id);
        }
    }
    for pos in &mut p.positions {
        pos.pending_lifecycle = pending.contains(&pos.id);
    }
    let earliest = p
        .transactions
        .iter()
        .map(|x| x.date.clone())
        .min()
        .unwrap_or_else(|| as_of.into());
    p.cycles
        .push(Cycle::new("import-cycle", "Imported history", &earliest));
    let mut cash_controls = BTreeMap::new();
    if let Ok(dash) = book.worksheet_range("Dashboard") {
        let rows: Vec<_> = dash.rows().collect();
        if let Some(row) = rows
            .iter()
            .find(|r| text(r, 0).to_lowercase().contains("investment cash"))
        {
            cash_controls.insert("USD".into(), amount(row, 1)?);
            cash_controls.insert("CAD".into(), amount(row, 2)?);
        }
    }
    let mut warnings=vec!["Historical funding is incomplete. Returns begin only at your reconciled opening baseline.".into(),"Option prices in this tracker are already per underlying unit; no second normalization is applied.".into()];
    if !pending.is_empty() {
        warnings.push("Historical assignments have no linked underlying event. Confirm current holdings and cash reconciliation before declaring those old obligations settled.".into());
    }
    for (id, q) in &reported {
        let actual = position_ledger(&p, id, Some(as_of))?.quantity;
        if (actual - *q).abs() > D::new(1, 6) {
            warnings.push(format!(
                "{id}: remaining quantity does not match the report; resolve before import"
            ));
        }
    }
    if fingerprint(path)? != source_hash {
        return Err(Error::Conflict("Legacy file changed during preview".into()));
    }
    Ok(LegacyPreview {
        portfolio: p,
        source_hash,
        as_of: as_of.into(),
        earliest_date: earliest,
        unsupported,
        pending_settlements: pending,
        warnings,
        reported_quantities: reported,
        cash_controls,
    })
}
pub fn apply_legacy(
    existing: &Portfolio,
    preview: &LegacyPreview,
    choices: &ImportChoices,
) -> Result<Portfolio> {
    let source = format!("legacy:{}", preview.source_hash);
    if existing.positions.iter().any(|p| p.source == source) {
        return Err(Error::Conflict(
            "This exact tracker has already been imported".into(),
        ));
    }
    if !preview.unsupported.is_empty() && !choices.acknowledge_exclusions {
        return Err(Error::Corrupt(
            "Acknowledge the unsupported commodity records before proceeding".into(),
        ));
    }
    if !preview.pending_settlements.is_empty() && !choices.reconcile_historical_settlements {
        return Err(Error::Corrupt(
            "Confirm that the report's current holdings and cash settle the historical assignments"
                .into(),
        ));
    }
    let mut next = existing.clone();
    for c in &choices.cycles {
        if !next.cycles.iter().any(|x| x.id == c.id) {
            next.cycles.push(c.clone())
        }
    }
    for a in &preview.portfolio.accounts {
        if !next.accounts.iter().any(|x| x.id == a.id) {
            next.accounts.push(a.clone())
        }
    }
    for p in &preview.portfolio.positions {
        if next.positions.iter().any(|x| x.id == p.id) {
            return Err(Error::Corrupt(format!(
                "Position ID already exists: {}",
                p.id
            )));
        }
        let mut p = p.clone();
        p.cycle_id = choices
            .position_cycles
            .get(&p.id)
            .cloned()
            .ok_or_else(|| Error::Corrupt(format!("Assign {} to a cycle", p.id)))?;
        if p.pending_lifecycle {
            p.pending_lifecycle = false;
            p.settlement_reconciled_on = Some(preview.as_of.clone())
        }
        next.positions.push(p);
    }
    next.instruments
        .extend(preview.portfolio.instruments.clone());
    next.market_data
        .extend(preview.portfolio.market_data.clone());
    next.transactions
        .extend(preview.portfolio.transactions.clone());
    let mut allocations = BTreeMap::<String, D>::new();
    let mut keys = BTreeSet::new();
    for c in &choices.cash {
        if !keys.insert((&c.account_id, &c.cycle_id, &c.currency)) {
            return Err(Error::Corrupt("Duplicate opening cash allocation".into()));
        }
        let sum = allocations.entry(c.currency.clone()).or_default();
        *sum = sum
            .checked_add(c.amount)
            .ok_or(portfolio_core::Error::Overflow)?;
        let mut m = CashMovement::funding(
            &new_id(),
            &c.account_id,
            &c.cycle_id,
            &c.currency,
            &preview.as_of,
            c.amount,
        );
        m.kind = CashKind::Baseline;
        m.order = 2_147_483_647;
        m.source = source.clone();
        next.cash_movements.push(m);
    }
    for pos in &preview.portfolio.positions {
        let currency = preview
            .portfolio
            .instrument(&pos.instrument_id)?
            .currency
            .clone();
        let owner = choices
            .position_cycles
            .get(&pos.id)
            .ok_or_else(|| Error::Corrupt("Missing cycle allocation".into()))?;
        if !choices.cash.iter().any(|c| {
            c.account_id == pos.account_id && c.cycle_id == *owner && c.currency == currency
        }) {
            return Err(Error::Corrupt(format!("Enter an explicit opening cash balance, including zero, for {} / {owner} / {currency}",pos.account_id)));
        }
    }
    for (ccy, control) in &preview.cash_controls {
        if (allocations.get(ccy).copied().unwrap_or_default() - *control).abs() > D::new(1, 2) {
            return Err(Error::Corrupt(format!(
                "Opening {ccy} cash allocations must reconcile to {control}"
            )));
        }
    }
    for (id, expected) in &preview.reported_quantities {
        if (position_ledger(&next, id, Some(&preview.as_of))?.quantity - *expected).abs()
            > D::new(1, 6)
        {
            return Err(Error::Corrupt(format!(
                "Resolve the quantity mismatch for {id} before importing"
            )));
        }
    }
    validate_transition(existing, &next)?;
    let cycles: BTreeSet<_> = preview
        .portfolio
        .positions
        .iter()
        .filter_map(|p| choices.position_cycles.get(&p.id))
        .cloned()
        .collect();
    let currencies: BTreeSet<_> = next
        .accounts
        .iter()
        .flat_map(|a| a.currencies.clone())
        .collect();
    for cycle in cycles
        .iter()
        .map(|x| Some(x.clone()))
        .chain(std::iter::once(None))
    {
        for currency in &currencies {
            let f = ReportFilter {
                cycle_id: cycle.clone(),
                ..ReportFilter::at(&preview.as_of)
            };
            if let Some(value) = equity(&next, &f, currency, &preview.as_of, true)? {
                next.snapshots.push(Snapshot {
                    id: new_id(),
                    date: preview.as_of.clone(),
                    cycle_id: cycle.clone(),
                    account_id: None,
                    currency: currency.clone(),
                    equity: value,
                    phase: "baseline".into(),
                    source: source.clone(),
                    input_head: String::new(),
                    quote_ids: next
                        .market_data
                        .iter()
                        .filter(|q| q.date == preview.as_of)
                        .map(|q| q.id.clone())
                        .collect(),
                });
            }
        }
    }
    Ok(next)
}

fn workbook_epoch(path: &Path) -> Result<bool> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| Error::Corrupt(e.to_string()))?;
    let mut entry = archive
        .by_name("xl/workbook.xml")
        .map_err(|e| Error::Corrupt(e.to_string()))?;
    if entry.size() > 1_000_000 {
        return Err(Error::Corrupt("Workbook metadata is too large".into()));
    }
    let mut xml = String::new();
    entry.read_to_string(&mut xml)?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e))
                if e.local_name().as_ref() == b"workbookPr" =>
            {
                for a in e.attributes() {
                    let a = a.map_err(|e| Error::Corrupt(e.to_string()))?;
                    if a.key.as_ref() == b"date1904" {
                        return Ok(matches!(a.value.as_ref(), b"1" | b"true"));
                    }
                }
                return Ok(false);
            }
            Ok(quick_xml::events::Event::Eof) => return Ok(false),
            Err(e) => return Err(Error::Corrupt(e.to_string())),
            _ => {}
        }
    }
}
