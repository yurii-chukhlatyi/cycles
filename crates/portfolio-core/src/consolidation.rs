use crate::{decimal::*, *};
use rust_decimal::Decimal as D;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consolidated {
    pub currency: String,
    pub equity: Option<D>,
    pub history: Vec<HistoryPoint>,
    pub performance: PerformanceView,
    pub position_pl: Option<D>,
    pub income: Option<D>,
    pub expenses: Option<D>,
    pub reconciliation: Option<D>,
    pub currency_effect: Option<D>,
    pub warnings: Vec<String>,
}
fn native_value(
    p: &Portfolio,
    f: &ReportFilter,
    h: &[HistoryPoint],
    ccy: &str,
    d: &str,
) -> Result<Option<D>> {
    if let Some(point) = h.iter().find(|x| x.currency == ccy && x.date == d) {
        return Ok(point.equity);
    }
    equity(p, f, ccy, d, true)
}
fn converted(p: &Portfolio, v: Option<D>, from: &str, to: &str, d: &str) -> Result<Option<D>> {
    if v == Some(D::ZERO) {
        return Ok(v);
    }
    if from != to {
        use chrono::Datelike;
        let mut expected = date(d)?;
        while matches!(
            expected.weekday(),
            chrono::Weekday::Sat | chrono::Weekday::Sun
        ) || p.settings.holidays.contains(&expected.to_string())
        {
            expected = expected.pred_opt().ok_or(Error::Overflow)?;
        }
        if fx_evidence_date(p, from, to, d).is_none_or(|x| x < expected.to_string()) {
            return Ok(None);
        }
    }
    match (v, fx(p, from, to, d)) {
        (Some(v), Some(rate)) => Ok(Some(mul(v, rate)?)),
        (Some(v), _) if v == D::ZERO => Ok(Some(D::ZERO)),
        _ => Ok(None),
    }
}
fn sum_opt(a: Option<D>, b: Option<D>) -> Result<Option<D>> {
    match (a, b) {
        (Some(a), Some(b)) => Ok(Some(add(a, b)?)),
        _ => Ok(None),
    }
}
pub fn consolidated(
    p: &Portfolio,
    f: &ReportFilter,
    h: &[HistoryPoint],
    target: &str,
) -> Result<Consolidated> {
    let currencies: BTreeSet<_> = p
        .accounts
        .iter()
        .filter(|a| f.account_id.as_ref().is_none_or(|id| *id == a.id))
        .flat_map(|a| a.currencies.iter())
        .filter(|c| f.currency.as_ref().is_none_or(|id| id == *c))
        .cloned()
        .collect();
    let baseline = p
        .cash_movements
        .iter()
        .filter(|m| {
            m.kind == CashKind::Baseline && crate::reporting::scope(&m.account_id, &m.cycle_id, f)
        })
        .map(|m| m.date.as_str())
        .chain(
            p.snapshots
                .iter()
                .filter(|s| {
                    s.phase == "baseline"
                        && currencies.contains(&s.currency)
                        && s.cycle_id == f.cycle_id
                        && s.account_id == f.account_id
                })
                .map(|s| s.date.as_str()),
        )
        .max();
    let mut dated_flows = BTreeMap::<(String, String), D>::new();
    let mut missing_flow_dates = vec![];
    for ccy in &currencies {
        for flow in capital_flows(p, f, ccy, false)? {
            if p.cash_movements
                .iter()
                .any(|m| m.id == flow.id && m.kind == CashKind::Fx)
            {
                continue;
            }
            if let Some(v) = converted(p, Some(flow.amount), ccy, target, &flow.date)? {
                let entry = dated_flows.entry((flow.date, flow.id)).or_default();
                *entry = add(*entry, v)?;
            } else {
                missing_flow_dates.push(flow.date);
            }
        }
    }
    let flows: Vec<_> = dated_flows
        .into_iter()
        .map(|((date, id), amount)| CapitalFlow { id, date, amount })
        .filter(|x| x.amount != D::ZERO)
        .collect();
    let mut dates: BTreeSet<_> = h.iter().map(|x| x.date.clone()).collect();
    dates.insert(f.as_of.clone());
    let mut history = vec![];
    let mut first = None;
    let mut missing_valuation = false;
    for d in dates {
        if baseline.is_some_and(|b| d.as_str() < b) {
            continue;
        }
        let mut value = Some(D::ZERO);
        for ccy in &currencies {
            value = sum_opt(
                value,
                converted(p, native_value(p, f, h, ccy, &d)?, ccy, target, &d)?,
            )?;
        }
        if value.is_none() {
            missing_valuation = true;
        }
        if first.is_none() {
            if let Some(v) = value {
                first = Some((d.clone(), v));
            }
        }
        let capital = if let Some((start, initial)) = &first {
            Some(
                flows
                    .iter()
                    .filter(|x| x.date > *start && x.date <= d)
                    .try_fold(*initial, |s, x| add(s, x.amount))?,
            )
        } else {
            None
        };
        let missing_flow_fx = first.as_ref().is_some_and(|(start, _)| {
            missing_flow_dates
                .iter()
                .any(|date| date > start && date <= &d)
        });
        let gain = if missing_flow_fx {
            None
        } else {
            match (value, capital) {
                (Some(v), Some(c)) => Some(sub(v, c)?),
                _ => None,
            }
        };
        history.push(HistoryPoint {
            date: d,
            currency: target.into(),
            equity: value,
            capital: if missing_flow_fx { None } else { capital },
            gain,
            source: "Dated native valuations and as-of FX".into(),
        });
    }
    let missing_flow_fx = first.as_ref().is_some_and(|(start, _)| {
        missing_flow_dates
            .iter()
            .any(|date| date > start && date <= &f.as_of)
    });
    let mut performance =
        crate::performance::performance_with_flows(p, f, target, &history, &flows, false)?;
    if missing_flow_fx {
        performance.twr = None;
        performance.xirr = None;
        performance.volatility = None;
        performance.investment_gain = None;
        performance.twr_method = "Missing flow-date FX".into();
        performance.xirr_status = "Missing flow-date FX".into();
    }
    let mut warnings = vec![];
    if missing_flow_fx {
        warnings.push("Missing exchange rates on a capital-flow date; consolidated return and gain are unavailable.".into());
    }
    if missing_valuation {
        warnings.push(
            "Some consolidated dates lack complete prices, valuations or current dated FX evidence. Current equity can use older as-of marks/rates; historical statistics require dated evidence.".into(),
        );
    }
    let mut position_pl = Some(D::ZERO);
    let mut income = Some(D::ZERO);
    let mut expenses = Some(D::ZERO);
    let mut reconciliation = Some(D::ZERO);
    if let Some((start, _)) = &first {
        for pos in &p.positions {
            let i = p.instrument(&pos.instrument_id)?;
            if !f.matches(pos, &i.currency) {
                continue;
            }
            let a = position_ledger(p, &pos.id, Some(start))?;
            let b = position_ledger(p, &pos.id, Some(&f.as_of))?;
            let un = |l: &Ledger, d: &str| -> Result<Option<D>> {
                if l.quantity == D::ZERO {
                    return Ok(Some(D::ZERO));
                }
                quote(p, &i.id, d)
                    .filter(|q| q.date == d)
                    .map(|q| {
                        mul(
                            pos.direction.sign(),
                            sub(mul(mul(l.quantity, i.multiplier)?, q.price)?, l.basis)?,
                        )
                    })
                    .transpose()
            };
            let au = converted(p, un(&a, start)?, &i.currency, target, start)?;
            let bu = converted(p, un(&b, &f.as_of)?, &i.currency, target, &f.as_of)?;
            let mut piece = match (au, bu) {
                (Some(a), Some(b)) => Some(sub(b, a)?),
                _ => None,
            };
            let mut previous_basis = D::ZERO;
            for row in &b.rows {
                let t = &row.transaction;
                let adjust = if t.action == Action::Basis {
                    mul(pos.direction.sign(), sub(row.basis_after, previous_basis)?)?
                } else {
                    D::ZERO
                };
                previous_basis = row.basis_after;
                if t.date <= *start {
                    continue;
                }
                piece = sum_opt(
                    piece,
                    converted(
                        p,
                        Some(add(row.realized, adjust)?),
                        &i.currency,
                        target,
                        &t.date,
                    )?,
                )?;
            }
            position_pl = sum_opt(position_pl, piece)?;
        }
        for m in &p.cash_movements {
            if m.date <= *start
                || m.date > f.as_of
                || !crate::reporting::scope(&m.account_id, &m.cycle_id, f)
                || !currencies.contains(&m.currency)
            {
                continue;
            }
            match m.kind {
                CashKind::Dividend | CashKind::Interest => {
                    income = sum_opt(
                        income,
                        converted(p, Some(m.amount), &m.currency, target, &m.date)?,
                    )?
                }
                CashKind::Expense => {
                    expenses = sum_opt(
                        expenses,
                        converted(p, Some(m.amount), &m.currency, target, &m.date)?,
                    )?
                }
                _ => {}
            }
        }
        for ccy in &currencies {
            let start_value = native_value(p, f, h, ccy, start)?;
            let end_value = native_value(p, f, h, ccy, &f.as_of)?;
            let net = capital_flows(p, f, ccy, false)?
                .iter()
                .filter(|x| x.date > *start && x.date <= f.as_of)
                .try_fold(D::ZERO, |s, x| add(s, x.amount))?;
            let gain = match (start_value, end_value) {
                (Some(a), Some(b)) => Some(sub(sub(b, a)?, net)?),
                _ => None,
            };
            let points = vec![
                HistoryPoint {
                    date: start.clone(),
                    currency: ccy.clone(),
                    equity: start_value,
                    capital: start_value,
                    gain: Some(D::ZERO),
                    source: "Bridge boundary".into(),
                },
                HistoryPoint {
                    date: f.as_of.clone(),
                    currency: ccy.clone(),
                    equity: end_value,
                    capital: Some(D::ZERO),
                    gain,
                    source: "Bridge boundary".into(),
                },
            ];
            let native = attribution(p, f, ccy, &points)?;
            reconciliation = sum_opt(
                reconciliation,
                converted(p, native.reconciliation, ccy, target, &f.as_of)?,
            )?;
        }
    } else {
        position_pl = None;
        income = None;
        expenses = None;
        reconciliation = None;
    }
    let currency_effect = match (
        performance.investment_gain,
        position_pl,
        income,
        expenses,
        reconciliation,
    ) {
        (Some(g), Some(pl), Some(i), Some(e), Some(r)) => {
            Some(sub(sub(add(sub(g, pl)?, e)?, i)?, r)?)
        }
        _ => None,
    };
    Ok(Consolidated {
        currency: target.into(),
        equity: consolidated_equity(p, f, target)?,
        history,
        performance,
        position_pl,
        income,
        expenses,
        reconciliation,
        currency_effect,
        warnings,
    })
}
