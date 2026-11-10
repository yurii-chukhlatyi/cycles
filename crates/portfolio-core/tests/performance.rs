use portfolio_core::*;
use rust_decimal::Decimal as D;
fn snap(id: &str, date: &str, equity: i64, phase: &str, source: &str) -> Snapshot {
    Snapshot {
        id: id.into(),
        date: date.into(),
        cycle_id: None,
        account_id: None,
        currency: "USD".into(),
        equity: D::from(equity),
        phase: phase.into(),
        source: source.into(),
        input_head: "test".into(),
        quote_ids: vec![],
    }
}
#[test]
fn contributions_are_not_profit_and_exact_flow_boundaries_chain() {
    let mut p = Portfolio::example();
    p.positions.clear();
    p.instruments.clear();
    p.cash_movements.push(CashMovement::funding(
        "initial",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        D::from(100000),
    ));
    let mut add = CashMovement::funding(
        "deposit",
        "account",
        "cycle",
        "USD",
        "2026-06-01",
        D::from(20000),
    );
    add.kind = CashKind::Deposit;
    p.cash_movements.push(add);
    p.snapshots = vec![
        snap("start", "2026-01-01", 100000, "baseline", "manual"),
        snap("before", "2026-06-01", 104000, "pre_flow", "deposit"),
        snap("after", "2026-06-01", 124000, "post_flow", "deposit"),
        snap("end", "2026-11-09", 125000, "close", "manual"),
    ];
    let r = report(&p, &ReportFilter::at("2026-11-09")).unwrap();
    let v = r.performance.iter().find(|x| x.currency == "USD").unwrap();
    assert_eq!(v.investment_gain, Some(D::from(5000)));
    let expected = 1.04 * (125000.0 / 124000.0) - 1.0;
    assert!((v.twr.unwrap() - expected).abs() < 1e-10);
    assert!(v.twr_method.starts_with("Exact"));
    assert_eq!(
        r.totals
            .iter()
            .find(|x| x.currency == "USD")
            .unwrap()
            .external_contributions,
        D::from(120000)
    );
    assert!(v.volatility.is_none());
}
#[test]
fn imported_baseline_prevents_invented_earlier_performance() {
    let mut p = Portfolio::example();
    p.positions.clear();
    p.instruments.clear();
    let mut b = CashMovement::funding(
        "baseline",
        "account",
        "cycle",
        "USD",
        "2026-11-08",
        D::from(10000),
    );
    b.kind = CashKind::Baseline;
    p.cash_movements.push(b);
    p.snapshots = vec![
        snap("earlier", "2026-01-01", 1000, "close", "manual"),
        snap("baseline", "2026-11-08", 10000, "baseline", "import"),
    ];
    let r = report(&p, &ReportFilter::at("2026-11-08")).unwrap();
    let h: Vec<_> = r.history.iter().filter(|h| h.currency == "USD").collect();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].date, "2026-11-08");
    let v = r.performance.iter().find(|v| v.currency == "USD").unwrap();
    assert!(v.twr.is_none() && v.xirr.is_none() && v.volatility.is_none());
}
#[test]
fn assignment_stays_in_winding_down_cycle_and_conserves_delivery() {
    let mut p = Portfolio::example();
    p.instruments[0].asset = Asset::Option;
    p.instruments[0].multiplier = D::from(100);
    p.instruments[0].option_right = Some(OptionRight::Put);
    p.instruments[0].strike = Some(D::from(50));
    p.instruments[0].expiry = Some("2026-11-20".into());
    p.instruments[0].underlying = Some("TEST".into());
    p.positions[0].direction = Direction::Short;
    p.cycles[0].state = CycleState::WindingDown;
    p.cycles[0].stopped_on = Some("2026-06-01".into());
    p.transactions.push(Transaction::trade(
        "open",
        "pos",
        "2026-01-02",
        1,
        Action::Open,
        D::ONE,
        D::from(2),
        D::ZERO,
    ));
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        D::from(10000),
    ));
    let next = settle(
        &p,
        &SettlementRequest {
            position_id: "pos".into(),
            date: "2026-11-09".into(),
            quantity: D::ONE,
            fee: D::ZERO,
            cash_price: None,
            underlying_position_id: None,
            order: 2,
        },
    )
    .unwrap();
    assert_eq!(next.positions.len(), 2);
    let stock = &next.positions[1];
    assert_eq!(stock.cycle_id, "cycle");
    assert_eq!(
        position_ledger(&next, &stock.id, None).unwrap().quantity,
        D::from(100)
    );
    assert_eq!(
        cash_balance(&next, "account", "cycle", "USD", "2026-11-09").unwrap(),
        D::from(5200)
    );
    assert_eq!(
        position_ledger(&next, "pos", None).unwrap().realized,
        D::from(200)
    );
}

