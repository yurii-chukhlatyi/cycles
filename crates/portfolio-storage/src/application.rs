//! Shared application operations used by the desktop UI and recovery tooling.
use crate::{Change, Error, Journal, Prepared, Result};
use portfolio_core::*;
use rust_decimal::Decimal as D;
use serde_json::Value;

pub fn stage(
    journal: &Journal,
    expected: &str,
    selected_cycle: Option<&str>,
    changes: Vec<Change>,
) -> Result<Prepared> {
    let state = journal.replay()?;
    if state.head != expected {
        return Err(Error::Conflict(
            "Source changed. Refresh and preview your retained draft again.".into(),
        ));
    }
    if changes.is_empty() {
        return Err(Error::Corrupt("There are no changes to save".into()));
    }
    for change in &changes {
        let owner = match change.table.as_str() {
            "Transactions" => state
                .portfolio
                .transactions
                .iter()
                .find(|x| x.id == change.id)
                .and_then(|x| state.portfolio.position(&x.position_id).ok())
                .map(|p| p.cycle_id.as_str()),
            "Positions" => state
                .portfolio
                .positions
                .iter()
                .find(|x| x.id == change.id)
                .map(|p| p.cycle_id.as_str()),
            "CashMovements" => state
                .portfolio
                .cash_movements
                .iter()
                .find(|x| x.id == change.id)
                .map(|p| p.cycle_id.as_str()),
            "Reservations" => state
                .portfolio
                .reservations
                .iter()
                .find(|x| x.id == change.id)
                .map(|p| p.cycle_id.as_str()),
            _ => None,
        };
        let new_owner = change
            .value
            .as_ref()
            .and_then(|v| v.get("cycle_id"))
            .and_then(Value::as_str);
        if matches!(
            change.table.as_str(),
            "Transactions" | "Positions" | "CashMovements" | "Reservations"
        ) {
            let selected = selected_cycle.ok_or_else(|| {
                Error::Corrupt("Select the owning cycle before changing its records".into())
            })?;
            if owner.is_some_and(|x| x != selected) || new_owner.is_some_and(|x| x != selected) {
                return Err(Error::Corrupt(
                    "This record belongs to a different cycle".into(),
                ));
            }
            if change.table == "Transactions" {
                if let Some(pid) = change
                    .value
                    .as_ref()
                    .and_then(|v| v.get("position_id"))
                    .and_then(Value::as_str)
                {
                    let candidate_owner = state
                        .portfolio
                        .positions
                        .iter()
                        .find(|x| x.id == pid)
                        .map(|x| x.cycle_id.as_str())
                        .or_else(|| {
                            changes
                                .iter()
                                .find(|x| x.table == "Positions" && x.id == pid)
                                .and_then(|x| x.value.as_ref())
                                .and_then(|x| x.get("cycle_id"))
                                .and_then(Value::as_str)
                        });
                    if candidate_owner != Some(selected) {
                        return Err(Error::Corrupt(
                            "The transaction must belong to the selected cycle's position".into(),
                        ));
                    }
                }
            }
        }
    }
    crate::prepare(&state, changes, &new_id(), "cycles-desktop")
}

pub fn change_cycle(
    p: &Portfolio,
    id: &str,
    state: CycleState,
    on: &str,
    head: &str,
) -> Result<Portfolio> {
    date(on)?;
    let mut next = p.clone();
    let c = next
        .cycles
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| Error::Corrupt("Unknown cycle".into()))?;
    if on < c.start_date.as_str() {
        return Err(Error::Corrupt("Cycle end precedes its start".into()));
    }
    if c.state == CycleState::Closed {
        return Err(Error::Corrupt("This cycle is already closed".into()));
    }
    if c.state == CycleState::Active && state != CycleState::Active {
        c.stopped_on = Some(on.into());
    }
    c.state = state.clone();
    if state == CycleState::Closed {
        c.closed_on = Some(on.into());
    }
    let f = ReportFilter {
        cycle_id: Some(id.into()),
        ..ReportFilter::at(on)
    };
    for a in &p.accounts {
        for currency in &a.currencies {
            if next.snapshots.iter().any(|s| {
                s.cycle_id.as_deref() == Some(id)
                    && s.account_id.as_deref() == Some(&a.id)
                    && s.currency == *currency
                    && s.phase == "stop"
            }) {
                continue;
            }
            let filter = ReportFilter {
                account_id: Some(a.id.clone()),
                ..f.clone()
            };
            let value = equity(p, &filter, currency, on, true)?.ok_or_else(|| {
                Error::Corrupt(format!(
                    "Enter dated {on} quotes before stopping this cycle"
                ))
            })?;
            let snapshot_id = new_id();
            next.snapshots.push(Snapshot {
                id: snapshot_id.clone(),
                date: on.into(),
                cycle_id: Some(id.into()),
                account_id: Some(a.id.clone()),
                currency: currency.clone(),
                equity: value,
                phase: "stop".into(),
                source: "Cycle stop".into(),
                input_head: head.into(),
                quote_ids: p
                    .market_data
                    .iter()
                    .filter(|q| q.date == on)
                    .map(|q| q.id.clone())
                    .collect(),
            });
            if let Some(c) = next.cycles.iter_mut().find(|c| c.id == id) {
                c.stop_snapshot_id = Some(snapshot_id);
            }
        }
    }
    validate_transition(p, &next)?;
    Ok(next)
}

