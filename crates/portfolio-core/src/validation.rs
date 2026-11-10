use crate::{decimal::*, *};
use rust_decimal::Decimal as D;
use std::collections::HashSet;
fn fail(s: impl Into<String>) -> Error {
    Error::Invalid(s.into())
}
fn ids<'a>(name: &str, items: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = HashSet::new();
    for id in items {
        if id.trim().is_empty() || id.len() > 200 || !seen.insert(id) {
            return Err(fail(format!(
                "Empty, oversized or duplicate {name} identifier: {id}"
            )));
        }
    }
    Ok(())
}
fn ccy(s: &str) -> Result<()> {
    if s.len() != 3 || !s.bytes().all(|x| x.is_ascii_uppercase()) {
        return Err(fail("Currencies must be three uppercase letters"));
    }
    Ok(())
}
pub fn validate(p: &Portfolio) -> Result<()> {
    if p.schema_version != 1 || p.policy_version != 1 {
        return Err(fail("Unsupported data or accounting policy version"));
    }
    if p.dataset_id.is_empty() {
        return Err(fail("Dataset identifier is missing"));
    }
    macro_rules! unique {
        ($f:ident) => {
            ids(stringify!($f), p.$f.iter().map(|x| x.id.as_str()))?;
        };
    }
    unique!(accounts);
    unique!(cycles);
    unique!(instruments);
    unique!(positions);
    unique!(transactions);
    unique!(cash_movements);
    unique!(reservations);
    unique!(market_data);
    unique!(fx_rates);
    unique!(snapshots);
    for a in &p.accounts {
        if a.name.trim().is_empty() {
            return Err(fail("Account name is required"));
        }
        if a.currencies.is_empty()
            || a.currencies.iter().collect::<HashSet<_>>().len() != a.currencies.len()
        {
            return Err(fail("Enable each account currency exactly once"));
        }
        for x in &a.currencies {
            ccy(x)?
        }
    }
    for c in &p.cycles {
        date(&c.start_date)?;
        if c.name.trim().is_empty() {
            return Err(fail("Cycle name is required"));
        }
        if let Some(x) = &c.stopped_on {
            date(x)?;
            if x < &c.start_date {
                return Err(fail("Cycle stop precedes start"));
            }
        }
        if c.state != CycleState::Active && c.stopped_on.is_none() {
            return Err(fail("A stopped cycle requires its stop date"));
        }
        if let Some(x) = &c.closed_on {
            date(x)?;
            if c.stopped_on.as_ref().is_none_or(|s| x < s) {
                return Err(fail("Cycle closure precedes stop date"));
            }
        }
        if c.state == CycleState::Closed && c.closed_on.is_none() {
            return Err(fail("Closed cycle requires a settlement date"));
        }
    }
    for i in &p.instruments {
        ccy(&i.currency)?;
        number(i.multiplier, "Multiplier", false)?;
        if i.symbol.trim().is_empty() || i.multiplier <= D::ZERO {
            return Err(fail("Symbol and a positive multiplier are required"));
        }
        if i.asset == Asset::Option {
            if i.option_right.is_none()
                || i.strike.is_none_or(|x| x <= D::ZERO)
                || i.expiry.is_none()
                || i.underlying.as_deref().is_none_or(|x| x.is_empty())
            {
                return Err(fail(format!("{}: option terms are incomplete", i.symbol)));
            }
            date(i.expiry.as_deref().unwrap())?;
        }
    }
    for pos in &p.positions {
        let a = p.account(&pos.account_id)?;
        let ins = p.instrument(&pos.instrument_id)?;
        let c = p.cycle(&pos.cycle_id)?;
        if !a.currencies.contains(&ins.currency) {
            return Err(fail("Position currency is not enabled for its account"));
        }
        if let Some(v) = pos.stop_price {
            number(v, "Stop", false)?
        }
        if let Some(id) = &pos.covered_by {
            let cov = p.position(id)?;
            let ci = p.instrument(&cov.instrument_id)?;
            if cov.id == pos.id
                || cov.cycle_id != pos.cycle_id
                || cov.account_id != pos.account_id
                || pos.direction != Direction::Short
                || cov.direction != Direction::Long
                || ci.asset != Asset::Stock
                || ins.asset != Asset::Option
                || ins.option_right != Some(OptionRight::Call)
                || ins.underlying.as_deref() != Some(&ci.symbol)
                || ci.currency != ins.currency
            {
                return Err(fail(
                    "Coverage requires matching long shares in the same cycle and account",
                ));
            }
        }
        let l = position_ledger(p, &pos.id, None)?;
        if c.state == CycleState::Closed && (l.quantity != D::ZERO || pos.pending_lifecycle) {
            return Err(fail(
                "A cycle with open positions or pending settlement cannot be closed",
            ));
        }
    }
    for cover in p.positions.iter().filter(|stock| {
        p.positions
            .iter()
            .any(|x| x.covered_by.as_deref() == Some(&stock.id))
    }) {
        let dates: std::collections::BTreeSet<_> =
            p.transactions.iter().map(|t| t.date.as_str()).collect();
        for d in dates {
            let supplied = position_ledger(p, &cover.id, Some(d))?.quantity;
            let mut needed = D::ZERO;
            for option in p
                .positions
                .iter()
                .filter(|x| x.covered_by.as_deref() == Some(&cover.id))
            {
                needed = add(
                    needed,
                    mul(
                        position_ledger(p, &option.id, Some(d))?.quantity,
                        p.instrument(&option.instrument_id)?.multiplier,
                    )?,
                )?;
            }
            if supplied < needed {
                return Err(fail("Covered shares cannot be sold while the remaining option obligation still binds them. Close the option or explicitly change its coverage and reservation."));
            }
        }
    }
    for t in &p.transactions {
        if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&t.order) {
            return Err(fail("Transaction order exceeds the portable integer range"));
        }
        let pos = p.position(&t.position_id)?;
        let i = p.instrument(&pos.instrument_id)?;
        let c = p.cycle(&pos.cycle_id)?;
        date(&t.date)?;
        if t.date < c.start_date {
            return Err(fail(format!("{}: transaction predates cycle start", t.id)));
        }
        if t.cycle_id.as_ref().is_some_and(|x| x != &pos.cycle_id) {
            return Err(fail("A cycle cannot transact another cycle's position"));
        }
        number(t.quantity, "Quantity", false)?;
        number(t.price, "Price", false)?;
        number(t.fee, "Fee", false)?;
        if let Some(b) = t.basis_total {
            number(b, "Reported basis", true)?;
            if !matches!(t.action, Action::Basis | Action::Snapshot) {
                return Err(fail("Only basis/snapshot inputs can override total basis"));
            }
        }
        if t.action == Action::Basis {
            if t.quantity != D::ZERO || t.fee != D::ZERO {
                return Err(fail("Basis adjustment quantity and fee must be zero"));
            }
        } else if t.quantity <= D::ZERO {
            return Err(fail("Quantity must be positive"));
        }
        if i.asset == Asset::Option && t.quantity.fract() != D::ZERO {
            return Err(fail("Option quantities must be whole contracts"));
        }
        if t.action == Action::Expiry
            && (i.asset != Asset::Option || i.expiry.as_ref().is_none_or(|d| t.date < *d))
        {
            return Err(fail(
                "Expiry requires an option on or after its expiry date",
            ));
        }
        if t.action == Action::Open
            && i.asset == Asset::Option
            && i.expiry.as_ref().is_some_and(|d| t.date > *d)
        {
            return Err(fail("An option cannot be opened after expiry"));
        }
        if t.action == Action::Split && (t.price != D::ZERO || t.fee != D::ZERO) {
            return Err(fail(
                "A stock split carries a ratio, not a price or cash fee",
            ));
        }
        if matches!(t.action, Action::Expiry) && t.price != D::ZERO {
            return Err(fail("An expiry price must be zero"));
        }
        if t.action == Action::Open
            && c.stopped_on.as_ref().is_some_and(|d| t.date > *d)
            && t.settlement_id.is_none()
        {
            return Err(fail(
                "A winding-down cycle cannot open or increase discretionary positions",
            ));
        }
        if c.closed_on.as_ref().is_some_and(|d| t.date > *d) {
            return Err(fail("Transaction follows cycle settlement date"));
        }
        if let Some(id) = &t.settlement_id {
            let event = p
                .transactions
                .iter()
                .find(|x| &x.id == id)
                .ok_or_else(|| fail("Settlement link is missing"))?;
            let ep = p.position(&event.position_id)?;
            if !matches!(event.action, Action::Assignment | Action::Exercise)
                || ep.cycle_id != pos.cycle_id
                || ep.account_id != pos.account_id
                || event.date != t.date
            {
                return Err(fail(
                    "Settlement must reference an option event in the same cycle/account/date",
                ));
            }
        }
    }
    for event in p
        .transactions
        .iter()
        .filter(|t| matches!(t.action, Action::Assignment | Action::Exercise))
    {
        let pos = p.position(&event.position_id)?;
        let i = p.instrument(&pos.instrument_id)?;
        if i.asset != Asset::Option {
            return Err(fail("Only options can be exercised or assigned"));
        }
        if (event.action == Action::Assignment) != (pos.direction == Direction::Short) {
            return Err(fail(
                "Assignment is a short-option event; exercise is a long-option event",
            ));
        }
        let linked: Vec<_> = p
            .transactions
            .iter()
            .filter(|t| t.settlement_id.as_deref() == Some(&event.id))
            .collect();
        if i.settlement == Settlement::Cash {
            if !linked.is_empty() {
                return Err(fail("Cash settlement must not create underlying shares"));
            }
        } else if !(linked.is_empty()
            && !event.source.is_empty()
            && (pos.pending_lifecycle
                || pos
                    .settlement_reconciled_on
                    .as_ref()
                    .is_some_and(|d| d >= &event.date)))
        {
            if event.price != D::ZERO {
                return Err(fail("Physical option settlement closes the option at zero"));
            }
            let delivery_sign = if (i.option_right == Some(OptionRight::Call))
                == (pos.direction == Direction::Long)
            {
                D::ONE
            } else {
                -D::ONE
            };
            let expected = mul(delivery_sign, mul(event.quantity, i.multiplier)?)?;
            let mut actual = D::ZERO;
            for t in linked {
                let up = p.position(&t.position_id)?;
                let ui = p.instrument(&up.instrument_id)?;
                if ui.asset != Asset::Stock
                    || ui.symbol != i.underlying.as_deref().unwrap_or_default()
                    || ui.currency != i.currency
                    || ui.multiplier != D::ONE
                    || t.price != i.strike.unwrap_or_default()
                    || !matches!(t.action, Action::Open | Action::Close)
                {
                    return Err(fail(
                        "Option deliverable does not match the underlying, strike or action",
                    ));
                }
                let effect = mul(up.direction.sign(), t.quantity)?;
                actual = add(
                    actual,
                    if t.action == Action::Open {
                        effect
                    } else {
                        -effect
                    },
                )?;
            }
            if actual != expected {
                return Err(fail(
                    "Option settlement does not conserve deliverable quantity",
                ));
            }
        }
    }
    for m in &p.cash_movements {
        if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&m.order) {
            return Err(fail(
                "Cash movement order exceeds the portable integer range",
            ));
        }
        let a = p.account(&m.account_id)?;
        let c = p.cycle(&m.cycle_id)?;
        date(&m.date)?;
        ccy(&m.currency)?;
        number(m.amount, "Amount", m.kind == CashKind::Baseline)?;
        if !a.currencies.contains(&m.currency) || m.date < c.start_date {
            return Err(fail(
                "Cash movement has invalid currency or predates its cycle",
            ));
        }
        if matches!(
            m.kind,
            CashKind::CycleTransfer | CashKind::AccountTransfer | CashKind::Fx
        ) {
            let dc = m.destination_cycle_id.as_deref().unwrap_or(&m.cycle_id);
            let da = m.destination_account_id.as_deref().unwrap_or(&m.account_id);
            let destination_cycle = p.cycle(dc)?;
            if m.date < destination_cycle.start_date {
                return Err(fail("Transfer predates the destination cycle"));
            }
            let dst = p.account(da)?;
            if m.kind != CashKind::Fx && !dst.currencies.contains(&m.currency) {
                return Err(fail(
                    "Enable the transfer currency in the destination account",
                ));
            }
            if m.kind == CashKind::Fx && (dc != m.cycle_id || da != m.account_id) {
                return Err(fail("Record FX within one account and cycle; use a separate balanced transfer for allocation"));
            }
            if m.kind == CashKind::CycleTransfer && (dc == m.cycle_id || da != m.account_id) {
                return Err(fail(
                    "Select a different cycle in the same account for a cycle transfer",
                ));
            }
            if m.kind == CashKind::AccountTransfer && (da == m.account_id || dc != m.cycle_id) {
                return Err(fail(
                    "Account transfers stay in the same cycle and require another account",
                ));
            }
            if m.kind == CashKind::Fx {
                let rc = m
                    .receive_currency
                    .as_deref()
                    .ok_or_else(|| fail("FX destination currency is required"))?;
                ccy(rc)?;
                if rc == m.currency
                    || m.receive_amount.is_none_or(|x| x <= D::ZERO)
                    || !dst.currencies.contains(&rc.to_string())
                {
                    return Err(fail(
                        "FX requires distinct supported currencies and a positive received amount",
                    ));
                }
            } else if m.receive_amount.is_some() || m.receive_currency.is_some() {
                return Err(fail(
                    "Only an FX conversion has a different received amount/currency",
                ));
            }
            let prior = cash_until(
                p,
                &m.account_id,
                &m.cycle_id,
                &m.currency,
                &m.date,
                Some((&m.date, m.order, &m.id)),
            )?;
            let mut prefix = p.clone();
            prefix.transactions.retain(|t| {
                (t.date.as_str(), t.order, t.id.as_str())
                    < (m.date.as_str(), m.order, m.id.as_str())
            });
            let r = reserves(&prefix, &m.account_id, &m.cycle_id, &m.currency, &m.date)?;
            if !r.complete || m.amount > sub(prior, r.amount)?.max(D::ZERO) {
                return Err(fail(format!(
                    "{}: transfer exceeds available cash or commitments are unknown",
                    m.id
                )));
            }
        } else if m.destination_cycle_id.is_some()
            || m.destination_account_id.is_some()
            || m.receive_currency.is_some()
            || m.receive_amount.is_some()
        {
            return Err(fail("Only transfers and FX can have destination fields"));
        }
    }
    for r in &p.reservations {
        p.account(&r.account_id)?;
        p.cycle(&r.cycle_id)?;
        ccy(&r.currency)?;
        if !p.account(&r.account_id)?.currencies.contains(&r.currency) {
            return Err(fail("Reservation currency is not enabled for its account"));
        }
        date(&r.start_date)?;
        number(r.amount, "Reservation", false)?;
        if let Some(d) = &r.end_date {
            date(d)?;
            if d < &r.start_date {
                return Err(fail("Reservation end precedes its start"));
            }
        }
        if let Some(id) = &r.position_id {
            let x = p.position(id)?;
            if x.account_id != r.account_id || x.cycle_id != r.cycle_id {
                return Err(fail("Reservation belongs to a different position owner"));
            }
        }
    }
    if !(20..=252).contains(&p.settings.volatility_window) {
        return Err(fail("Volatility window must be 20–252 observations"));
    }
    if let Some(c) = &p.settings.reporting_currency {
        ccy(c)?;
    }
    for d in &p.settings.holidays {
        date(d)?;
    }
    let mut quote_keys = HashSet::new();
    for q in &p.market_data {
        if !quote_keys.insert((&q.instrument_id, &q.date, q.adjusted)) {
            return Err(fail(
                "Edit the existing quote for this instrument, date and price series",
            ));
        }
        p.instrument(&q.instrument_id)?;
        date(&q.date)?;
        number(q.price, "Quote", false)?;
        if q.delta.is_some_and(|x| x < -D::ONE || x > D::ONE) {
            return Err(fail("Delta must be between -1 and 1"));
        }
        if let Some(x) = q.underlying_price {
            number(x, "Underlying quote", false)?
        }
    }
    let mut fx_keys = HashSet::new();
    for f in &p.fx_rates {
        let pair = if f.from < f.to {
            (&f.from, &f.to)
        } else {
            (&f.to, &f.from)
        };
        if !fx_keys.insert((pair, &f.date)) {
            return Err(fail(
                "Edit the existing exchange rate for this currency pair and date",
            ));
        }
        ccy(&f.from)?;
        ccy(&f.to)?;
        date(&f.date)?;
        number(f.rate, "FX rate", false)?;
        if f.rate <= D::ZERO || f.from == f.to {
            return Err(fail("FX requires different currencies and a positive rate"));
        }
    }
    for s in &p.snapshots {
        date(&s.date)?;
        ccy(&s.currency)?;
        // Snapshot values can contain the full precision of app-computed valuations.
        if let Some(id) = &s.cycle_id {
            p.cycle(id)?;
        }
        if let Some(id) = &s.account_id {
            p.account(id)?;
        }
        if !["baseline", "close", "pre_flow", "post_flow", "stop"].contains(&s.phase.as_str()) {
            return Err(fail("Unknown valuation phase"));
        }
    }
    Ok(())
}
pub fn validate_transition(before: &Portfolio, after: &Portfolio) -> Result<()> {
    for old in &before.instruments {
        if before.positions.iter().any(|pos| {
            pos.instrument_id == old.id
                && before.transactions.iter().any(|t| t.position_id == pos.id)
        }) {
            if let Some(next) = after.instruments.iter().find(|i| i.id == old.id) {
                if serde_json::to_value(old)? != serde_json::to_value(next)? {
                    return Err(fail("Executed instrument terms are permanent. Correct transaction facts within the position, or record a corporate action explicitly."));
                }
            }
        }
    }
    for old in &before.transactions {
        if after
            .transactions
            .iter()
            .any(|t| t.id == old.id && t.position_id != old.position_id)
        {
            return Err(fail(
                "Transaction ownership is permanent; correct the event within its position",
            ));
        }
    }
    for pos in &before.positions {
        if let Some(x) = after.positions.iter().find(|x| x.id == pos.id) {
            if x.cycle_id != pos.cycle_id
                || x.account_id != pos.account_id
                || x.instrument_id != pos.instrument_id
                || x.direction != pos.direction
            {
                return Err(fail(
                    "Position ownership, instrument and direction are permanent",
                ));
            }
        } else if before.transactions.iter().any(|t| t.position_id == pos.id) {
            return Err(fail(
                "A position with transaction history cannot be deleted",
            ));
        }
    }
    for c in &before.cycles {
        if let Some(x) = after.cycles.iter().find(|x| x.id == c.id) {
            if c.state != CycleState::Active && x.state == CycleState::Active {
                return Err(fail(
                    "A stopped strategy cannot be reactivated; start a new cycle",
                ));
            }
        }
    }
    for t in &after.transactions {
        if t.action == Action::Open
            && t.settlement_id.is_none()
            && !before.transactions.iter().any(|old| old.id == t.id)
        {
            let pos = after.position(&t.position_id)?;
            if before
                .cycles
                .iter()
                .any(|c| c.id == pos.cycle_id && c.state != CycleState::Active)
            {
                return Err(fail(
                    "A stopped cycle cannot receive new discretionary entries",
                ));
            }
        }
    }
    validate(after)
}
