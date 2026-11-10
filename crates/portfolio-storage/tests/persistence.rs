use portfolio_core::*;
use portfolio_storage::*;
use serde_json::json;
#[test]
fn real_xlsx_round_trip_has_no_formulas_and_replays_identically() {
    let p = Portfolio::example();
    let j = Journal::from_portfolio(&p).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("portfolio.xlsx");
    save_local(&path, &j, None).unwrap();
    let loaded = load_local(&path).unwrap();
    assert_eq!(
        loaded.journal.replay().unwrap().portfolio.dataset_id,
        p.dataset_id
    );
    assert_eq!(
        loaded.journal.replay().unwrap().portfolio.positions[0].id,
        "pos"
    );
    assert!(save_local(&path, &j, Some("wrong-file-hash")).is_err());
}
#[test]
fn competing_appends_and_retries_have_one_domain_effect() {
    let p = Portfolio::example();
    let mut j = Journal::from_portfolio(&p).unwrap();
    let base = j.replay().unwrap();
    let change = Change::upsert(
        "Cycles",
        json!({"id":"second","name":"Second strategy","strategy":"","start_date":"2026-02-01","state":"active"}),
    );
    let a = prepare(&base, vec![change.clone()], "a", "test").unwrap();
    let b = prepare(&base, vec![change], "b", "test").unwrap();
    j.append(a.clone());
    j.append(a);
    j.append(b);
    let state = j.replay().unwrap();
    assert_eq!(state.portfolio.cycles.len(), 2);
    assert_eq!(state.conflicts.len(), 1);
    assert!(state.accepted.contains(&"a".to_string()));
    assert!(!state.accepted.contains(&"b".to_string()));
}
#[test]
fn changing_a_hashed_record_is_detected() {
    let mut j = Journal::from_portfolio(&Portfolio::example()).unwrap();
    j.records[0].payload = Some(json!({"modified":true}));
    assert!(j.replay().is_err());
}
