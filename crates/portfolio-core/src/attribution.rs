use crate::{decimal::*, *};
use rust_decimal::Decimal as D;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribution {
    pub currency: String,
    pub from: Option<String>,
    pub to: String,
    pub investment_gain: Option<D>,
    pub period_position_pl: Option<D>,
    pub income: D,
    pub expenses: D,
    pub reconciliation: Option<D>,
    pub note: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    pub position_id: String,
    pub symbol: String,
    pub currency: String,
    pub signed_value: Option<D>,
    pub gross_share: Option<D>,
}
pub fn attribution(
    p: &Portfolio,
    f: &ReportFilter,
    ccy: &str,
    h: &[HistoryPoint],
) -> Result<Attribution> {
    let start = h.iter().find(|h| h.equity.is_some());
    let end = h.last();
    let mut pl = Some(D::ZERO);
    let mut income = D::ZERO;
    let mut expenses = D::ZERO;
    if let Some(start) = start {
        for pos in &p.positions {
            let i = p.instrument(&pos.instrument_id)?;
            if i.currency != ccy || !f.matches(pos, ccy) {
                continue;
            }
            let a = position_ledger(p, &pos.id, Some(&start.date))?;
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
            let piece = match (un(&a, &start.date)?, un(&b, &f.as_of)?) {
                (Some(aun), Some(bun)) => Some(add(
                    sub(add(sub(b.realized, a.realized)?, bun)?, aun)?,
                    sub(b.basis_adjustments, a.basis_adjustments)?,
                )?),
                _ => None,
            };
            pl = match (pl, piece) {
                (Some(a), Some(b)) => Some(add(a, b)?),
                _ => None,
            };
        }
        for m in &p.cash_movements {
            if m.date <= start.date
                || m.date > f.as_of
                || m.currency != ccy
                || !crate::reporting::scope(&m.account_id, &m.cycle_id, f)
            {
                continue;
            }
            match m.kind {
                CashKind::Dividend | CashKind::Interest => income = add(income, m.amount)?,
                CashKind::Expense => expenses = add(expenses, m.amount)?,
                _ => {}
            }
        }
    }
    let gain = end.and_then(|h| h.gain);
    let reconciliation = match (gain, pl) {
        (Some(g), Some(pl)) => Some(sub(g, sub(add(pl, income)?, expenses)?)?),
        _ => None,
    };
    Ok(Attribution{currency:ccy.into(),from:start.map(|h|h.date.clone()),to:f.as_of.clone(),investment_gain:gain,period_position_pl:if start.is_some(){pl}else{None},income,expenses,reconciliation,note:"Native currency bridge. Residuals identify baseline/basis/cash reconciliation differences; they are not labelled FX profit. Currency conversions are capital movements within each native-currency view.".into()})
}
pub fn allocation(positions: &[PositionView]) -> Result<Vec<Allocation>> {
    positions
        .iter()
        .filter(|v| v.quantity > D::ZERO)
        .map(|v| {
            let same: Vec<_> = positions
                .iter()
                .filter(|x| x.instrument.currency == v.instrument.currency && x.quantity > D::ZERO)
                .collect();
            let gross = if same.iter().any(|x| x.market_value.is_none()) {
                None
            } else {
                Some(
                    same.into_iter()
                        .try_fold(D::ZERO, |s, x| add(s, x.market_value.unwrap().abs()))?,
                )
            };
            Ok(Allocation {
                position_id: v.position.id.clone(),
                symbol: v.instrument.symbol.clone(),
                currency: v.instrument.currency.clone(),
                signed_value: v.market_value,
                gross_share: match (v.market_value, gross) {
                    (Some(m), Some(g)) if g > D::ZERO => Some(div(m.abs(), g)?),
                    _ => None,
                },
            })
        })
        .collect()
}
