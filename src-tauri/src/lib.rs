use portfolio_core::*;
use portfolio_storage as store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Source {
    Sample,
    Local { path: String },
    Google { id: String },
}
#[derive(Clone, Serialize, Deserialize)]
struct Intent {
    source: Source,
    prepared: store::Prepared,
}
struct Session {
    journal: store::Journal,
    source: Source,
    staged: Option<store::Prepared>,
    replaces_pending: Option<String>,
    legacy: Option<store::LegacyPreview>,
    dir: PathBuf,
    initialized: bool,
    notice: Option<String>,
}
impl Session {
    fn new(dir: PathBuf) -> Self {
        Self {
            journal: store::Journal::empty(),
            source: Source::Sample,
            staged: None,
            replaces_pending: None,
            legacy: None,
            dir,
            initialized: false,
            notice: None,
        }
    }
    fn pending_dir(&self) -> PathBuf {
        self.dir.join("pending")
    }
    fn pending_path(&self, id: &str) -> Result<PathBuf> {
        uuid::Uuid::parse_str(id).map_err(err)?;
        Ok(self.pending_dir().join(format!("{id}.json")))
    }
    fn pending(&self) -> Vec<Value> {
        let Ok(entries) = fs::read_dir(self.pending_dir()) else {
            return vec![];
        };
        entries.filter_map(|e|{let p=e.ok()?.path();if p.extension()?.to_str()?!="json"{return None}let i:Intent=serde_json::from_slice(&fs::read(p).ok()?).ok()?;Some(json!({"id":i.prepared.commit.command_id,"source":i.source,"created_at":i.prepared.commit.recorded_at}))}).collect()
    }
    fn remember(&self) -> Result<()> {
        private_write(
            &self.dir.join("last-source.json"),
            &serde_json::to_vec(&self.source).map_err(err)?,
        )
    }
    fn view(&self, f: &ReportFilter) -> Result<Value> {
        let r = self.journal.replay().map_err(err)?;
        let report = report(&r.portfolio, f).map_err(err)?;
        Ok(
            json!({"portfolio":r.portfolio,"head":r.head,"revision":r.revision,"conflicts":r.conflicts,"source":self.source,"pending":self.pending(),"report":report,"consolidated":report.consolidated,"google_connected":credentials().is_ok(),"notice":self.notice}),
        )
    }
    fn stage(&mut self, p: store::Prepared, f: &ReportFilter) -> Result<Value> {
        let mut next = self.journal.clone();
        next.append(p.clone());
        let r = next.replay().map_err(err)?;
        let preview = report(&r.portfolio, f).map_err(err)?;
        let all = ReportFilter {
            as_of: f.as_of.clone(),
            ..Default::default()
        };
        let before = self.journal.replay().map_err(err)?;
        let cash_before = report(&before.portfolio, &all).map_err(err)?.cash_balances;
        let cash_after = report(&r.portfolio, &all).map_err(err)?.cash_balances;
        let prior_records:Vec<_>=p.records.iter().map(|x|json!({"table":x.table,"record_id":x.record_id,"payload":before.current_value(&x.table,&x.record_id)})).collect();
        self.staged = Some(p.clone());
        Ok(
            json!({"command_id":p.commit.command_id,"changes":p.records,"report":preview,"cash_before":cash_before,"cash_after":cash_after,"prior_records":prior_records}),
        )
    }
    async fn reload(&mut self) -> Result<()> {
        self.journal = load_source(&self.source, &self.journal).await?;
        Ok(())
    }
    fn check_head(&self, v: &Value) -> Result<String> {
        let head = self.journal.replay().map_err(err)?.head;
        if v["expected_head"].as_str() != Some(&head) {
            return Err(
                "Conflict: Source changed. Your draft is retained; refresh and preview it again."
                    .into(),
            );
        }
        Ok(head)
    }
    fn keep_intent(&self, intent: &Intent) -> Result<()> {
        let path = self.pending_path(&intent.prepared.commit.command_id)?;
        let bytes = serde_json::to_vec(intent).map_err(err)?;
        if path.exists() {
            if fs::read(&path).map_err(err)? != bytes {
                return Err(
                    "Pending command contents differ; do not retry with changed inputs".into(),
                );
            }
            return Ok(());
        }
        private_write(&path, &bytes)
    }
    async fn commit_intent(&mut self, intent: Intent) -> Result<()> {
        self.keep_intent(&intent)?;
        let mut current = load_source(&intent.source, &self.journal).await?;
        let replay = current.replay().map_err(err)?;
        let id = &intent.prepared.commit.command_id;
        if !replay.accepted.contains(id) {
            if replay.head != intent.prepared.commit.parent {
                return Err("Conflict: another command was saved first. This pending command has no effect. Discard it and preview the retained draft against the refreshed source.".into());
            }
            match &intent.source {
                Source::Local { path } => {
                    let doc = store::load_local(Path::new(path)).map_err(err)?;
                    current = doc.journal;
                    if current.replay().map_err(err)?.head != intent.prepared.commit.parent {
                        return Err("Conflict: workbook changed".into());
                    }
                    current.append(intent.prepared.clone());
                    store::save_local(Path::new(path), &current, Some(&doc.fingerprint))
                        .map_err(err)?;
                    current = store::load_local(Path::new(path)).map_err(err)?.journal;
                }
                Source::Google { id: sheet_id } => {
                    let client = google().await?;
                    let doc = client.load(sheet_id).await.map_err(err)?;
                    current = client
                        .append(&doc, &intent.prepared)
                        .await
                        .map_err(err)?
                        .journal;
                }
                Source::Sample => {
                    current.append(intent.prepared.clone());
                }
            }
        }
        if !current.replay().map_err(err)?.accepted.contains(id) {
            return Err("The source did not accept this command. It remains pending.".into());
        }
        if let Err(e) = fs::remove_file(self.pending_path(id)?) {
            self.notice = Some(format!(
                "The save is verified, but its recovery file could not be cleared: {e}"
            ));
        }
        if let Some(old) = self.replaces_pending.take() {
            let _ = fs::remove_file(self.pending_path(&old)?);
        }
        self.journal = current;
        self.source = intent.source;
        self.staged = None;
        if let Err(e) = self.remember() {
            self.notice = Some(format!(
                "Saved, but the startup source preference could not be stored: {e}"
            ));
        }
        Ok(())
    }
}
fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(err)?;
    }
    let temp = path.with_extension(format!("{}.tmp", new_id()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&temp).map_err(err)?;
    use std::io::Write;
    f.write_all(bytes).map_err(err)?;
    f.sync_all().map_err(err)?;
    drop(f);
    if path.exists() {
        fs::remove_file(path).map_err(err)?;
    }
    fs::rename(temp, path).map_err(err)
}
fn credential_entry() -> Result<keyring::Entry> {
    keyring::Entry::new("com.portfolio.cycles", "google-desktop").map_err(err)
}
fn credentials() -> Result<store::GoogleCredentials> {
    let raw = credential_entry()?
        .get_password()
        .map_err(|_| "Connect Google in Data & source first".to_string())?;
    serde_json::from_str(&raw).map_err(err)
}
fn save_credentials(c: &store::GoogleCredentials) -> Result<()> {
    credential_entry()?.set_password(&serde_json::to_string(c).map_err(err)?).map_err(|_|"Google signed in, but the OS credential store could not save the session. Unlock Keychain / Credential Manager and reconnect.".into())
}
async fn google() -> Result<store::GoogleSheets> {
    let mut c = credentials()?;
    store::refresh_credentials(&mut c).await.map_err(err)?;
    save_credentials(&c)?;
    store::GoogleSheets::new(c.access_token).map_err(err)
}
async fn load_source(source: &Source, fallback: &store::Journal) -> Result<store::Journal> {
    match source {
        Source::Sample => Ok(fallback.clone()),
        Source::Local { path } => Ok(store::load_local(Path::new(path)).map_err(err)?.journal),
        Source::Google { id } => Ok(google().await?.load(id).await.map_err(err)?.journal),
    }
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(err)
}
fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing {key}"))
}
fn filter(v: &Value) -> Result<ReportFilter> {
    if v["filter"].is_object() {
        decode(v["filter"].clone())
    } else {
        Ok(ReportFilter::at(&today()))
    }
}

