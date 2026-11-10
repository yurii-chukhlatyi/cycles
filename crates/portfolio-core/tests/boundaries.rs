use portfolio_core::*;
use rust_decimal::Decimal as D;
use std::str::FromStr;
fn d(s: &str) -> D {
    D::from_str(s).unwrap()
}
fn option() -> Portfolio {
    let mut p = Portfolio::example();
    p.instruments[0].asset = Asset::Option;
    p.instruments[0].multiplier = d("100");
    p.instruments[0].option_right = Some(OptionRight::Put);
    p.instruments[0].strike = Some(d("100"));
    p.instruments[0].expiry = Some("2026-12-18".into());
    p.instruments[0].underlying = Some("TEST".into());
    p.positions[0].direction = Direction::Short;
    p
}
#[test]
fn transfer_cannot_use_collateral_freed_later_the_same_day() {
    let mut p = option();
    p.cycles.push(Cycle::new("next", "Next", "2026-01-01"));
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        d("10100"),
    ));
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-02",
        0,
        Action::Open,
        d("1"),
        d("2"),
        D::ZERO,
    ));
    p.transactions.push(Transaction::trade(
        "close",
        "pos",
        "2026-01-03",
        2,
        Action::Close,
        d("1"),
        D::ZERO,
        D::ZERO,
    ));
    let mut t = CashMovement::funding("move", "account", "cycle", "USD", "2026-01-03", d("1000"));
    t.order = 1;
    t.kind = CashKind::CycleTransfer;
    t.destination_cycle_id = Some("next".into());
    p.cash_movements.push(t);
    assert!(validate(&p).is_err());
}
#[test]
fn settlement_link_cannot_forge_unrelated_underlying_trades() {
    let mut p = option();
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-02",
        0,
        Action::Open,
        d("1"),
        d("2"),
        D::ZERO,
    ));
    p.transactions.push(Transaction::trade(
        "assignment",
        "pos",
        "2026-01-03",
        0,
        Action::Assignment,
        d("1"),
        D::ZERO,
        D::ZERO,
    ));
    let mut ins = p.instruments[0].clone();
    ins.id = "underlying".into();
    ins.asset = Asset::Stock;
    ins.multiplier = D::ONE;
    p.instruments.push(ins);
    let mut pos = p.positions[0].clone();
    pos.id = "shares".into();
    pos.instrument_id = "underlying".into();
    pos.direction = Direction::Long;
    p.positions.push(pos);
    let mut t = Transaction::trade(
        "forged",
        "shares",
        "2026-01-03",
        1,
        Action::Open,
        d("9999"),
        d("1"),
        D::ZERO,
    );
    t.settlement_id = Some("assignment".into());
    p.transactions.push(t);
    assert!(validate(&p).is_err());
}
#[test]
fn partial_put_close_releases_only_matching_collateral() {
    let mut p = option();
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-02",
        0,
        Action::Open,
        d("2"),
        d("2"),
        D::ZERO,
    ));
    p.transactions.push(Transaction::trade(
        "close",
        "pos",
        "2026-01-03",
        0,
        Action::Close,
        d("1"),
        d("1"),
        D::ZERO,
    ));
    assert_eq!(
        reserves(&p, "account", "cycle", "USD", "2026-01-03")
            .unwrap()
            .amount,
        d("10000")
    );
}
#[test]
fn period_profit_uses_the_change_in_unrealized_value() {
    let mut p = Portfolio::example();
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-01",
        0,
        Action::Open,
        d("10"),
        d("100"),
        D::ZERO,
    ));
    p.market_data
        .push(MarketData::quote("start", "stock", "2026-01-02", d("120")));
    p.market_data
        .push(MarketData::quote("end", "stock", "2026-01-05", d("122")));
    let mut f = ReportFilter::at("2026-01-05");
    f.from = Some("2026-01-02".into());
    let r = report(&p, &f).unwrap();
    assert_eq!(r.positions[0].total_pl, Some(d("220")));
    assert_eq!(r.positions[0].period_pl, Some(d("20")));
}
#[test]
fn no_future_quotes_and_ambiguous_irr() {
    let mut p = Portfolio::example();
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-01",
        0,
        Action::Open,
        d("1"),
        d("100"),
        D::ZERO,
    ));
    p.market_data
        .push(MarketData::quote("future", "stock", "2026-01-10", d("200")));
    assert!(report(&p, &ReportFilter::at("2026-01-05"))
        .unwrap()
        .positions[0]
        .market_value
        .is_none());
    let a = date("2026-01-01").unwrap();
    let b = date("2027-01-01").unwrap();
    let c = date("2028-01-01").unwrap();
    assert!(xirr(&[(a, -100.0), (b, 230.0), (c, -132.0)]).0.is_none());
}

#[test]
fn noncanonical_dates_and_duplicate_account_currencies_are_rejected() {
    assert!(date("2026-1-01").is_err());
    let mut p = Portfolio::example();
    p.accounts[0].currencies.push("USD".into());
    assert!(validate(&p).is_err());
}
#[test]
fn transfer_to_an_account_without_the_currency_cannot_hide_cash() {
    let mut p = Portfolio::example();
    p.accounts.push(Account {
        id: "cad-only".into(),
        name: "CAD account".into(),
        broker: String::new(),
        currencies: vec!["CAD".into()],
    });
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        d("1000"),
    ));
    let mut m = CashMovement::funding(
        "transfer",
        "account",
        "cycle",
        "USD",
        "2026-01-02",
        d("100"),
    );
    m.kind = CashKind::AccountTransfer;
    m.destination_account_id = Some("cad-only".into());
    p.cash_movements.push(m);
    assert!(validate(&p).is_err());
}

#[test]
fn event_order_must_survive_the_native_json_boundary() {
    let mut p = Portfolio::example();
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-01",
        i64::MAX,
        Action::Open,
        d("1"),
        d("10"),
        D::ZERO,
    ));
    assert!(validate(&p)
        .unwrap_err()
        .to_string()
        .contains("portable integer"));
    p.transactions[0].order = 1;
    let mut movement =
        CashMovement::funding("fund", "account", "cycle", "USD", "2026-01-01", d("1000"));
    movement.order = i64::MIN;
    p.cash_movements.push(movement);
    assert!(validate(&p)
        .unwrap_err()
        .to_string()
        .contains("portable integer"));
}
