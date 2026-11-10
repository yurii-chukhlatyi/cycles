use portfolio_core::*;
use rust_decimal::Decimal;
use std::str::FromStr;
fn d(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}
#[test]
fn partial_close_and_later_entries_keep_realized_profit() {
    let mut p = Portfolio::example();
    p.transactions = vec![
        Transaction::trade(
            "a",
            "pos",
            "2026-01-02",
            0,
            Action::Open,
            d("10"),
            d("100"),
            d("0"),
        ),
        Transaction::trade(
            "b",
            "pos",
            "2026-01-03",
            0,
            Action::Open,
            d("10"),
            d("120"),
            d("0"),
        ),
        Transaction::trade(
            "c",
            "pos",
            "2026-01-04",
            0,
            Action::Close,
            d("5"),
            d("130"),
            d("0"),
        ),
    ];
    let ledger = position_ledger(&p, "pos", None).unwrap();
    assert_eq!(ledger.quantity, d("15"));
    assert_eq!(ledger.basis, d("1650"));
    assert_eq!(ledger.realized, d("100"));
    p.market_data
        .push(MarketData::quote("q", "stock", "2026-01-05", d("125")));
    let report = report(&p, &ReportFilter::at("2026-01-05")).unwrap();
    assert_eq!(report.positions[0].unrealized, Some(d("225")));
    assert_eq!(report.positions[0].total_pl, Some(d("325")));
}
#[test]
fn fees_and_short_option_contracts_have_correct_signs() {
    let mut p = Portfolio::example();
    p.transactions = vec![
        Transaction::trade(
            "a",
            "pos",
            "2026-01-02",
            0,
            Action::Open,
            d("10"),
            d("100"),
            d("2"),
        ),
        Transaction::trade(
            "b",
            "pos",
            "2026-01-03",
            0,
            Action::Close,
            d("4"),
            d("110"),
            d("1"),
        ),
    ];
    let l = position_ledger(&p, "pos", None).unwrap();
    assert_eq!(l.realized, d("38.2"));
    assert_eq!(l.basis, d("601.2"));
    p.instruments[0].asset = Asset::Option;
    p.instruments[0].multiplier = d("100");
    p.instruments[0].option_right = Some(OptionRight::Put);
    p.instruments[0].strike = Some(d("100"));
    p.instruments[0].expiry = Some("2026-12-18".into());
    p.instruments[0].underlying = Some("TEST".into());
    p.positions[0].direction = Direction::Short;
    p.transactions = vec![
        Transaction::trade(
            "a",
            "pos",
            "2026-01-02",
            0,
            Action::Open,
            d("1"),
            d("2"),
            d("0"),
        ),
        Transaction::trade(
            "b",
            "pos",
            "2026-01-03",
            0,
            Action::Close,
            d("1"),
            d("0.5"),
            d("0"),
        ),
    ];
    let l = position_ledger(&p, "pos", None).unwrap();
    assert_eq!(l.realized, d("150"));
    assert_eq!(l.cash, d("150"));
}
#[test]
fn ownership_and_cash_transfers_are_validated() {
    let mut p = Portfolio::example();
    p.cycles.push(Cycle::new("next", "Next", "2026-01-01"));
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        d("10000"),
    ));
    p.transactions.push(Transaction::trade(
        "buy",
        "pos",
        "2026-01-02",
        0,
        Action::Open,
        d("60"),
        d("100"),
        d("0"),
    ));
    let mut movement = CashMovement::funding(
        "transfer",
        "account",
        "cycle",
        "USD",
        "2026-01-03",
        d("3000"),
    );
    movement.kind = CashKind::CycleTransfer;
    movement.destination_cycle_id = Some("next".into());
    p.cash_movements.push(movement);
    validate(&p).unwrap();
    assert_eq!(
        cash_balance(&p, "account", "cycle", "USD", "2026-01-03").unwrap(),
        d("1000")
    );
    assert_eq!(
        cash_balance(&p, "account", "next", "USD", "2026-01-03").unwrap(),
        d("3000")
    );
    let mut bad = p.clone();
    bad.cash_movements.last_mut().unwrap().amount = d("5000");
    assert!(validate(&bad).is_err());
    let mut tx = Transaction::trade(
        "bad",
        "pos",
        "2026-01-04",
        0,
        Action::Close,
        d("1"),
        d("100"),
        d("0"),
    );
    tx.cycle_id = Some("next".into());
    p.transactions.push(tx);
    assert!(validate(&p).is_err());
}
#[test]
fn imported_identifiers_and_decimals_round_trip() {
    let mut p = Portfolio::example();
    p.positions[0].id = "LEGACY-LOT-1".into();
    let json = serde_json::to_string(&p).unwrap();
    let copy: Portfolio = serde_json::from_str(&json).unwrap();
    assert_eq!(copy.positions[0].id, "LEGACY-LOT-1");
    validate(&copy).unwrap();
}
