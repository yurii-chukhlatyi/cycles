use crate::{decimal::*, *};
use chrono::{Datelike, Duration, Weekday};
use rust_decimal::Decimal as D;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalFlow {
    pub id: String,
    pub date: String,
    pub amount: D,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub date: String,
    pub currency: String,
    pub equity: Option<D>,
    pub capital: Option<D>,
    pub gain: Option<D>,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceView {
    pub currency: String,
    pub from: Option<String>,
    pub to: String,
    pub investment_gain: Option<D>,
    pub twr: Option<f64>,
    pub twr_method: String,
    pub xirr: Option<f64>,
    pub xirr_status: String,
    pub volatility: Option<f64>,
    pub observations: usize,
    pub missing_periods: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetVolatility {
    pub instrument_id: String,
    pub volatility: Option<f64>,
    pub observations: usize,
    pub method: String,
}
pub fn capital_flows(
    p: &Portfolio,
    f: &ReportFilter,
    ccy: &str,
    external_only: bool,
) -> Result<Vec<CapitalFlow>> {
    let mut flows = vec![];
    for m in &p.cash_movements {
        if m.date > f.as_of {
            continue;
        }
        let source = scope(&m.account_id, &m.cycle_id, f) && m.currency == ccy;
        let dst = scope(
            m.destination_account_id.as_deref().unwrap_or(&m.account_id),
            m.destination_cycle_id.as_deref().unwrap_or(&m.cycle_id),
            f,
        ) && m.receive_currency.as_deref().unwrap_or(&m.currency) == ccy;
        let amount = match m.kind {
            CashKind::Initial | CashKind::Deposit if source => m.amount,
            CashKind::Withdrawal if source => -m.amount,
            CashKind::CycleTransfer | CashKind::AccountTransfer | CashKind::Fx
                if !external_only =>
            {
                sub(
                    if dst {
                        m.receive_amount.unwrap_or(m.amount)
                    } else {
                        D::ZERO
                    },
                    if source { m.amount } else { D::ZERO },
                )?
            }
            _ => D::ZERO,
        };
        if amount != D::ZERO {
            flows.push(CapitalFlow {
                id: m.id.clone(),
                date: m.date.clone(),
                amount,
            })
        }
    }
    flows.sort_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)));
    Ok(flows)
}
pub fn history(p: &Portfolio, f: &ReportFilter, ccy: &str) -> Result<Vec<HistoryPoint>> {
    let flows = capital_flows(p, f, ccy, false)?;
    let mut out = vec![];
    let mut first: Option<(String, D)> = None;
    let baseline = p
        .cash_movements
        .iter()
        .filter(|m| {
            m.kind == CashKind::Baseline
                && m.currency == ccy
                && scope(&m.account_id, &m.cycle_id, f)
        })
        .map(|m| m.date.as_str())
        .chain(
            p.snapshots
                .iter()
                .filter(|s| {
                    s.phase == "baseline"
                        && s.currency == ccy
                        && s.cycle_id == f.cycle_id
                        && s.account_id == f.account_id
                })
                .map(|s| s.date.as_str()),
        )
        .max();
    let mut dates = market_dates(p);
    dates.insert(f.as_of.clone());
    for d in dates {
        if d > f.as_of
            || baseline.is_some_and(|x| d.as_str() < x)
            || f.from.as_ref().is_some_and(|x| d < *x)
        {
            continue;
        }
        let snap = p
            .snapshots
            .iter()
            .filter(|s| {
                s.date == d
                    && s.currency == ccy
                    && s.cycle_id == f.cycle_id
                    && s.account_id == f.account_id
                    && matches!(s.phase.as_str(), "close" | "baseline" | "stop")
            })
            .max_by_key(|s| &s.id);
        let value = match snap {
            Some(s) => Some(s.equity),
            None => equity(p, f, ccy, &d, true)?,
        };
        let any = p.transactions.iter().any(|t| {
            t.date <= d
                && p.position(&t.position_id).ok().is_some_and(|pos| {
                    scope(&pos.account_id, &pos.cycle_id, f)
                        && p.instrument(&pos.instrument_id)
                            .ok()
                            .is_some_and(|i| i.currency == ccy)
                })
        }) || flows.iter().any(|x| x.date <= d)
            || snap.is_some();
        if !any {
            continue;
        }
        if first.is_none() {
            if let Some(v) = value {
                first = Some((d.clone(), v));
            }
        }
        let (capital, gain) = if let Some((start, initial)) = &first {
            let net = flows
                .iter()
                .filter(|x| x.date > *start && x.date <= d)
                .try_fold(D::ZERO, |s, x| add(s, x.amount))?;
            let cap = add(*initial, net)?;
            (cap, value.map(|v| sub(v, cap)).transpose()?)
        } else {
            (D::ZERO, None)
        };
        out.push(HistoryPoint {
            date: d,
            currency: ccy.into(),
            equity: value,
            capital: Some(capital),
            gain,
            source: if snap.is_some() {
                "Recorded valuation"
            } else {
                "Dated market inputs"
            }
            .into(),
        });
    }
    Ok(out)
}
fn business(p: &Portfolio, d: chrono::NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
        && !p.settings.holidays.contains(&d.to_string())
}
fn adjacent(p: &Portfolio, a: &str, b: &str) -> bool {
    let Ok(mut next) = date(a).map(|x| x + Duration::days(1)) else {
        return false;
    };
    for _ in 0..14 {
        if business(p, next) {
            return next.to_string() == b;
        }
        next += Duration::days(1);
    }
    false
}
fn volatility(xs: &[f64]) -> Option<f64> {
    if xs.len() < 20 {
        return None;
    }
    let avg = xs.iter().sum::<f64>() / xs.len() as f64;
    let var = xs.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    let v = (var * 252.0).sqrt();
    v.is_finite().then_some(v)
}
pub fn performance_view(
    p: &Portfolio,
    f: &ReportFilter,
    ccy: &str,
    h: &[HistoryPoint],
) -> Result<PerformanceView> {
    let flows = capital_flows(p, f, ccy, false)?;
    performance_with_flows(p, f, ccy, h, &flows, true)
}
pub(crate) fn performance_with_flows(
    p: &Portfolio,
    f: &ReportFilter,
    ccy: &str,
    h: &[HistoryPoint],
    flows: &[CapitalFlow],
    flow_snapshots: bool,
) -> Result<PerformanceView> {
    let first = h.iter().find(|x| x.equity.is_some());
    let last = h.last();
    let mut out = PerformanceView {
        currency: ccy.into(),
        from: first.map(|x| x.date.clone()),
        to: f.as_of.clone(),
        investment_gain: last.and_then(|x| x.gain),
        twr: None,
        twr_method: "Insufficient valuation history".into(),
        xirr: None,
        xirr_status: "Insufficient cash-flow history".into(),
        volatility: None,
        observations: 0,
        missing_periods: 0,
    };
    let Some(start) = first else { return Ok(out) };
    let Some(end) = last.filter(|x| x.equity.is_some()) else {
        return Ok(out);
    };
    if end.date <= start.date {
        return Ok(out);
    }
    let sv = start.equity.unwrap();
    let ev = end.equity.unwrap();
    let between: Vec<_> = flows
        .iter()
        .filter(|x| x.date > start.date && x.date <= end.date)
        .collect();
    if sv > D::ZERO {
        if between.is_empty() {
            out.twr = Some(num(div(ev, sv)?) - 1.0);
            out.twr_method = "Exact · no external flows".into();
        } else {
            let mut growth = 1.0;
            let mut anchor = sv;
            let mut all = true;
            for flow in &between {
                let pre = p.snapshots.iter().find(|s| {
                    s.date == flow.date
                        && s.phase == "pre_flow"
                        && s.source == flow.id
                        && s.currency == ccy
                        && s.cycle_id == f.cycle_id
                        && s.account_id == f.account_id
                });
                let post = p.snapshots.iter().find(|s| {
                    s.date == flow.date
                        && s.phase == "post_flow"
                        && s.source == flow.id
                        && s.currency == ccy
                        && s.cycle_id == f.cycle_id
                        && s.account_id == f.account_id
                });
                match (pre, post) {
                    (Some(a), Some(b))
                        if anchor > D::ZERO && add(a.equity, flow.amount)? == b.equity =>
                    {
                        growth *= num(div(a.equity, anchor)?);
                        anchor = b.equity;
                    }
                    _ => {
                        all = false;
                        break;
                    }
                }
            }
            if all && flow_snapshots && anchor > D::ZERO {
                out.twr = Some(growth * num(div(ev, anchor)?) - 1.0);
                out.twr_method = "Exact · flow-time valuations".into();
            }
        }
    }
    let mut rs = vec![];
    let mut daily_growth = 1.0;
    let mut full = true;
    for pair in h.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if !adjacent(p, &a.date, &b.date) {
            out.missing_periods += 1;
            full = false;
            continue;
        }
        match (a.equity, b.equity) {
            (Some(ae), Some(be)) if ae > D::ZERO => {
                let net = between
                    .iter()
                    .filter(|x| x.date > a.date && x.date <= b.date)
                    .try_fold(D::ZERO, |s, x| add(s, x.amount))?;
                let r = num(div(sub(be, net)?, ae)?) - 1.0;
                if r.is_finite() {
                    rs.push(r);
                    daily_growth *= 1.0 + r;
                } else {
                    full = false;
                }
            }
            _ => {
                out.missing_periods += 1;
                full = false;
            }
        }
    }
    if out.twr.is_none() && full && !rs.is_empty() {
        out.twr = Some(daily_growth - 1.0);
        out.twr_method = "Estimated · end-of-day cash flows".into();
    }
    let window = p.settings.volatility_window.clamp(20, 252);
    let tail = &rs[rs.len().saturating_sub(window)..];
    out.observations = tail.len();
    out.volatility = volatility(tail);
    let mut cash = vec![(date(&start.date)?, -num(sv))];
    for flow in between {
        cash.push((date(&flow.date)?, -num(flow.amount)))
    }
    cash.push((date(&end.date)?, num(ev)));
    let (irr, status) = xirr(&cash);
    out.xirr = irr;
    out.xirr_status = status;
    Ok(out)
}
pub fn xirr(flows: &[(chrono::NaiveDate, f64)]) -> (Option<f64>, String) {
    if flows.len() < 2 || !flows.iter().any(|x| x.1 < 0.0) || !flows.iter().any(|x| x.1 > 0.0) {
        return (
            None,
            "Requires both invested capital and proceeds/value".into(),
        );
    }
    let mut grouped = std::collections::BTreeMap::<chrono::NaiveDate, f64>::new();
    for (d, v) in flows {
        if !v.is_finite() {
            return (None, "Invalid non-finite cash flow".into());
        }
        *grouped.entry(*d).or_default() += v;
    }
    let signs: Vec<_> = grouped
        .values()
        .filter(|x| **x != 0.0)
        .map(|x| x.signum())
        .collect();
    if signs.windows(2).filter(|x| x[0] != x[1]).count() > 1 {
        return (
            None,
            "Non-conventional flows: XIRR uniqueness is unresolved (-100% to 100,000,000% search)"
                .into(),
        );
    }
    let start = flows.iter().map(|x| x.0).min().unwrap();
    if flows.iter().all(|x| x.0 == start) {
        return (None, "Requires more than one valuation date".into());
    }
    let scale = flows.iter().map(|x| x.1.abs()).fold(1.0, f64::max);
    let npv = |y: f64| {
        flows
            .iter()
            .map(|(d, v)| v / scale * (-y * ((*d - start).num_days() as f64 / 365.0)).exp())
            .sum::<f64>()
    };
    let lo = -20.0;
    let hi = (1_000_001.0f64).ln();
    let mut roots = vec![];
    let mut a = lo;
    let mut fa = npv(a);
    for k in 1..=4096 {
        let b = lo + (hi - lo) * k as f64 / 4096.0;
        let fb = npv(b);
        if fa.is_finite() && fb.is_finite() && fa.signum() != fb.signum() {
            let (mut l, mut r) = (a, b);
            let mut fl = fa;
            for _ in 0..100 {
                let m = (l + r) / 2.0;
                let fm = npv(m);
                if fm.abs() < 1e-12 {
                    l = m;
                    r = m;
                    break;
                }
                if fm.signum() == fl.signum() {
                    l = m;
                    fl = fm;
                } else {
                    r = m;
                }
            }
            let root = ((l + r) / 2.0).exp() - 1.0;
            if roots.iter().all(|x: &f64| (x - root).abs() > 1e-6) {
                roots.push(root)
            }
        }
        a = b;
        fa = fb;
    }
    match roots.len() {
        1 => (Some(roots[0]), "Annualized XIRR · Actual/365".into()),
        0 => (
            None,
            "No supported XIRR root (-100% to 100,000,000% search)".into(),
        ),
        _ => (None, "Multiple XIRR roots; return is ambiguous".into()),
    }
}
pub fn asset_volatility(p: &Portfolio, id: &str, as_of: &str) -> Result<AssetVolatility> {
    let mut q: Vec<_> = p
        .market_data
        .iter()
        .filter(|q| q.instrument_id == id && q.date.as_str() <= as_of)
        .collect();
    let adjusted = q.iter().any(|x| x.adjusted);
    q.retain(|x| x.adjusted == adjusted);
    q.sort_by(|a, b| (&b.date, &b.id).cmp(&(&a.date, &a.id)));
    q.dedup_by(|a, b| a.date == b.date);
    q.reverse();
    let mut rs = vec![];
    for pair in q.windows(2) {
        if adjacent(p, &pair[0].date, &pair[1].date) && pair[0].price > D::ZERO {
            rs.push(num(div(pair[1].price, pair[0].price)?) - 1.0)
        }
    }
    let window = p.settings.volatility_window.clamp(20, 252);
    let tail = &rs[rs.len().saturating_sub(window)..];
    Ok(AssetVolatility {
        instrument_id: id.into(),
        volatility: volatility(tail),
        observations: tail.len(),
        method: if adjusted {
            "Adjusted-price return volatility"
        } else {
            "Price-return volatility · unadjusted"
        }
        .into(),
    })
}
