use crate::{Error, Result};
use portfolio_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
pub const TABLES: &[&str] = &[
    "Settings",
    "Accounts",
    "Cycles",
    "Instruments",
    "Positions",
    "Transactions",
    "CashMovements",
    "Reservations",
    "MarketData",
    "FxRates",
    "ValuationInputs",
];
pub fn hash<T: Serialize>(value: &T) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub schema_version: u32,
    pub dataset_id: String,
    pub created_at: String,
    pub policy_version: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordRevision {
    pub table: String,
    pub record_id: String,
    pub version: u64,
    pub command_id: String,
    pub payload: Option<Value>,
    pub hash: String,
}
impl RecordRevision {
    fn computed_hash(&self) -> Result<String> {
        hash(&(
            &self.table,
            &self.record_id,
            self.version,
            &self.command_id,
            &self.payload,
        ))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub command_id: String,
    pub parent: String,
    pub records: Vec<String>,
    pub recorded_at: String,
    pub writer_id: String,
    pub hash: String,
}
impl Commit {
    fn computed_hash(&self) -> Result<String> {
        hash(&(
            &self.command_id,
            &self.parent,
            &self.records,
            &self.recorded_at,
            &self.writer_id,
        ))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prepared {
    pub records: Vec<RecordRevision>,
    pub commit: Commit,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub table: String,
    pub id: String,
    pub value: Option<Value>,
}
impl Change {
    pub fn upsert(table: &str, value: Value) -> Self {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(if table == "Settings" { "settings" } else { "" })
            .to_string();
        Self {
            table: table.into(),
            id,
            value: Some(value),
        }
    }
    pub fn delete(table: &str, id: &str) -> Self {
        Self {
            table: table.into(),
            id: id.into(),
            value: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Journal {
    pub meta: Meta,
    pub records: Vec<RecordRevision>,
    pub commits: Vec<Commit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub command_id: String,
    pub reason: String,
}
#[derive(Debug, Clone)]
pub struct Replayed {
    pub portfolio: Portfolio,
    pub head: String,
    pub revision: u64,
    pub accepted: Vec<String>,
    pub conflicts: Vec<Conflict>,
    pub(crate) current: BTreeMap<(String, String), RecordRevision>,
}
fn portfolio(meta: &Meta, rows: &BTreeMap<(String, String), RecordRevision>) -> Result<Portfolio> {
    let mut p = Portfolio {
        schema_version: meta.schema_version,
        dataset_id: meta.dataset_id.clone(),
        created_at: meta.created_at.clone(),
        policy_version: meta.policy_version,
        ..Portfolio::default()
    };
    for r in rows.values() {
        let Some(v) = r.payload.clone() else { continue };
        match r.table.as_str() {
            "Settings" => p.settings = serde_json::from_value(v)?,
            "Accounts" => p.accounts.push(serde_json::from_value(v)?),
            "Cycles" => p.cycles.push(serde_json::from_value(v)?),
            "Instruments" => p.instruments.push(serde_json::from_value(v)?),
            "Positions" => p.positions.push(serde_json::from_value(v)?),
            "Transactions" => p.transactions.push(serde_json::from_value(v)?),
            "CashMovements" => p.cash_movements.push(serde_json::from_value(v)?),
            "Reservations" => p.reservations.push(serde_json::from_value(v)?),
            "MarketData" => p.market_data.push(serde_json::from_value(v)?),
            "FxRates" => p.fx_rates.push(serde_json::from_value(v)?),
            "ValuationInputs" => p.snapshots.push(serde_json::from_value(v)?),
            _ => return Err(Error::Corrupt("Unknown canonical table".into())),
        }
    }
    Ok(p)
}
pub fn changes_between(before: &Portfolio, after: &Portfolio) -> Result<Vec<Change>> {
    let a = records_from_portfolio(before)?;
    let b = records_from_portfolio(after)?;
    let mut changes = vec![];
    for (key, v) in &b {
        if a.get(key) != Some(v) {
            changes.push(Change {
                table: key.0.clone(),
                id: key.1.clone(),
                value: Some(v.clone()),
            });
        }
    }
    for key in a.keys() {
        if !b.contains_key(key) {
            changes.push(Change::delete(&key.0, &key.1));
        }
    }
    Ok(changes)
}
fn records_from_portfolio(p: &Portfolio) -> Result<BTreeMap<(String, String), Value>> {
    let mut out = BTreeMap::new();
    out.insert(
        ("Settings".into(), "settings".into()),
        serde_json::to_value(&p.settings)?,
    );
    macro_rules! rows {
        ($field:ident,$table:literal) => {
            for x in &p.$field {
                out.insert(($table.into(), x.id.clone()), serde_json::to_value(x)?);
            }
        };
    }
    rows!(accounts, "Accounts");
    rows!(cycles, "Cycles");
    rows!(instruments, "Instruments");
    rows!(positions, "Positions");
    rows!(transactions, "Transactions");
    rows!(cash_movements, "CashMovements");
    rows!(reservations, "Reservations");
    rows!(market_data, "MarketData");
    rows!(fx_rates, "FxRates");
    rows!(snapshots, "ValuationInputs");
    Ok(out)
}
impl Replayed {
    pub fn current_value(&self, table: &str, id: &str) -> Option<&Value> {
        self.current
            .get(&(table.into(), id.into()))
            .and_then(|r| r.payload.as_ref())
    }
}
impl Journal {
    pub fn empty() -> Self {
        let p = Portfolio::default();
        Self {
            meta: Meta {
                schema_version: p.schema_version,
                dataset_id: p.dataset_id,
                created_at: p.created_at,
                policy_version: p.policy_version,
            },
            records: vec![],
            commits: vec![],
        }
    }
    pub fn from_portfolio(p: &Portfolio) -> Result<Self> {
        validate(p)?;
        let mut j = Self {
            meta: Meta {
                schema_version: p.schema_version,
                dataset_id: p.dataset_id.clone(),
                created_at: p.created_at.clone(),
                policy_version: p.policy_version,
            },
            records: vec![],
            commits: vec![],
        };
        let state = j.replay()?;
        let changes = records_from_portfolio(p)?
            .into_iter()
            .map(|((table, id), v)| Change {
                table,
                id,
                value: Some(v),
            })
            .collect();
        let command = prepare(&state, changes, &new_id(), "initialization")?;
        j.append(command);
        j.replay()?;
        Ok(j)
    }
    pub fn append(&mut self, command: Prepared) {
        self.records.extend(command.records);
        self.commits.push(command.commit);
    }
    pub fn replay(&self) -> Result<Replayed> {
        if self.meta.schema_version != 1 || self.meta.policy_version != 1 {
            return Err(Error::Corrupt(
                "Unsupported workbook schema or accounting policy".into(),
            ));
        }
        let mut head = hash(&self.meta)?;
        let mut current = BTreeMap::new();
        let mut revision = 0;
        let mut accepted = vec![];
        let mut conflicts = vec![];
        let mut rows = HashMap::new();
        for row in &self.records {
            if !TABLES.contains(&row.table.as_str())
                || row.record_id.is_empty()
                || row.hash != row.computed_hash()?
            {
                return Err(Error::Corrupt(
                    "A raw spreadsheet record was changed or is invalid".into(),
                ));
            }
            if let Some(v) = &row.payload {
                if row.table != "Settings"
                    && v.get("id").and_then(Value::as_str) != Some(row.record_id.as_str())
                {
                    return Err(Error::Corrupt(
                        "Record ID does not match its raw payload".into(),
                    ));
                }
            }
            if let Some(old) = rows.insert(row.hash.clone(), row) {
                if serde_json::to_value(old)? != serde_json::to_value(row)? {
                    return Err(Error::Corrupt("Conflicting duplicate raw record".into()));
                }
            }
        }
        let mut commands = HashMap::new();
        let mut state = portfolio(&self.meta, &current)?;
        for c in &self.commits {
            if c.hash != c.computed_hash()? {
                return Err(Error::Corrupt("Commit journal was changed".into()));
            }
            if let Some(old) = commands.insert(c.command_id.clone(), c.hash.clone()) {
                if old != c.hash {
                    return Err(Error::Corrupt(
                        "Command ID reused with different content".into(),
                    ));
                }
                continue;
            }
            if c.parent != head {
                conflicts.push(Conflict {
                    command_id: c.command_id.clone(),
                    reason: "Source changed before this operation was accepted".into(),
                });
                continue;
            }
            let mut next = current.clone();
            let mut touched = HashSet::new();
            for id in &c.records {
                let r = rows
                    .get(id)
                    .ok_or_else(|| Error::Corrupt("Commit references a missing row".into()))?;
                if r.command_id != c.command_id || !touched.insert((&r.table, &r.record_id)) {
                    return Err(Error::Corrupt(
                        "Commit references an invalid record group".into(),
                    ));
                }
                let key = (r.table.clone(), r.record_id.clone());
                let expected = next.get(&key).map_or(1, |x: &RecordRevision| x.version + 1);
                if r.version != expected {
                    return Err(Error::Corrupt("Record revision is out of order".into()));
                }
                next.insert(key, (*r).clone());
            }
            let candidate = portfolio(&self.meta, &next)?;
            validate_transition(&state, &candidate).map_err(|e| {
                Error::Corrupt(format!("Committed financial records are invalid: {e}"))
            })?;
            state = candidate;
            current = next;
            head = c.hash.clone();
            revision += 1;
            accepted.push(c.command_id.clone());
        }
        Ok(Replayed {
            portfolio: state,
            head,
            revision,
            accepted,
            conflicts,
            current,
        })
    }
}
pub fn prepare(
    state: &Replayed,
    changes: Vec<Change>,
    command_id: &str,
    writer_id: &str,
) -> Result<Prepared> {
    if command_id.is_empty() {
        return Err(Error::Corrupt("Command identifier is missing".into()));
    }
    let mut rows = vec![];
    let mut next = state.current.clone();
    let mut seen = HashSet::new();
    for change in changes {
        if !TABLES.contains(&change.table.as_str())
            || change.id.is_empty()
            || !seen.insert((change.table.clone(), change.id.clone()))
        {
            return Err(Error::Corrupt(
                "Invalid table, identifier or duplicate edit".into(),
            ));
        }
        let key = (change.table.clone(), change.id.clone());
        let version = next.get(&key).map_or(1, |x| x.version + 1);
        let mut r = RecordRevision {
            table: change.table,
            record_id: change.id,
            version,
            command_id: command_id.into(),
            payload: change.value,
            hash: String::new(),
        };
        r.hash = r.computed_hash()?;
        next.insert(key, r.clone());
        rows.push(r);
    }
    let candidate = portfolio(
        &Meta {
            schema_version: state.portfolio.schema_version,
            dataset_id: state.portfolio.dataset_id.clone(),
            created_at: state.portfolio.created_at.clone(),
            policy_version: state.portfolio.policy_version,
        },
        &next,
    )?;
    validate_transition(&state.portfolio, &candidate)?;
    let mut commit = Commit {
        command_id: command_id.into(),
        parent: state.head.clone(),
        records: rows.iter().map(|r| r.hash.clone()).collect(),
        recorded_at: chrono::Utc::now().to_rfc3339(),
        writer_id: writer_id.into(),
        hash: String::new(),
    };
    commit.hash = commit.computed_hash()?;
    Ok(Prepared {
        records: rows,
        commit,
    })
}
pub const RECORD_HEADERS: &[&str] = &[
    "record_id",
    "record_version",
    "command_id",
    "row_hash",
    "record_json",
    "label",
    "date",
    "cycle_id",
    "account_id",
    "position_id",
    "symbol",
    "currency",
    "quantity",
    "price",
    "amount",
];
pub const COMMIT_HEADERS: &[&str] = &[
    "command_id",
    "expected_parent_hash",
    "record_hashes_json",
    "recorded_at",
    "writer_id",
    "commit_hash",
];
pub fn record_cells(r: &RecordRevision) -> Result<Vec<String>> {
    let mut row = vec![
        r.record_id.clone(),
        r.version.to_string(),
        r.command_id.clone(),
        r.hash.clone(),
        serde_json::to_string(&r.payload)?,
    ];
    let v = r.payload.as_ref().unwrap_or(&Value::Null);
    for field in &RECORD_HEADERS[5..] {
        row.push(
            v.get(*field)
                .map(|v| {
                    v.as_str()
                        .map(String::from)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_default(),
        );
    }
    Ok(row)
}
pub fn parse_record(table: &str, row: &[String]) -> Result<RecordRevision> {
    if row.len() < 5 {
        return Err(Error::Corrupt("Truncated raw record".into()));
    }
    let r = RecordRevision {
        table: table.into(),
        record_id: row[0].clone(),
        version: row[1]
            .parse()
            .map_err(|_| Error::Corrupt("Invalid record version".into()))?,
        command_id: row[2].clone(),
        hash: row[3].clone(),
        payload: serde_json::from_str(&row[4])?,
    };
    let expected = record_cells(&r)?;
    for (i, cell) in expected.iter().enumerate() {
        if row.get(i).map(String::as_str).unwrap_or("") != cell {
            return Err(Error::Corrupt(format!(
                "A display/input cell in {table} was edited outside the app"
            )));
        }
    }
    Ok(r)
}
pub fn commit_cells(c: &Commit) -> Result<Vec<String>> {
    Ok(vec![
        c.command_id.clone(),
        c.parent.clone(),
        serde_json::to_string(&c.records)?,
        c.recorded_at.clone(),
        c.writer_id.clone(),
        c.hash.clone(),
    ])
}
pub fn parse_commit(row: &[String]) -> Result<Commit> {
    if row.len() < 6 {
        return Err(Error::Corrupt("Truncated commit row".into()));
    }
    Ok(Commit {
        command_id: row[0].clone(),
        parent: row[1].clone(),
        records: serde_json::from_str(&row[2])?,
        recorded_at: row[3].clone(),
        writer_id: row[4].clone(),
        hash: row[5].clone(),
    })
}
pub fn metadata_rows(j: &Journal) -> Result<Vec<Vec<String>>> {
    Ok(vec![vec!["key".into(),"value".into()],vec!["format".into(),"portfolio-cycles".into()],vec!["meta_json".into(),serde_json::to_string(&j.meta)?],vec!["genesis_hash".into(),hash(&j.meta)?],vec!["instructions".into(),"App-managed raw records. No formulas. Data entry and correction belong in Cycles. record_json is canonical; other columns are checked inspection fields.".into()]])
}
pub fn parse_meta(rows: &[Vec<String>]) -> Result<Meta> {
    let values: BTreeMap<_, _> = rows
        .iter()
        .skip(1)
        .filter(|r| r.len() >= 2)
        .map(|r| (r[0].as_str(), r[1].as_str()))
        .collect();
    if values.get("format") != Some(&"portfolio-cycles") {
        return Err(Error::Corrupt(
            "This is not a Cycles workbook. Use Import tracker for a legacy file.".into(),
        ));
    }
    let meta: Meta = serde_json::from_str(
        values
            .get("meta_json")
            .ok_or_else(|| Error::Corrupt("Workbook metadata is missing".into()))?,
    )?;
    if values.get("genesis_hash").copied() != Some(hash(&meta)?.as_str()) {
        return Err(Error::Corrupt("Workbook metadata checksum changed".into()));
    }
    Ok(meta)
}
pub fn journal_tables(j: &Journal) -> Result<BTreeMap<String, Vec<Vec<String>>>> {
    let mut tables = BTreeMap::new();
    tables.insert("Meta".into(), metadata_rows(j)?);
    for table in TABLES {
        let mut rows = vec![RECORD_HEADERS.iter().map(|x| x.to_string()).collect()];
        for r in j.records.iter().filter(|r| r.table == *table) {
            rows.push(record_cells(r)?);
        }
        tables.insert(table.to_string(), rows);
    }
    let mut commits = vec![COMMIT_HEADERS.iter().map(|x| x.to_string()).collect()];
    for c in &j.commits {
        commits.push(commit_cells(c)?);
    }
    tables.insert("Commits".into(), commits);
    Ok(tables)
}
pub fn parse_tables(tables: &BTreeMap<String, Vec<Vec<String>>>) -> Result<Journal> {
    let meta = parse_meta(
        tables
            .get("Meta")
            .ok_or_else(|| Error::Corrupt("Meta sheet missing".into()))?,
    )?;
    let mut j = Journal {
        meta,
        records: vec![],
        commits: vec![],
    };
    for table in TABLES {
        let rows = tables
            .get(*table)
            .ok_or_else(|| Error::Corrupt(format!("{table} sheet missing")))?;
        let header = rows
            .first()
            .ok_or_else(|| Error::Corrupt("Missing headers".into()))?;
        if header
            .iter()
            .map(String::as_str)
            .ne(RECORD_HEADERS.iter().copied())
        {
            return Err(Error::Corrupt(format!("{table} headers changed")));
        }
        for row in rows
            .iter()
            .skip(1)
            .filter(|r| r.iter().any(|x| !x.is_empty()))
        {
            j.records.push(parse_record(table, row)?);
        }
    }
    let commits = tables
        .get("Commits")
        .ok_or_else(|| Error::Corrupt("Commits sheet missing".into()))?;
    if commits.first().is_none_or(|r| {
        r.iter()
            .map(String::as_str)
            .ne(COMMIT_HEADERS.iter().copied())
    }) {
        return Err(Error::Corrupt("Commit headers changed".into()));
    }
    for row in commits
        .iter()
        .skip(1)
        .filter(|r| r.iter().any(|x| !x.is_empty()))
    {
        j.commits.push(parse_commit(row)?);
    }
    j.replay()?;
    Ok(j)
}
pub fn snapshot_change(snapshot: &Snapshot) -> Result<Change> {
    Ok(Change::upsert("ValuationInputs", json!(snapshot)))
}
