use crate::{decimal::*, *};
use rust_decimal::Decimal as D;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementRequest {
    pub position_id: String,
    pub date: String,
    pub quantity: D,
    pub fee: D,
    #[serde(default)]
    pub cash_price: Option<D>,
    #[serde(default)]
    pub underlying_position_id: Option<String>,
    #[serde(default)]
    pub order: i64,
}
pub fn settle(p: &Portfolio, request: &SettlementRequest) -> Result<Portfolio> {
    let pos = p.position(&request.position_id)?;
    let i = p.instrument(&pos.instrument_id)?;
    if i.asset != Asset::Option {
        return Err(Error::Invalid(
            "Settlement requires an option position".into(),
        ));
    }
    let remaining = position_ledger(p, &pos.id, Some(&request.date))?.quantity;
    if request.quantity <= D::ZERO || request.quantity > remaining {
        return Err(Error::Invalid(
            "Settlement quantity exceeds remaining contracts".into(),
        ));
    }
    let mut next = p.clone();
    let event_id = new_id();
    let mut event = Transaction::trade(
        &event_id,
        &pos.id,
        &request.date,
        request.order,
        if pos.direction == Direction::Short {
            Action::Assignment
        } else {
            Action::Exercise
        },
        request.quantity,
        if i.settlement == Settlement::Cash {
            request
                .cash_price
                .ok_or_else(|| Error::Invalid("Enter the cash settlement quote".into()))?
        } else {
            D::ZERO
        },
        request.fee,
    );
    event.cycle_id = Some(pos.cycle_id.clone());
    next.transactions.push(event);
    if i.settlement == Settlement::Physical {
        let underlying = i
            .underlying
            .as_deref()
            .ok_or_else(|| Error::Invalid("Underlying symbol is missing".into()))?;
        let sign =
            if (i.option_right == Some(OptionRight::Call)) == (pos.direction == Direction::Long) {
                Direction::Long
            } else {
                Direction::Short
            };
        let mut q = mul(request.quantity, i.multiplier)?;
        let mut order = request.order + 1;
        if let Some(id) = &request.underlying_position_id {
            let up = p.position(id)?;
            let ui = p.instrument(&up.instrument_id)?;
            if up.cycle_id != pos.cycle_id
                || up.account_id != pos.account_id
                || ui.asset != Asset::Stock
                || ui.symbol != underlying
                || ui.currency != i.currency
            {
                return Err(Error::Invalid(
                    "Select matching underlying shares in the same cycle and account".into(),
                ));
            }
            let l = position_ledger(p, id, Some(&request.date))?;
            if up.direction != sign && l.quantity > D::ZERO {
                let take = q.min(l.quantity);
                let mut t = Transaction::trade(
                    &new_id(),
                    id,
                    &request.date,
                    order,
                    Action::Close,
                    take,
                    i.strike.unwrap_or_default(),
                    D::ZERO,
                );
                t.settlement_id = Some(event_id.clone());
                t.cycle_id = Some(pos.cycle_id.clone());
                next.transactions.push(t);
                q = sub(q, take)?;
                order += 1;
            }
        }
        if q > D::ZERO {
            let instrument = next
                .instruments
                .iter()
                .find(|x| {
                    x.asset == Asset::Stock && x.symbol == underlying && x.currency == i.currency
                })
                .map(|x| x.id.clone())
                .unwrap_or_else(new_id);
            if !next.instruments.iter().any(|x| x.id == instrument) {
                next.instruments.push(Instrument {
                    id: instrument.clone(),
                    symbol: underlying.into(),
                    asset: Asset::Stock,
                    currency: i.currency.clone(),
                    multiplier: D::ONE,
                    option_right: None,
                    strike: None,
                    expiry: None,
                    underlying: None,
                    settlement: Settlement::Physical,
                    deliverable: String::new(),
                });
            }
            let id = new_id();
            next.positions.push(Position {
                id: id.clone(),
                instrument_id: instrument,
                account_id: pos.account_id.clone(),
                cycle_id: pos.cycle_id.clone(),
                direction: sign,
                name: format!("Settlement · {}", i.symbol),
                stop_price: None,
                covered_by: None,
                notes: format!("Linked to option settlement {event_id}"),
                pending_lifecycle: false,
                settlement_reconciled_on: None,
                source: String::new(),
            });
            let mut t = Transaction::trade(
                &new_id(),
                &id,
                &request.date,
                order,
                Action::Open,
                q,
                i.strike.unwrap_or_default(),
                D::ZERO,
            );
            t.settlement_id = Some(event_id);
            t.cycle_id = Some(pos.cycle_id.clone());
            next.transactions.push(t);
        }
    }
    if request.quantity == remaining {
        if let Some(x) = next.positions.iter_mut().find(|x| x.id == pos.id) {
            x.pending_lifecycle = false;
        }
        for r in next
            .reservations
            .iter_mut()
            .filter(|r| r.position_id.as_deref() == Some(&pos.id) && r.end_date.is_none())
        {
            r.end_date = Some(request.date.clone());
        }
    }
    validate_transition(p, &next)?;
    Ok(next)
}
