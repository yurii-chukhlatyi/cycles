use portfolio_core::*;
use rust_decimal::Decimal as D;
fn rate(p: &mut Portfolio, id: &str, date: &str, value: i64, scale: u32) {
    p.fx_rates.push(FxRate {
        id: id.into(),
        from: "USD".into(),
        to: "CAD".into(),
        date: date.into(),
        rate: D::new(value, scale),
        source: "fixture".into(),
    });
}
#[test]
fn local_profit_and_currency_translation_reconcile_to_consolidated_gain() {
    let mut p = Portfolio::example();
    p.settings.reporting_currency = Some("CAD".into());
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        D::from(100),
    ));
    p.transactions.push(Transaction::trade(
        "buy",
        "pos",
        "2026-01-02",
        1,
        Action::Open,
        D::ONE,
        D::from(100),
        D::ZERO,
    ));
    p.market_data
        .push(MarketData::quote("q1", "stock", "2026-01-02", D::from(100)));
    p.market_data
        .push(MarketData::quote("q2", "stock", "2026-01-05", D::from(110)));
    rate(&mut p, "fx1", "2026-01-01", 1, 0);
    rate(&mut p, "fx2", "2026-01-05", 14, 1);
    let c = report(&p, &ReportFilter::at("2026-01-05"))
        .unwrap()
        .consolidated
        .unwrap();
    assert_eq!(c.equity, Some(D::from(154)));
    assert_eq!(c.performance.investment_gain, Some(D::from(54)));
    assert_eq!(c.position_pl, Some(D::from(14)));
    assert_eq!(c.currency_effect, Some(D::from(40)));
    assert_eq!(c.reconciliation, Some(D::ZERO));
    assert!((c.performance.twr.unwrap() - 0.54).abs() < 1e-10);
}
#[test]
fn flow_date_fx_is_used_and_a_conversion_is_not_an_external_deposit() {
    let mut p = Portfolio::example();
    p.positions.clear();
    p.instruments.clear();
    p.settings.reporting_currency = Some("CAD".into());
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        D::from(100),
    ));
    let mut deposit =
        CashMovement::funding("add", "account", "cycle", "USD", "2026-01-05", D::from(20));
    deposit.kind = CashKind::Deposit;
    p.cash_movements.push(deposit);
    rate(&mut p, "fx1", "2026-01-01", 1, 0);
    rate(&mut p, "fx2", "2026-01-05", 15, 1);
    rate(&mut p, "fx3", "2026-01-09", 2, 0);
    let c = report(&p, &ReportFilter::at("2026-01-09"))
        .unwrap()
        .consolidated
        .unwrap();
    assert_eq!(c.equity, Some(D::from(240)));
    assert_eq!(c.history.last().unwrap().capital, Some(D::from(130)));
    assert_eq!(c.performance.investment_gain, Some(D::from(110)));
    p.cash_movements.pop();
    let mut fx = CashMovement::funding(
        "convert",
        "account",
        "cycle",
        "USD",
        "2026-01-05",
        D::from(50),
    );
    fx.kind = CashKind::Fx;
    fx.receive_currency = Some("CAD".into());
    fx.receive_amount = Some(D::from(75));
    p.cash_movements.push(fx);
    let c = report(&p, &ReportFilter::at("2026-01-09"))
        .unwrap()
        .consolidated
        .unwrap();
    assert_eq!(c.equity, Some(D::from(175)));
    assert_eq!(c.history.last().unwrap().capital, Some(D::from(100)));
    assert_eq!(c.currency_effect, Some(D::from(75)));
}
#[test]
fn missing_flow_fx_withholds_return_even_when_current_equity_is_known() {
    let mut p = Portfolio::example();
    p.positions.clear();
    p.instruments.clear();
    p.settings.reporting_currency = Some("CAD".into());
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "CAD",
        "2026-01-01",
        D::from(100),
    ));
    let mut deposit =
        CashMovement::funding("add", "account", "cycle", "USD", "2026-01-05", D::from(20));
    deposit.kind = CashKind::Deposit;
    p.cash_movements.push(deposit);
    rate(&mut p, "fx", "2026-01-09", 2, 0);
    let c = report(&p, &ReportFilter::at("2026-01-09"))
        .unwrap()
        .consolidated
        .unwrap();
    assert_eq!(c.equity, Some(D::from(140)));
    assert!(c.performance.investment_gain.is_none());
    assert!(c.performance.twr.is_none() && c.performance.xirr.is_none());
    assert!(c.history.last().unwrap().capital.is_none());
}
