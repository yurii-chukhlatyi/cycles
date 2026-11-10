use crate::{Change, Error, Result};
use portfolio_core::{new_id, MarketData};
use std::path::Path;
pub fn market_csv(path: &Path) -> Result<Vec<Change>> {
    if std::fs::metadata(path)?.len() > 10 * 1024 * 1024 {
        return Err(Error::Corrupt("Quote CSV exceeds 10 MB".into()));
    }
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)
        .map_err(|e| Error::Corrupt(e.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|e| Error::Corrupt(e.to_string()))?
        .clone();
    for name in ["instrument_id", "date", "price"] {
        if !headers.iter().any(|h| h == name) {
            return Err(Error::Corrupt(format!("Missing CSV column {name}")));
        }
    }
    let mut changes = vec![];
    for (row, record) in reader.records().enumerate() {
        let record = record.map_err(|e| Error::Corrupt(e.to_string()))?;
        let get = |name: &str| {
            headers
                .iter()
                .position(|h| h == name)
                .and_then(|i| record.get(i))
                .unwrap_or("")
        };
        let mut quote = MarketData::quote(
            &new_id(),
            get("instrument_id"),
            get("date"),
            get("price")
                .parse()
                .map_err(|_| Error::Corrupt(format!("Invalid price on CSV row {}", row + 2)))?,
        );
        quote.delta = if get("delta").is_empty() {
            None
        } else {
            Some(
                get("delta")
                    .parse()
                    .map_err(|_| Error::Corrupt(format!("Invalid delta on CSV row {}", row + 2)))?,
            )
        };
        quote.underlying_price = if get("underlying_price").is_empty() {
            None
        } else {
            Some(get("underlying_price").parse().map_err(|_| {
                Error::Corrupt(format!("Invalid underlying price on CSV row {}", row + 2))
            })?)
        };
        quote.adjusted = match get("adjusted") {
            "true" | "1" => true,
            "false" | "0" | "" => false,
            _ => {
                return Err(Error::Corrupt(format!(
                    "Invalid adjusted flag on CSV row {}",
                    row + 2
                )))
            }
        };
        quote.source = get("source").into();
        if quote.source.is_empty() {
            quote.source = "CSV import".into()
        };
        changes.push(Change::upsert("MarketData", serde_json::to_value(quote)?));
        if changes.len() > 10000 {
            return Err(Error::Corrupt(
                "Import at most 10,000 quotes per command".into(),
            ));
        }
    }
    Ok(changes)
}
