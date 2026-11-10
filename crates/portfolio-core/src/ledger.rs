use crate::{decimal::*, *};
use rust_decimal::Decimal as D;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub quantity: D,
    pub basis: D,
    pub realized: D,
    pub cash: D,
    pub opened: D,
    pub closed: D,
    pub basis_adjustments: D,
    pub rows: Vec<LedgerRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerRow {
    pub transaction: Transaction,
    pub quantity_after: D,
    pub basis_after: D,
    pub realized: D,
    pub cash_delta: D,
}
pub fn position_ledger(p: &Portfolio, id: &str, as_of: Option<&str>) -> Result<Ledger> {
    let pos = p.position(id)?;
    let ins = p.instrument(&pos.instrument_id)?;
    let s = pos.direction.sign();
    let mut out = Ledger::default();
    let mut ts: Vec<_> = p
        .transactions
        .iter()
        .filter(|t| t.position_id == id && as_of.is_none_or(|d| t.date.as_str() <= d))
        .collect();
    ts.sort_by(|a, b| (&a.date, a.order, &a.id).cmp(&(&b.date, b.order, &b.id)));
    for t in ts {
        let gross = mul(mul(t.quantity, ins.multiplier)?, t.price)?;
        let mut realized = D::ZERO;
        let mut cash = D::ZERO;
        match t.action {
            Action::Open | Action::Snapshot => {
                out.quantity = add(out.quantity, t.quantity)?;
                out.opened = add(out.opened, t.quantity)?;
                out.basis = add(
                    out.basis,
                    if t.action == Action::Snapshot {
                        t.basis_total.unwrap_or(add(gross, mul(s, t.fee)?)?)
                    } else {
                        add(gross, mul(s, t.fee)?)?
                    },
                )?;
                if t.action == Action::Open {
                    cash = sub(-mul(s, gross)?, t.fee)?;
                }
            }
            Action::Basis => {
                if out.quantity <= D::ZERO {
                    return Err(Error::Invalid(format!(
                        "{}: basis adjustment requires an open position",
                        t.id
                    )));
                }
                let next = t
                    .basis_total
                    .unwrap_or(mul(mul(out.quantity, ins.multiplier)?, t.price)?);
                out.basis_adjustments = add(out.basis_adjustments, mul(s, sub(next, out.basis)?)?)?;
                out.basis = next;
            }
            Action::Split => {
                if ins.asset != Asset::Stock || out.quantity <= D::ZERO || t.quantity <= D::ZERO {
                    return Err(Error::Invalid(
                        "A split requires an open stock and positive ratio".into(),
                    ));
                }
                out.quantity = mul(out.quantity, t.quantity)?;
            }
            _ => {
                if t.quantity > out.quantity || t.quantity <= D::ZERO {
                    return Err(Error::Invalid(format!(
                        "{}: close exceeds remaining quantity",
                        t.id
                    )));
                }
                let allocated = if t.quantity == out.quantity {
                    out.basis
                } else {
                    mul(out.basis, div(t.quantity, out.quantity)?)?
                };
                realized = sub(mul(s, sub(gross, allocated)?)?, t.fee)?;
                cash = sub(mul(s, gross)?, t.fee)?;
                out.quantity = sub(out.quantity, t.quantity)?;
                out.basis = sub(out.basis, allocated)?;
                out.closed = add(out.closed, t.quantity)?;
                out.realized = add(out.realized, realized)?;
            }
        }
        out.cash = add(out.cash, cash)?;
        out.rows.push(LedgerRow {
            transaction: t.clone(),
            quantity_after: out.quantity,
            basis_after: out.basis,
            realized,
            cash_delta: cash,
        });
    }
    Ok(out)
}
pub fn cash_balance(p: &Portfolio, a: &str, c: &str, ccy: &str, as_of: &str) -> Result<D> {
    cash_until(p, a, c, ccy, as_of, None)
}
pub(crate) fn cash_until(
    p: &Portfolio,
    a: &str,
    c: &str,
    ccy: &str,
    as_of: &str,
    before: Option<(&str, i64, &str)>,
) -> Result<D> {
    let eligible = |d: &str, o: i64, id: &str| d <= as_of && before.is_none_or(|b| (d, o, id) < b);
    let anchor = p
        .cash_movements
        .iter()
        .filter(|x| {
            x.account_id == a
                && x.cycle_id == c
                && x.currency == ccy
                && x.kind == CashKind::Baseline
                && eligible(&x.date, x.order, &x.id)
        })
        .max_by(|x, y| (&x.date, x.order, &x.id).cmp(&(&y.date, y.order, &y.id)));
    let after = |d: &str, o: i64, id: &str| {
        anchor.is_none_or(|b| (d, o, id) > (b.date.as_str(), b.order, b.id.as_str()))
    };
    let mut cash = anchor.map_or(D::ZERO, |x| x.amount);
    for pos in p
        .positions
        .iter()
        .filter(|x| x.account_id == a && x.cycle_id == c)
    {
        let ins = p.instrument(&pos.instrument_id)?;
        if ins.currency != ccy {
            continue;
        }
        for row in position_ledger(p, &pos.id, Some(as_of))?.rows {
            let t = &row.transaction;
            if eligible(&t.date, t.order, &t.id) && after(&t.date, t.order, &t.id) {
                cash = add(cash, row.cash_delta)?;
            }
        }
    }
    for x in &p.cash_movements {
        if x.kind == CashKind::Baseline
            || !eligible(&x.date, x.order, &x.id)
            || !after(&x.date, x.order, &x.id)
        {
            continue;
        }
        if x.account_id == a && x.cycle_id == c && x.currency == ccy {
            let sign = match x.kind {
                CashKind::Withdrawal
                | CashKind::Expense
                | CashKind::CycleTransfer
                | CashKind::AccountTransfer
                | CashKind::Fx => -D::ONE,
                _ => D::ONE,
            };
            cash = add(cash, mul(sign, x.amount)?)?;
        }
        let da = x.destination_account_id.as_deref().unwrap_or(&x.account_id);
        let dc = x.destination_cycle_id.as_deref().unwrap_or(&x.cycle_id);
        let dst_ccy = x.receive_currency.as_deref().unwrap_or(&x.currency);
        if matches!(
            x.kind,
            CashKind::CycleTransfer | CashKind::AccountTransfer | CashKind::Fx
        ) && da == a
            && dc == c
            && dst_ccy == ccy
        {
            cash = add(cash, x.receive_amount.unwrap_or(x.amount))?;
        }
    }
    Ok(cash)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reserve {
    pub amount: D,
    pub complete: bool,
    pub reasons: Vec<String>,
}
pub fn reserves(p: &Portfolio, a: &str, c: &str, ccy: &str, as_of: &str) -> Result<Reserve> {
    let mut out = Reserve {
        amount: D::ZERO,
        complete: true,
        reasons: vec![],
    };
    let manual: Vec<_> = p
        .reservations
        .iter()
        .filter(|r| {
            r.account_id == a
                && r.cycle_id == c
                && r.currency == ccy
                && r.start_date.as_str() <= as_of
                && r.end_date.as_deref().is_none_or(|x| x > as_of)
        })
        .collect();
    for r in &manual {
        out.amount = add(out.amount, r.amount)?;
    }
    for pos in p
        .positions
        .iter()
        .filter(|x| x.account_id == a && x.cycle_id == c && x.direction == Direction::Short)
    {
        let ins = p.instrument(&pos.instrument_id)?;
        if ins.currency != ccy {
            continue;
        }
        let l = position_ledger(p, &pos.id, Some(as_of))?;
        if l.quantity == D::ZERO {
            continue;
        }
        if ins.asset == Asset::Option && ins.option_right == Some(OptionRight::Put) {
            out.amount = add(
                out.amount,
                mul(
                    mul(l.quantity, ins.multiplier)?,
                    ins.strike.unwrap_or_default(),
                )?,
            )?;
        } else if ins.asset == Asset::Option
            && ins.option_right == Some(OptionRight::Call)
            && pos.covered_by.is_some()
        {
            let cp = p.position(pos.covered_by.as_deref().unwrap())?;
            let ci = p.instrument(&cp.instrument_id)?;
            let need = mul(l.quantity, ins.multiplier)?;
            let supplied = mul(
                position_ledger(p, &cp.id, Some(as_of))?.quantity,
                ci.multiplier,
            )?;
            let claimed = p
                .positions
                .iter()
                .filter(|x| x.id != pos.id && x.covered_by == pos.covered_by)
                .try_fold(D::ZERO, |sum, x| -> Result<D> {
                    add(
                        sum,
                        mul(
                            position_ledger(p, &x.id, Some(as_of))?.quantity,
                            p.instrument(&x.instrument_id)?.multiplier,
                        )?,
                    )
                })?;
            if supplied < add(need, claimed)? {
                out.complete = false;
                out.reasons
                    .push(format!("{}: insufficient covered shares", ins.symbol));
            }
        } else if !manual
            .iter()
            .any(|r| r.position_id.as_deref() == Some(&pos.id))
        {
            out.complete = false;
            out.reasons
                .push(format!("{} needs an explicit cash reserve", ins.symbol));
        }
    }
    Ok(out)
}