/// An explicit sample dataset, never substituted for a user's spreadsheet.
pub fn sample_portfolio() -> Portfolio {
    let mut p = Portfolio::example();
    p.positions[0].stop_price = Some(D::from(90));
    p.instruments[0].symbol = "ACME".into();
    p.cycles[0].strategy = "Long-term quality equities with defined exit levels".into();
    p.cash_movements.push(CashMovement::funding(
        "sample-initial",
        "account",
        "cycle",
        "USD",
        "2026-01-02",
        D::from(25000),
    ));
    p.transactions.push(Transaction::trade(
        "sample-open",
        "pos",
        "2026-01-05",
        1,
        Action::Open,
        D::from(100),
        D::from(100),
        D::from(2),
    ));
    p.transactions.push(Transaction::trade(
        "sample-close",
        "pos",
        "2026-09-01",
        1,
        Action::Close,
        D::from(20),
        D::from(119),
        D::ONE,
    ));
    let mut deposit = CashMovement::funding(
        "sample-deposit",
        "account",
        "cycle",
        "USD",
        "2026-06-01",
        D::from(5000),
    );
    deposit.kind = CashKind::Deposit;
    p.cash_movements.push(deposit);
    let mut d = date("2026-01-02").expect("sample date");
    let end = date("2026-11-09").expect("sample date");
    let mut n = 0i64;
    use chrono::Datelike;
    while d <= end {
        if !matches!(d.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
            let price = D::from(100) + D::new(n, 1) + D::new((n % 9) - 4, 1);
            p.market_data.push(MarketData::quote(
                &format!("sample-q-{n}"),
                "stock",
                &d.to_string(),
                price,
            ));
            n += 1;
        }
        d = d.succ_opt().expect("sample date");
    }
    p
}

/// Capture a dated close or an explicitly supplied flow-time valuation.
pub fn valuation_changes(
    p: &Portfolio,
    f: &ReportFilter,
    phase: &str,
    flow_id: &str,
    manual: Option<D>,
    currency: Option<&str>,
    head: &str,
) -> Result<Vec<Change>> {
    if !matches!(phase, "close" | "baseline" | "pre_flow" | "post_flow") {
        return Err(Error::Corrupt("Invalid valuation phase".into()));
    }
    let flow_time = phase.ends_with("flow");
    if flow_time && (manual.is_none() || currency.is_none()) {
        return Err(Error::Corrupt(
            "Enter the measured flow-time equity and its currency".into(),
        ));
    }
    if manual.is_some_and(|v| v.scale() > 12) {
        return Err(Error::Corrupt("Enter at most 12 decimal places".into()));
    }
    let mut changes = vec![];
    for total in report(p, f)?.totals {
        if flow_time && currency != Some(total.currency.as_str()) {
            continue;
        }
        let value = if flow_time {
            let flows = capital_flows(p, f, &total.currency, false)?;
            if !flows.iter().any(|x| x.id == flow_id && x.date == f.as_of) {
                return Err(Error::Corrupt(
                    "Choose a capital movement in this scope on the selected date".into(),
                ));
            }
            manual.expect("checked")
        } else {
            equity(p, f, &total.currency, &f.as_of, true)?.ok_or_else(|| {
                Error::Corrupt(format!(
                    "Enter quotes dated {} for every open asset",
                    f.as_of
                ))
            })?
        };
        let existing = p.snapshots.iter().find(|s| {
            s.date == f.as_of
                && s.cycle_id == f.cycle_id
                && s.account_id == f.account_id
                && s.currency == total.currency
                && s.phase == phase
                && (!flow_time || s.source == flow_id)
        });
        let snapshot = Snapshot {
            id: existing.map(|s| s.id.clone()).unwrap_or_else(new_id),
            date: f.as_of.clone(),
            cycle_id: f.cycle_id.clone(),
            account_id: f.account_id.clone(),
            currency: total.currency,
            equity: value,
            phase: phase.into(),
            source: if flow_time {
                flow_id.into()
            } else {
                "Manual valuation".into()
            },
            input_head: head.into(),
            quote_ids: p
                .market_data
                .iter()
                .filter(|q| q.date == f.as_of)
                .map(|q| q.id.clone())
                .collect(),
        };
        changes.push(crate::snapshot_change(&snapshot)?);
    }
    if changes.is_empty() {
        return Err(Error::Corrupt(
            "No matching currency bucket for this valuation".into(),
        ));
    }
    Ok(changes)
}
