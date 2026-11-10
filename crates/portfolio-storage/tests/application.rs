use portfolio_core::*;
use portfolio_storage::*;
use rust_decimal::Decimal as D;
use serde_json::json;
#[test]
fn a_transaction_cannot_be_written_through_another_cycle_or_moved_to_it() {
    let mut p = sample_portfolio();
    p.cycles
        .push(Cycle::new("other", "Different strategy", "2026-01-01"));
    let mut other = p.positions[0].clone();
    other.id = "other-position".into();
    other.cycle_id = "other".into();
    p.positions.push(other);
    let j = Journal::from_portfolio(&p).unwrap();
    let head = j.replay().unwrap().head;
    let mut event = p.transactions[0].clone();
    event.position_id = "other-position".into();
    event.cycle_id = Some("other".into());
    let change = Change::upsert("Transactions", serde_json::to_value(event).unwrap());
    assert!(stage(&j, &head, Some("other"), vec![change.clone()]).is_err());
    assert!(prepare(&j.replay().unwrap(), vec![change], "move", "test").is_err());
    let change = Change::upsert(
        "Transactions",
        json!({"id":"new","position_id":"pos","cycle_id":"cycle","date":"2026-11-09","order":2,"action":"close","quantity":"1","price":"120","fee":"0"}),
    );
    assert!(stage(&j, &head, Some("other"), vec![change]).is_err());
}
#[test]
fn winding_down_preserves_ownership_and_only_accepts_obligated_new_exposure() {
    let p = sample_portfolio();
    let mut stopped =
        change_cycle(&p, "cycle", CycleState::WindingDown, "2026-11-09", "head").unwrap();
    assert!(change_cycle(&stopped, "cycle", CycleState::Closed, "2026-11-09", "head").is_err());
    stopped.transactions.push(Transaction::trade(
        "increase",
        "pos",
        "2026-11-09",
        5,
        Action::Open,
        D::ONE,
        D::from(100),
        D::ZERO,
    ));
    assert!(validate_transition(&p, &stopped).is_ok()); // Transitioning and recording a trade before the stop is allowed.
    let mut again = stopped.clone();
    again.transactions.push(Transaction::trade(
        "new-after-stop",
        "pos",
        "2026-11-09",
        6,
        Action::Open,
        D::ONE,
        D::from(100),
        D::ZERO,
    ));
    assert!(validate_transition(&stopped, &again).is_err());
}

#[test]
fn flow_time_valuations_require_measured_equity_and_the_scoped_capital_event() {
    let p = sample_portfolio();
    let flow = &p.cash_movements[0];
    let f = ReportFilter::at(&flow.date);
    assert!(valuation_changes(&p, &f, "pre_flow", &flow.id, None, Some("USD"), "head").is_err());
    assert!(valuation_changes(
        &p,
        &f,
        "pre_flow",
        "missing-flow",
        Some(D::from(123)),
        Some("USD"),
        "head"
    )
    .is_err());
    let changes = valuation_changes(
        &p,
        &f,
        "pre_flow",
        &flow.id,
        Some(D::from(123)),
        Some("USD"),
        "head",
    )
    .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].value.as_ref().unwrap()["equity"], "123");
    assert_eq!(changes[0].value.as_ref().unwrap()["source"], flow.id);
}
