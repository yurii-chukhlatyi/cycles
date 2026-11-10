use portfolio_storage::*;
use rust_xlsxwriter::Workbook;
#[test]
fn legacy_numeric_dates_and_option_price_units_are_preserved() {
    for epoch in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.xlsx");
        let mut w = Workbook::new();
        let s = w.add_worksheet();
        s.set_name("Positions").unwrap();
        s.write_string(3, 0, "Position ID").unwrap();
        for (col, text) in [
            (0, "legacy-option"),
            (1, "TEST"),
            (2, "Option"),
            (4, "Short"),
            (5, "Put"),
            (27, "Brokerage"),
            (28, "1234"),
            (29, "USD"),
            (30, "TEST PUT"),
        ] {
            s.write_string(4, col, text).unwrap();
        }
        s.write_number(4, 7, 100.0).unwrap();
        s.write_number(4, 8, 46374.0 - if epoch { 1462.0 } else { 0.0 })
            .unwrap();
        s.write_number(4, 12, 2.0).unwrap();
        s.write_number(4, 32, 1.0).unwrap();
        let t = w.add_worksheet();
        t.set_name("Transactions").unwrap();
        t.write_string(3, 0, "Transaction ID").unwrap();
        t.write_string(4, 0, "legacy-trade").unwrap();
        t.write_string(4, 1, "legacy-option").unwrap();
        t.write_number(4, 2, 46023.0 - if epoch { 1462.0 } else { 0.0 })
            .unwrap();
        t.write_string(4, 3, "OPEN").unwrap();
        t.write_number(4, 4, 1.0).unwrap();
        t.write_number(4, 5, 2.0).unwrap();
        w.save(&path).unwrap();
        let path = if epoch {
            use std::io::{Read, Write};
            let mut source = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
            let dest = dir.path().join("1904.xlsx");
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&dest).unwrap());
            for n in 0..source.len() {
                let mut entry = source.by_index(n).unwrap();
                let mut data = vec![];
                entry.read_to_end(&mut data).unwrap();
                if entry.name() == "xl/workbook.xml" {
                    data = String::from_utf8(data)
                        .unwrap()
                        .replace("<workbookPr ", "<workbookPr date1904=\"1\" ")
                        .into_bytes();
                }
                writer
                    .start_file(entry.name(), zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(&data).unwrap();
            }
            writer.finish().unwrap();
            dest
        } else {
            path
        };
        let preview = preview_legacy(&path, "2026-11-08").unwrap();
        assert_eq!(preview.portfolio.positions[0].id, "legacy-option");
        assert_eq!(preview.portfolio.transactions[0].price.to_string(), "2");
        assert_eq!(
            preview.portfolio.instruments[0].multiplier.to_string(),
            "100"
        );
        assert_eq!(preview.portfolio.transactions[0].date, "2026-01-01");
        assert_eq!(preview.portfolio.market_data[0].date, "2026-11-08");
        let choices = ImportChoices {
            position_cycles: preview
                .portfolio
                .positions
                .iter()
                .map(|p| (p.id.clone(), "import-cycle".into()))
                .collect(),
            cycles: preview.portfolio.cycles.clone(),
            cash: vec![OpeningCash {
                account_id: preview.portfolio.accounts[0].id.clone(),
                cycle_id: "import-cycle".into(),
                currency: "USD".into(),
                amount: rust_decimal::Decimal::from(50000),
            }],
            acknowledge_exclusions: false,
            reconcile_historical_settlements: false,
        };
        let imported =
            apply_legacy(&portfolio_core::Portfolio::default(), &preview, &choices).unwrap();
        assert!(imported.cash_movements[0].order <= 9_007_199_254_740_991);
        let r = portfolio_core::report(&imported, &portfolio_core::ReportFilter::at("2026-11-08"))
            .unwrap();
        assert_eq!(r.totals[0].cash, rust_decimal::Decimal::from(50000));
        assert!(r.performance[0].twr.is_none());
        assert!(r.history.iter().all(|h| h.date.as_str() >= "2026-11-08"));
        let invalid = dir.path().join("invalid-metadata.xlsx");
        for (col, invalid_value, message, restored) in [
            (29, " ", "currency is missing", "USD"),
            (4, "Unknown", "unknown long/short direction", "Short"),
            (5, "Unknown", "unknown option right", "Put"),
        ] {
            w.worksheet_from_name("Positions")
                .unwrap()
                .write_string(4, col, invalid_value)
                .unwrap();
            w.save(&invalid).unwrap();
            assert!(preview_legacy(&invalid, "2026-11-08")
                .unwrap_err()
                .to_string()
                .contains(message));
            w.worksheet_from_name("Positions")
                .unwrap()
                .write_string(4, col, restored)
                .unwrap();
        }
    }
}