#[tauri::command]
async fn api(
    action: String,
    payload: Value,
    state: State<'_, Mutex<Session>>,
    app: tauri::AppHandle,
) -> Result<Value> {
    // Browser OAuth must not hold the session lock while waiting for a person.
    if action == "authorize_google" {
        let config =
            store::ClientConfig::read(Path::new(required(&payload, "path")?)).map_err(err)?;
        let pending = store::begin_oauth(config).map_err(err)?;
        app.opener()
            .open_url(&pending.url, None::<&str>)
            .map_err(err)?;
        let c = store::finish_oauth(pending).await.map_err(err)?;
        save_credentials(&c)?;
        return Ok(json!({"connected":true}));
    }
    if action == "disconnect_google" {
        credential_entry()?.delete_credential().map_err(err)?;
        return Ok(json!({"connected":false}));
    }
    let mut s = state.lock().await;
    let f = filter(&payload)?;
    if !s.initialized {
        s.initialized = true;
        if let Ok(bytes) = fs::read(s.dir.join("last-source.json")) {
            if let Ok(source) = serde_json::from_slice::<Source>(&bytes) {
                if source != Source::Sample {
                    match load_source(&source, &s.journal).await {
                        Ok(j) => {
                            s.journal = j;
                            s.source = source;
                        }
                        Err(e) => {
                            s.notice = Some(format!("Previous source could not be opened: {e}"))
                        }
                    }
                }
            }
        }
    }
    match action.as_str() {
        "state" => {}
        "refresh" => {
            s.reload().await?;
            s.staged = None;
        }
        "sample" => {
            s.journal = store::Journal::from_portfolio(&store::sample_portfolio()).map_err(err)?;
            s.source = Source::Sample;
            s.staged = None;
            s.notice = None;
        }
        "new" => {
            let mut p = Portfolio::default();
            let start = required(&payload, "date")?;
            date(start).map_err(err)?;
            p.accounts.push(Account {
                id: new_id(),
                name: payload["account"].as_str().unwrap_or("Brokerage").into(),
                broker: String::new(),
                currencies: vec!["USD".into(), "CAD".into()],
            });
            p.cycles.push(Cycle::new(
                &new_id(),
                payload["cycle"].as_str().unwrap_or("First cycle"),
                start,
            ));
            s.journal = store::Journal::from_portfolio(&p).map_err(err)?;
            s.source = Source::Sample;
            s.staged = None;
            s.notice = None;
        }
        "open_local" => {
            let path = required(&payload, "path")?;
            let j = store::load_local(Path::new(path)).map_err(err)?.journal;
            s.journal = j;
            s.source = Source::Local { path: path.into() };
            s.staged = None;
            s.notice = None;
            s.remember()?;
        }
        "save_as" | "export" => {
            s.reload().await?;
            let path = required(&payload, "path")?;
            if Path::new(path)
                .extension()
                .and_then(|x| x.to_str())
                .is_none_or(|x| !x.eq_ignore_ascii_case("xlsx"))
            {
                return Err("Choose a .xlsx file".into());
            }
            store::export_backup(Path::new(path), &s.journal).map_err(err)?;
            if action == "save_as" {
                s.source = Source::Local { path: path.into() };
                s.staged = None;
                s.remember()?;
            }
        }
        "create_google" => {
            let doc = google()
                .await?
                .create(
                    payload["title"].as_str().unwrap_or("Cycles portfolio"),
                    &s.journal,
                )
                .await
                .map_err(err)?;
            s.source = Source::Google { id: doc.id };
            s.journal = doc.journal;
            s.staged = None;
            s.remember()?;
        }
        "open_google" => {
            let doc = google()
                .await?
                .load(required(&payload, "id")?)
                .await
                .map_err(err)?;
            s.source = Source::Google { id: doc.id };
            s.journal = doc.journal;
            s.staged = None;
            s.notice = None;
            s.remember()?;
        }
        "pending_preview" => {
            let id = required(&payload, "id")?;
            let intent: Intent =
                serde_json::from_slice(&fs::read(s.pending_path(id)?).map_err(err)?)
                    .map_err(err)?;
            let j = load_source(&intent.source, &s.journal).await?;
            let r = j.replay().map_err(err)?;
            if r.accepted.contains(&intent.prepared.commit.command_id) {
                return Err(
                    "This command was already saved. Retry it to clear the recovery file.".into(),
                );
            }
            let changes = intent
                .prepared
                .records
                .iter()
                .map(|row| store::Change {
                    table: row.table.clone(),
                    id: row.record_id.clone(),
                    value: row.payload.clone(),
                })
                .collect();
            let command = store::prepare(&r, changes, &new_id(), "cycles-recovery").map_err(err)?;
            s.journal = j;
            s.source = intent.source;
            s.replaces_pending = Some(id.into());
            return s.stage(command, &f);
        }
        "preview" => {
            s.replaces_pending = None;
            s.reload().await?;
            let head = s.check_head(&payload)?;
            let changes = decode(payload["changes"].clone())?;
            let command = store::stage(
                &s.journal,
                &head,
                payload["selected_cycle"].as_str(),
                changes,
            )
            .map_err(err)?;
            return s.stage(command, &f);
        }
        "settle_preview" => {
            s.replaces_pending = None;
            s.reload().await?;
            s.check_head(&payload)?;
            let p = s.journal.replay().map_err(err)?.portfolio;
            let request: SettlementRequest = decode(payload["request"].clone())?;
            if p.position(&request.position_id).map_err(err)?.cycle_id
                != required(&payload, "selected_cycle")?
            {
                return Err("The option belongs to another cycle".into());
            }
            let next = settle(&p, &request).map_err(err)?;
            let r = s.journal.replay().map_err(err)?;
            let command = store::prepare(
                &r,
                store::changes_between(&p, &next).map_err(err)?,
                &new_id(),
                "cycles-desktop",
            )
            .map_err(err)?;
            return s.stage(command, &f);
        }
        "cycle_preview" => {
            s.replaces_pending = None;
            s.reload().await?;
            let head = s.check_head(&payload)?;
            let r = s.journal.replay().map_err(err)?;
            let target = decode(payload["state"].clone())?;
            let next = store::change_cycle(
                &r.portfolio,
                required(&payload, "id")?,
                target,
                required(&payload, "date")?,
                &head,
            )
            .map_err(err)?;
            let command = store::prepare(
                &r,
                store::changes_between(&r.portfolio, &next).map_err(err)?,
                &new_id(),
                "cycles-desktop",
            )
            .map_err(err)?;
            return s.stage(command, &f);
        }
        "snapshot_preview" => {
            s.replaces_pending = None;
            s.reload().await?;
            let head = s.check_head(&payload)?;
            let r = s.journal.replay().map_err(err)?;
            let phase = payload["phase"].as_str().unwrap_or("close");
            let manual: Option<Money> = if phase.ends_with("flow") {
                Some(decode(payload["equity"].clone())?)
            } else {
                None
            };
            let changes = store::valuation_changes(
                &r.portfolio,
                &f,
                phase,
                payload["flow_id"].as_str().unwrap_or(""),
                manual,
                payload["currency"].as_str(),
                &head,
            )
            .map_err(err)?;
            let command = store::prepare(&r, changes, &new_id(), "cycles-desktop").map_err(err)?;
            return s.stage(command, &f);
        }
        "market_csv_preview" => {
            s.replaces_pending = None;
            s.reload().await?;
            let head = s.check_head(&payload)?;
            let changes = store::market_csv(Path::new(required(&payload, "path")?)).map_err(err)?;
            let command = store::stage(&s.journal, &head, None, changes).map_err(err)?;
            return s.stage(command, &f);
        }
        "import_preview" => {
            let preview = store::preview_legacy(
                Path::new(required(&payload, "path")?),
                required(&payload, "as_of")?,
            )
            .map_err(err)?;
            s.legacy = Some(preview.clone());
            return serde_json::to_value(preview).map_err(err);
        }
        "import_prepare" => {
            s.replaces_pending = None;
            s.reload().await?;
            s.check_head(&payload)?;
            let preview = s.legacy.as_ref().ok_or("Preview the tracker first")?;
            let choices = decode(payload["choices"].clone())?;
            let r = s.journal.replay().map_err(err)?;
            let next = store::apply_legacy(&r.portfolio, preview, &choices).map_err(err)?;
            let command = store::prepare(
                &r,
                store::changes_between(&r.portfolio, &next).map_err(err)?,
                &new_id(),
                "cycles-import",
            )
            .map_err(err)?;
            return s.stage(command, &f);
        }
        "commit" => {
            let command = s
                .staged
                .clone()
                .filter(|p| Some(p.commit.command_id.as_str()) == payload["command_id"].as_str())
                .ok_or("Preview this command before saving")?;
            let intent = Intent {
                source: s.source.clone(),
                prepared: command,
            };
            s.commit_intent(intent).await?;
        }
        "retry_pending" => {
            let id = required(&payload, "id")?;
            let intent: Intent = decode(
                serde_json::from_slice(&fs::read(s.pending_path(id)?).map_err(err)?)
                    .map_err(err)?,
            )?;
            if intent.source == Source::Sample {
                return Err("An in-memory sample cannot be recovered after restart. Discard this draft and reopen a saved spreadsheet.".into());
            }
            s.commit_intent(intent).await?;
        }
        "discard_pending" => {
            let path = s.pending_path(required(&payload, "id")?)?;
            fs::remove_file(path).map_err(err)?;
        }
        "discard_preview" => s.staged = None,
        _ => return Err("Unknown app operation".into()),
    }
    s.view(&f)
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(feature = "e2e")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(not(feature="e2e"))]
            let dir = app.path().app_data_dir()?;
            #[cfg(feature="e2e")]
            let dir=std::env::var_os("CYCLES_TEST_DATA_DIR").map(PathBuf::from).ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidInput,"Run the e2e build through pnpm test:desktop with its isolated data directory"))?;
            fs::create_dir_all(&dir)?;
            app.manage(Mutex::new(Session::new(dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api])
        .run(tauri::generate_context!())
        .expect("failed to run Cycles desktop app");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn retry_after_verified_local_save_never_duplicates_a_cash_movement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portfolio.xlsx");
        let p = store::sample_portfolio();
        let journal = store::Journal::from_portfolio(&p).unwrap();
        store::export_backup(&path, &journal).unwrap();
        let replay = journal.replay().unwrap();
        let movement = CashMovement::funding(
            "new-deposit",
            "account",
            "cycle",
            "USD",
            "2026-11-09",
            Money::from(100),
        );
        let prepared = store::prepare(
            &replay,
            vec![store::Change::upsert(
                "CashMovements",
                serde_json::to_value(movement).unwrap(),
            )],
            &new_id(),
            "test",
        )
        .unwrap();
        let intent = Intent {
            source: Source::Local {
                path: path.to_string_lossy().into(),
            },
            prepared,
        };
        let mut session = Session::new(dir.path().join("state"));
        session.commit_intent(intent.clone()).await.unwrap();
        session.commit_intent(intent).await.unwrap();
        let saved = store::load_local(&path).unwrap().journal.replay().unwrap();
        assert_eq!(
            saved
                .portfolio
                .cash_movements
                .iter()
                .filter(|m| m.id == "new-deposit")
                .count(),
            1
        );
        assert!(session.pending().is_empty());
        assert_eq!(saved.revision, 2);
    }
    #[tokio::test]
    async fn a_conflicted_command_survives_restart_without_changing_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portfolio.xlsx");
        let mut j = store::Journal::from_portfolio(&store::sample_portfolio()).unwrap();
        store::export_backup(&path, &j).unwrap();
        let original = j.replay().unwrap();
        let prepared=store::prepare(&original,vec![store::Change::upsert("Cycles",json!({"id":"next","name":"Next","strategy":"","start_date":"2026-11-09","state":"active"}))],&new_id(),"test").unwrap();
        let rival=store::prepare(&original,vec![store::Change::upsert("Cycles",json!({"id":"rival","name":"Different","strategy":"","start_date":"2026-11-09","state":"active"}))],&new_id(),"other").unwrap();
        j.append(rival);
        let hash = store::fingerprint(&path).unwrap();
        store::save_local(&path, &j, Some(&hash)).unwrap();
        let head = j.replay().unwrap().head;
        let intent = Intent {
            source: Source::Local {
                path: path.to_string_lossy().into(),
            },
            prepared,
        };
        let mut session = Session::new(dir.path().join("state"));
        assert!(session
            .commit_intent(intent.clone())
            .await
            .unwrap_err()
            .contains("Conflict"));
        let restarted = Session::new(dir.path().join("state"));
        assert_eq!(restarted.pending().len(), 1);
        assert_eq!(
            store::load_local(&path)
                .unwrap()
                .journal
                .replay()
                .unwrap()
                .head,
            head
        );
        assert!(restarted.pending_path("../../bad").is_err());
    }
}

#[cfg(test)]
mod live_tests;