#[test]
fn a_deposit_does_not_create_volatility_and_missing_trading_dates_are_not_bridged() {
    use chrono::Datelike;
    let mut p = Portfolio::example();
    p.transactions.push(Transaction::trade(
        "buy",
        "pos",
        "2026-01-02",
        1,
        Action::Open,
        D::from(100),
        D::from(100),
        D::ZERO,
    ));
    p.cash_movements.push(CashMovement::funding(
        "fund",
        "account",
        "cycle",
        "USD",
        "2026-01-01",
        D::from(10000),
    ));
    let mut movement = CashMovement::funding(
        "add",
        "account",
        "cycle",
        "USD",
        "2026-01-20",
        D::from(5000),
    );
    movement.kind = CashKind::Deposit;
    p.cash_movements.push(movement);
    let mut day = date("2026-01-02").unwrap();
    while day <= date("2026-02-20").unwrap() {
        if !matches!(day.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
            p.market_data.push(MarketData::quote(
                &day.to_string(),
                "stock",
                &day.to_string(),
                D::from(100),
            ));
        }
        day = day.succ_opt().unwrap();
    }
    let f = ReportFilter::at("2026-02-20");
    let r = report(&p, &f).unwrap();
    let full = r.performance.iter().find(|v| v.currency == "USD").unwrap();
    assert_eq!(full.volatility, Some(0.0));
    assert_eq!(full.twr, Some(0.0));
    let observations = full.observations;
    p.market_data.retain(|q| q.date != "2026-02-10");
    let r = report(&p, &f).unwrap();
    let gapped = r.performance.iter().find(|v| v.currency == "USD").unwrap();
    assert!(gapped.twr.is_none());
    assert!(gapped.missing_periods > 0);
    assert_eq!(gapped.observations, observations - 2);
}

#[test]
fn option_settlement_matrix_conserves_cash_and_adjusted_contract_delivery() {
    for direction in [Direction::Long, Direction::Short] {
        for right in [OptionRight::Call, OptionRight::Put] {
            for multiplier in [50, 100] {
                for cash_settled in [false, true] {
                    let mut p = Portfolio::example();
                    let i = &mut p.instruments[0];
                    i.asset = Asset::Option;
                    i.multiplier = D::from(multiplier);
                    i.option_right = Some(right.clone());
                    i.strike = Some(D::from(100));
                    i.expiry = Some("2026-12-18".into());
                    i.underlying = Some("TEST".into());
                    i.settlement = if cash_settled {
                        Settlement::Cash
                    } else {
                        Settlement::Physical
                    };
                    p.positions[0].direction = direction.clone();
                    p.transactions.push(Transaction::trade(
                        "open",
                        "pos",
                        "2026-01-02",
                        1,
                        Action::Open,
                        D::from(2),
                        D::from(2),
                        D::ONE,
                    ));
                    p.cash_movements.push(CashMovement::funding(
                        "initial",
                        "account",
                        "cycle",
                        "USD",
                        "2026-01-01",
                        D::from(100000),
                    ));
                    let before = cash_balance(&p, "account", "cycle", "USD", "2026-11-09").unwrap();
                    let next = settle(
                        &p,
                        &SettlementRequest {
                            position_id: "pos".into(),
                            date: "2026-11-09".into(),
                            quantity: D::ONE,
                            fee: D::from(3),
                            cash_price: cash_settled.then_some(D::from(4)),
                            underlying_position_id: None,
                            order: 2,
                        },
                    )
                    .unwrap();
                    let delivery_sign =
                        if (right == OptionRight::Call) == (direction == Direction::Long) {
                            D::ONE
                        } else {
                            -D::ONE
                        };
                    let expected = if cash_settled {
                        before + direction.sign() * D::from(4 * multiplier) - D::from(3)
                    } else {
                        before - delivery_sign * D::from(100 * multiplier) - D::from(3)
                    };
                    assert_eq!(
                        cash_balance(&next, "account", "cycle", "USD", "2026-11-09").unwrap(),
                        expected
                    );
                    assert_eq!(
                        position_ledger(&next, "pos", None).unwrap().quantity,
                        D::ONE
                    );
                    if cash_settled {
                        assert_eq!(next.positions.len(), 1)
                    } else {
                        assert_eq!(next.positions.len(), 2);
                        assert_eq!(next.positions[1].cycle_id, "cycle");
                        assert_eq!(next.positions[1].direction.sign(), delivery_sign);
                        assert_eq!(
                            position_ledger(&next, &next.positions[1].id, None)
                                .unwrap()
                                .quantity,
                            D::from(multiplier)
                        );
                    }
                }
            }
        }
    }
}
