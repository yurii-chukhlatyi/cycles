use crate::{decimal::*, *};
use chrono::Datelike;
use rust_decimal::{prelude::ToPrimitive, Decimal as D};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionView {
    pub position: Position,
    pub instrument: Instrument,
    pub cycle_name: String,
    pub account_name: String,
    pub quantity: D,
    pub opened: D,
    pub closed: D,
    pub basis: D,
    pub average_entry: Option<D>,
    pub realized: D,
    pub unrealized: Option<D>,
    pub total_pl: Option<D>,
    pub period_pl: Option<D>,
    pub price: Option<D>,
    pub quote_date: Option<String>,
    pub market_value: Option<D>,
    pub planned_risk: Option<D>,
    pub delta_exposure: Option<D>,
    pub maximum_loss: Option<D>,
    pub unbounded: bool,
    pub status: String,
    pub stale: bool,
    pub ledger: Vec<LedgerRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashBalance {
    pub account_id: String,
    pub cycle_id: String,
    pub currency: String,
    pub balance: D,
    pub reserved: D,
    pub available: Option<D>,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyTotals {
    pub currency: String,
    pub cash: D,
    pub market_value: Option<D>,
    pub equity: Option<D>,
    pub realized: D,
    pub unrealized: Option<D>,
    pub total_pl: Option<D>,
    pub period_pl: Option<D>,
    pub contributed: D,
    pub external_contributions: D,
    pub income: D,
    pub expenses: D,
    pub gross_exposure: Option<D>,
    pub net_exposure: Option<D>,
    pub planned_risk: Option<D>,
    pub delta_exposure: Option<D>,
    pub missing_quotes: usize,
    pub missing_stops: usize,
    pub missing_deltas: usize,
    pub open_positions: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub positions: Vec<PositionView>,
    pub consolidated: Option<Consolidated>,
    pub allocation: Vec<Allocation>,
    pub attribution: Vec<Attribution>,
    pub cash_balances: Vec<CashBalance>,
    pub totals: Vec<CurrencyTotals>,
    pub history: Vec<HistoryPoint>,
    pub performance: Vec<PerformanceView>,
    pub asset_volatility: Vec<AssetVolatility>,
    pub warnings: Vec<String>,
    pub as_of: String,
    pub from: Option<String>,
}
pub fn quote<'a>(p: &'a Portfolio, id: &str, as_of: &str) -> Option<&'a MarketData> {
    p.market_data
        .iter()
        .filter(|q| q.instrument_id == id && q.date.as_str() <= as_of)
        .max_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)))
}
pub fn fx(p: &Portfolio, from: &str, to: &str, as_of: &str) -> Option<D> {
    if from == to {
        return Some(D::ONE);
    }
    let direct = p
        .fx_rates
        .iter()
        .filter(|f| f.from == from && f.to == to && f.date.as_str() <= as_of)
        .max_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)));
    if let Some(f) = direct {
        return Some(f.rate);
    }
    p.fx_rates
        .iter()
        .filter(|f| f.from == to && f.to == from && f.date.as_str() <= as_of)
        .max_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)))
        .and_then(|f| D::ONE.checked_div(f.rate))
}
pub fn fx_evidence_date(p: &Portfolio, from: &str, to: &str, as_of: &str) -> Option<String> {
    if from == to {
        return Some(as_of.into());
    }
    let find = |a: &str, b: &str| {
        p.fx_rates
            .iter()
            .filter(|f| f.from == a && f.to == b && f.date.as_str() <= as_of)
            .max_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)))
    };
    find(from, to)
        .or_else(|| find(to, from))
        .map(|r| r.date.clone())
}
pub fn report(p: &Portfolio, f: &ReportFilter) -> Result<Report> {
    validate(p)?;
    date(&f.as_of)?;
    if let Some(from) = &f.from {
        date(from)?;
        if from > &f.as_of {
            return Err(Error::Invalid("Period start follows its end".into()));
        }
    }
    let mut views = vec![];
    let mut warnings = vec![];
    for pos in &p.positions {
        let i = p.instrument(&pos.instrument_id)?;
        if !f.matches(pos, &i.currency) {
            continue;
        }
        let l = position_ledger(p, &pos.id, Some(&f.as_of))?;
        if l.rows.is_empty() {
            continue;
        }
        let q = quote(p, &i.id, &f.as_of);
        let gross = q
            .map(|x| mul(mul(l.quantity, i.multiplier)?, x.price))
            .transpose()?;
        let mv = if l.quantity == D::ZERO {
            Some(D::ZERO)
        } else {
            gross.map(|x| mul(pos.direction.sign(), x)).transpose()?
        };
        let un = if l.quantity == D::ZERO {
            Some(D::ZERO)
        } else {
            gross
                .map(|g| mul(pos.direction.sign(), sub(g, l.basis)?))
                .transpose()?
        };
        let period_pl = if let Some(from) = &f.from {
            let prior = position_ledger(p, &pos.id, Some(from))?;
            let pq = quote(p, &i.id, from).filter(|q| q.date == *from);
            let prior_un = if prior.quantity == D::ZERO {
                Some(D::ZERO)
            } else {
                pq.map(|q| {
                    mul(
                        pos.direction.sign(),
                        sub(
                            mul(mul(prior.quantity, i.multiplier)?, q.price)?,
                            prior.basis,
                        )?,
                    )
                })
                .transpose()?
            };
            match (un, prior_un) {
                (Some(end), Some(start)) => Some(add(
                    sub(add(sub(l.realized, prior.realized)?, end)?, start)?,
                    sub(l.basis_adjustments, prior.basis_adjustments)?,
                )?),
                _ => None,
            }
        } else {
            None
        };
        let sign = pos.direction.sign();
        let planned = match (q, pos.stop_price) {
            (Some(q), Some(stop))
                if l.quantity > D::ZERO
                    && ((sign > D::ZERO && stop <= q.price)
                        || (sign < D::ZERO && stop >= q.price)) =>
            {
                Some(mul(
                    mul(l.quantity, i.multiplier)?,
                    mul(sign, sub(q.price, stop)?)?,
                )?)
            }
            _ => None,
        };
        let delta = if l.quantity == D::ZERO {
            Some(D::ZERO)
        } else if i.asset == Asset::Stock {
            mv
        } else {
            q.and_then(|q| q.delta.zip(q.underlying_price))
                .map(|(d, u)| mul(mul(mul(mul(sign, l.quantity)?, i.multiplier)?, d)?, u))
                .transpose()?
        };
        let unbounded = l.quantity > D::ZERO
            && pos.direction == Direction::Short
            && (i.asset == Asset::Stock || i.option_right == Some(OptionRight::Call))
            && pos.covered_by.is_none();
        let maximum_loss = if l.quantity == D::ZERO {
            Some(D::ZERO)
        } else if pos.direction == Direction::Long {
            Some(l.basis.max(D::ZERO))
        } else if i.option_right == Some(OptionRight::Put) {
            Some(
                sub(
                    mul(mul(l.quantity, i.multiplier)?, i.strike.unwrap_or_default())?,
                    l.basis,
                )?
                .max(D::ZERO),
            )
        } else {
            None
        };
        let mut previous = date(&f.as_of)?;
        while matches!(
            previous.weekday(),
            chrono::Weekday::Sat | chrono::Weekday::Sun
        ) || p.settings.holidays.contains(&previous.to_string())
        {
            previous = previous
                .pred_opt()
                .ok_or_else(|| Error::Invalid("Date outside calendar".into()))?;
        }
        let stale = l.quantity > D::ZERO && q.is_some_and(|q| q.date < previous.to_string());
        views.push(PositionView {
            position: pos.clone(),
            instrument: i.clone(),
            cycle_name: p.cycle(&pos.cycle_id)?.name.clone(),
            account_name: p.account(&pos.account_id)?.name.clone(),
            quantity: l.quantity,
            opened: l.opened,
            closed: l.closed,
            basis: l.basis,
            average_entry: if l.quantity > D::ZERO {
                Some(div(l.basis, mul(l.quantity, i.multiplier)?)?)
            } else {
                None
            },
            realized: l.realized,
            unrealized: un,
            total_pl: un.map(|x| add(x, l.realized)).transpose()?,
            period_pl,
            price: q.map(|x| x.price),
            quote_date: q.map(|x| x.date.clone()),
            market_value: mv,
            planned_risk: planned,
            delta_exposure: delta,
            maximum_loss,
            unbounded,
            status: if l.quantity == D::ZERO {
                "Closed"
            } else if l.closed > D::ZERO {
                "Partially closed"
            } else {
                "Open"
            }
            .into(),
            stale,
            ledger: l.rows,
        });
    }
    let mut balances = vec![];
    let mut currencies = BTreeSet::new();
    for a in &p.accounts {
        if f.account_id.as_ref().is_some_and(|x| x != &a.id) {
            continue;
        }
        for c in &p.cycles {
            if f.cycle_id.as_ref().is_some_and(|x| x != &c.id) || c.start_date > f.as_of {
                continue;
            }
            for ccy in &a.currencies {
                if f.currency.as_ref().is_some_and(|x| x != ccy) {
                    continue;
                }
                currencies.insert(ccy.clone());
                let b = cash_balance(p, &a.id, &c.id, ccy, &f.as_of)?;
                let r = reserves(p, &a.id, &c.id, ccy, &f.as_of)?;
                balances.push(CashBalance {
                    account_id: a.id.clone(),
                    cycle_id: c.id.clone(),
                    currency: ccy.clone(),
                    balance: b,
                    reserved: r.amount,
                    available: if r.complete {
                        Some(sub(b, r.amount)?.max(D::ZERO))
                    } else {
                        None
                    },
                    reasons: r.reasons,
                });
            }
        }
    }
    let mut totals = vec![];
    let mut all_history = vec![];
    let mut performance = vec![];
    let mut attribution_views = vec![];
    for ccy in currencies {
        let vs: Vec<_> = views
            .iter()
            .filter(|v| v.instrument.currency == ccy)
            .collect();
        let sum = |values: Vec<D>| values.into_iter().try_fold(D::ZERO, add);
        let optsum = |values: Vec<Option<D>>| -> Result<Option<D>> {
            if values.iter().any(Option::is_none) {
                Ok(None)
            } else {
                Ok(Some(sum(values.into_iter().flatten().collect())?))
            }
        };
        let cash = sum(balances
            .iter()
            .filter(|b| b.currency == ccy)
            .map(|b| b.balance)
            .collect())?;
        let mv = optsum(vs.iter().map(|v| v.market_value).collect())?;
        let realized = sum(vs.iter().map(|v| v.realized).collect())?;
        let un = optsum(vs.iter().map(|v| v.unrealized).collect())?;
        let period = if f.from.is_some() {
            optsum(vs.iter().map(|v| v.period_pl).collect())?
        } else {
            None
        };
        let mut income = D::ZERO;
        let mut expenses = D::ZERO;
        for m in &p.cash_movements {
            if m.currency != ccy
                || m.date > f.as_of
                || f.from.as_ref().is_some_and(|d| m.date <= *d)
                || !scope(&m.account_id, &m.cycle_id, f)
            {
                continue;
            }
            match m.kind {
                CashKind::Dividend | CashKind::Interest => income = add(income, m.amount)?,
                CashKind::Expense => expenses = add(expenses, m.amount)?,
                _ => {}
            }
        }
        let movements = capital_flows(p, f, &ccy, false)?;
        let external = capital_flows(p, f, &ccy, true)?;
        totals.push(CurrencyTotals {
            currency: ccy.clone(),
            cash,
            market_value: mv,
            equity: mv.map(|x| add(x, cash)).transpose()?,
            realized,
            unrealized: un,
            total_pl: un.map(|x| add(x, realized)).transpose()?,
            period_pl: period,
            contributed: sum(movements.iter().map(|x| x.amount).collect())?,
            external_contributions: sum(external.iter().map(|x| x.amount).collect())?,
            income,
            expenses,
            gross_exposure: optsum(vs.iter().map(|v| v.market_value.map(|x| x.abs())).collect())?,
            net_exposure: mv,
            planned_risk: optsum(
                vs.iter()
                    .filter(|v| v.quantity > D::ZERO)
                    .map(|v| v.planned_risk)
                    .collect(),
            )?,
            delta_exposure: optsum(vs.iter().map(|v| v.delta_exposure).collect())?,
            missing_quotes: vs.iter().filter(|v| v.market_value.is_none()).count(),
            missing_stops: vs
                .iter()
                .filter(|v| v.quantity > D::ZERO && v.planned_risk.is_none())
                .count(),
            missing_deltas: vs
                .iter()
                .filter(|v| v.quantity > D::ZERO && v.delta_exposure.is_none())
                .count(),
            open_positions: vs.iter().filter(|v| v.quantity > D::ZERO).count(),
        });
        let h = history(p, f, &ccy)?;
        performance.push(performance_view(p, f, &ccy, &h)?);
        attribution_views.push(attribution(p, f, &ccy, &h)?);
        all_history.extend(h);
    }
    if views.iter().any(|v| v.stale) {
        warnings.push(
            "Some current quotes are stale. Historical returns require dated valuation evidence."
                .into(),
        )
    }
    if p.positions.iter().any(|x| !x.source.is_empty()) {
        warnings.push("Imported P/L covers retained known-basis history; it is not necessarily lifetime account performance.".into())
    }
    let asset_volatility = p
        .instruments
        .iter()
        .filter(|i| views.iter().any(|v| v.instrument.id == i.id))
        .map(|i| asset_volatility(p, &i.id, &f.as_of))
        .collect::<Result<Vec<_>>>()?;
    Ok(Report {
        consolidated: p
            .settings
            .reporting_currency
            .as_ref()
            .map(|c| consolidated(p, f, &all_history, c))
            .transpose()?,
        allocation: allocation(&views)?,
        attribution: attribution_views,
        positions: views,
        cash_balances: balances,
        totals,
        history: all_history,
        performance,
        asset_volatility,
        warnings,
        as_of: f.as_of.clone(),
        from: f.from.clone(),
    })
}
pub(crate) fn scope(a: &str, c: &str, f: &ReportFilter) -> bool {
    f.account_id.as_deref().is_none_or(|x| x == a) && f.cycle_id.as_deref().is_none_or(|x| x == c)
}
pub fn equity(
    p: &Portfolio,
    f: &ReportFilter,
    ccy: &str,
    d: &str,
    strict: bool,
) -> Result<Option<D>> {
    let mut v = D::ZERO;
    for a in &p.accounts {
        for c in &p.cycles {
            if !scope(&a.id, &c.id, f) || c.start_date.as_str() > d {
                continue;
            }
            if a.currencies.contains(&ccy.to_string()) {
                v = add(v, cash_balance(p, &a.id, &c.id, ccy, d)?)?;
            }
        }
    }
    for pos in &p.positions {
        let i = p.instrument(&pos.instrument_id)?;
        if i.currency != ccy || !f.matches(pos, ccy) {
            continue;
        }
        let l = position_ledger(p, &pos.id, Some(d))?;
        if l.quantity == D::ZERO {
            continue;
        }
        let Some(q) = quote(p, &i.id, d).filter(|q| !strict || q.date == d) else {
            return Ok(None);
        };
        v = add(
            v,
            mul(
                mul(mul(pos.direction.sign(), l.quantity)?, i.multiplier)?,
                q.price,
            )?,
        )?;
    }
    Ok(Some(v))
}
pub fn consolidated_equity(p: &Portfolio, f: &ReportFilter, target: &str) -> Result<Option<D>> {
    let mut sum = D::ZERO;
    let currencies: BTreeSet<_> = p
        .accounts
        .iter()
        .flat_map(|a| a.currencies.iter())
        .filter(|c| f.currency.as_ref().is_none_or(|x| x == *c))
        .collect();
    for c in currencies {
        let Some(v) = equity(p, f, c, &f.as_of, false)? else {
            return Ok(None);
        };
        if v == D::ZERO {
            continue;
        }
        let Some(rate) = fx(p, c, target, &f.as_of) else {
            return Ok(None);
        };
        sum = add(sum, mul(v, rate)?)?;
    }
    Ok(Some(sum))
}
pub fn market_dates(p: &Portfolio) -> BTreeSet<String> {
    p.market_data
        .iter()
        .map(|q| q.date.clone())
        .chain(p.cash_movements.iter().map(|x| x.date.clone()))
        .chain(p.snapshots.iter().map(|s| s.date.clone()))
        .chain(p.fx_rates.iter().map(|s| s.date.clone()))
        .chain(p.transactions.iter().map(|x| x.date.clone()))
        .collect()
}
pub fn num(x: D) -> f64 {
    x.to_f64().unwrap_or(f64::NAN)
}
