//! Explicitly opted-in integration against an owner-authorized disposable sheet.
use super::*;
#[tokio::test]
#[ignore = "Requires an owner Google Desktop client connected in Cycles and explicit CYCLES_RUN_GOOGLE_LIVE=1"]
async fn google_live_atomic_groups_concurrent_successors_and_xlsx_parity() {
    assert_eq!(
        std::env::var("CYCLES_RUN_GOOGLE_LIVE").as_deref(),
        Ok("1"),
        "Run pnpm test:google-live with explicit opt-in"
    );
    let client = google().await.expect(
        "Connect Google in Cycles first; credentials are read only from the OS credential store",
    );
    let mut p = store::sample_portfolio();
    p.cycles
        .push(Cycle::new("live-next", "Next strategy", "2026-11-09"));
    p.instruments.push(Instrument {
        id: "live-option".into(),
        symbol: "ACME 2026-12-18 Call".into(),
        asset: Asset::Option,
        currency: "USD".into(),
        multiplier: Money::from(100),
        option_right: Some(OptionRight::Call),
        strike: Some(Money::from(120)),
        expiry: Some("2026-12-18".into()),
        underlying: Some("ACME".into()),
        settlement: Settlement::Physical,
        deliverable: "100 ACME shares".into(),
    });
    let mut option = p.positions[0].clone();
    option.id = "live-option-pos".into();
    option.instrument_id = "live-option".into();
    p.positions.push(option);
    p.transactions.push(Transaction::trade(
        "live-option-open",
        "live-option-pos",
        "2026-11-09",
        1,
        Action::Open,
        Money::ONE,
        Money::from(2),
        Money::ONE,
    ));
    p.market_data.push(MarketData::quote(
        "live-option-quote",
        "live-option",
        "2026-11-09",
        Money::from(2),
    ));
    let journal = store::Journal::from_portfolio(&p).unwrap();
    let title = format!("Cycles disposable integration {}", new_id());
    let mut doc = client.create(&title, &journal).await.unwrap();
    eprintln!("Disposable integration spreadsheet: {} (contains sample data only; remove it from Drive after inspection)",doc.id);
    let state = doc.journal.replay().unwrap();
    let mut movement = CashMovement::funding(
        "live-transfer",
        "account",
        "cycle",
        "USD",
        "2026-11-09",
        Money::from(1000),
    );
    movement.kind = CashKind::CycleTransfer;
    movement.order = 10;
    movement.destination_cycle_id = Some("live-next".into());
    let command = store::prepare(
        &state,
        vec![store::Change::upsert(
            "CashMovements",
            serde_json::to_value(movement).unwrap(),
        )],
        &new_id(),
        "live-test",
    )
    .unwrap();
    doc = client.append(&doc, &command).await.unwrap();
    doc = client.append(&doc, &command).await.unwrap();
    assert_eq!(
        doc.journal
            .replay()
            .unwrap()
            .portfolio
            .cash_movements
            .iter()
            .filter(|m| m.id == "live-transfer")
            .count(),
        1
    );
    let state = doc.journal.replay().unwrap();
    let stopped = store::change_cycle(
        &state.portfolio,
        "cycle",
        CycleState::WindingDown,
        "2026-11-09",
        &state.head,
    )
    .unwrap();
    let settled = settle(
        &stopped,
        &SettlementRequest {
            position_id: "live-option-pos".into(),
            date: "2026-11-09".into(),
            quantity: Money::ONE,
            fee: Money::ONE,
            cash_price: None,
            underlying_position_id: Some("pos".into()),
            order: 20,
        },
    )
    .unwrap();
    let command = store::prepare(
        &state,
        store::changes_between(&state.portfolio, &settled).unwrap(),
        &new_id(),
        "live-test",
    )
    .unwrap();
    doc = client.append(&doc, &command).await.unwrap();
    assert_eq!(
        position_ledger(
            &doc.journal.replay().unwrap().portfolio,
            "live-option-pos",
            None
        )
        .unwrap()
        .quantity,
        Money::ZERO
    );
    let state = doc.journal.replay().unwrap();
    let make = |id: &str| {
        let movement = CashMovement::funding(
            id,
            "account",
            "live-next",
            "USD",
            "2026-11-09",
            Money::from(10),
        );
        store::prepare(
            &state,
            vec![store::Change::upsert(
                "CashMovements",
                serde_json::to_value(movement).unwrap(),
            )],
            &new_id(),
            id,
        )
        .unwrap()
    };
    let a = make("concurrent-a");
    let b = make("concurrent-b");
    let other = client.clone();
    let (ra, rb) = tokio::join!(client.append(&doc, &a), other.append(&doc, &b));
    assert!(ra.is_ok() || rb.is_ok());
    let loaded = client.load(&doc.id).await.unwrap();
    let accepted = loaded.journal.replay().unwrap();
    assert_eq!(
        accepted
            .accepted
            .iter()
            .filter(|id| **id == a.commit.command_id || **id == b.commit.command_id)
            .count(),
        1
    );
    let tmp = tempfile::tempdir().unwrap();
    let backup = tmp.path().join("google-backup.xlsx");
    store::export_backup(&backup, &loaded.journal).unwrap();
    let restored = store::load_local(&backup)
        .unwrap()
        .journal
        .replay()
        .unwrap();
    assert_eq!(accepted.head, restored.head);
    assert_eq!(
        serde_json::to_value(&accepted.portfolio).unwrap(),
        serde_json::to_value(&restored.portfolio).unwrap()
    );
    let f = ReportFilter::at("2026-11-09");
    assert_eq!(
        serde_json::to_value(report(&accepted.portfolio, &f).unwrap()).unwrap(),
        serde_json::to_value(report(&restored.portfolio, &f).unwrap()).unwrap()
    );
}
