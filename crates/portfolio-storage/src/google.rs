use crate::*;
use crate::{Error, Result};
use reqwest::Method;
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};
#[derive(Debug, Clone)]
pub struct GoogleDocument {
    pub id: String,
    pub journal: Journal,
    pub sheets: BTreeMap<String, i64>,
}
#[derive(Clone)]
pub struct GoogleSheets {
    http: reqwest::Client,
    token: String,
    endpoint: String,
}
pub fn spreadsheet_id(value: &str) -> Result<String> {
    let parsed;
    let id = if value.contains("://") {
        parsed =
            url::Url::parse(value).map_err(|_| Error::Remote("Invalid spreadsheet URL".into()))?;
        if parsed.scheme() != "https" || parsed.host_str() != Some("docs.google.com") {
            return Err(Error::Remote(
                "Use a docs.google.com spreadsheet URL or its ID".into(),
            ));
        }
        parsed
            .path()
            .strip_prefix("/spreadsheets/d/")
            .and_then(|p| p.split('/').next())
            .unwrap_or_default()
    } else {
        value
    };
    if id.is_empty()
        || id.len() > 200
        || !id
            .bytes()
            .all(|x| x.is_ascii_alphanumeric() || x == b'-' || x == b'_')
    {
        return Err(Error::Remote(
            "Paste a valid Google Sheets URL or spreadsheet ID".into(),
        ));
    }
    Ok(id.into())
}
impl GoogleSheets {
    pub fn new(token: String) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .map_err(|_| Error::Remote("Could not initialize secure networking".into()))?,
            token,
            endpoint: "https://sheets.googleapis.com/v4/spreadsheets".into(),
        })
    }
    async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Option<Value>,
    ) -> Result<Value> {
        for attempt in 0..4 {
            let mut r = self
                .http
                .request(method.clone(), format!("{}{path}", self.endpoint))
                .bearer_auth(&self.token)
                .query(query);
            if let Some(b) = &body {
                r = r.json(b)
            }
            let response=r.send().await.map_err(|_|Error::Remote("Google request outcome could not be confirmed. Reload or retry the pending operation.".into()))?;
            let status = response.status();
            if (status.as_u16() == 429 || (status.is_server_error() && method == Method::GET))
                && attempt < 3
            {
                tokio::time::sleep(Duration::from_millis(400 * (1 << attempt))).await;
                continue;
            }
            if !status.is_success() {
                return Err(Error::Remote(match status.as_u16(){401=>"Google authorization expired or was revoked. Reconnect your account.",403=>"Google access denied. Check that this file was created by Cycles under this Google app/account and that the Sheets API is enabled.",404=>"Google spreadsheet not found or not authorized for this app.",_=>"Google could not confirm the operation. Keep the pending edit and reload before retrying."}.into()));
            }
            return response
                .json()
                .await
                .map_err(|_| Error::Remote("Google returned an unreadable response".into()));
        }
        Err(Error::Remote(
            "Google quota retries exhausted; the edit remains pending".into(),
        ))
    }
    pub async fn create(&self, title: &str, journal: &Journal) -> Result<GoogleDocument> {
        let names: Vec<_> = std::iter::once("Meta")
            .chain(TABLES.iter().copied())
            .chain(std::iter::once("Commits"))
            .collect();
        let sheets:Vec<_>=names.iter().enumerate().map(|(n,name)|json!({"properties":{"sheetId":n,"title":name,"gridProperties":{"rowCount":1000,"columnCount":16,"frozenRowCount":1}}})).collect();
        let created = self
            .request(
                Method::POST,
                "",
                &[],
                Some(json!({"properties":{"title":title},"sheets":sheets})),
            )
            .await?;
        let id = created["spreadsheetId"]
            .as_str()
            .ok_or_else(|| Error::Remote("Google did not return the new spreadsheet ID".into()))?
            .to_string();
        let tables = journal_tables(journal)?;
        let append = |name: &str, rows: &[Vec<String>]| -> Result<Value> {
            let index = names
                .iter()
                .position(|x| *x == name)
                .ok_or_else(|| Error::Corrupt("Unknown table".into()))?;
            Ok(
                json!({"appendCells":{"sheetId":index,"rows":grid_rows(rows),"fields":"userEnteredValue"}}),
            )
        };
        let mut headers = vec![];
        for (name, rows) in &tables {
            headers.push(append(
                name,
                if name == "Meta" { rows } else { &rows[..1] },
            )?);
        }
        let initialize = async {
            self.request(
                Method::POST,
                &format!("/{id}:batchUpdate"),
                &[],
                Some(json!({"requests":headers})),
            )
            .await?;
            for (name, rows) in tables
                .iter()
                .filter(|(name, _)| name.as_str() != "Meta" && name.as_str() != "Commits")
            {
                for chunk in rows[1..].chunks(200) {
                    self.request(
                        Method::POST,
                        &format!("/{id}:batchUpdate"),
                        &[],
                        Some(json!({"requests":[append(name,chunk)?]})),
                    )
                    .await?;
                }
            }
            for chunk in tables["Commits"][1..].chunks(200) {
                self.request(
                    Method::POST,
                    &format!("/{id}:batchUpdate"),
                    &[],
                    Some(json!({"requests":[append("Commits",chunk)?]})),
                )
                .await?;
            }
            Ok::<(), Error>(())
        }
        .await;
        initialize.map_err(|e|Error::Remote(format!("Google created spreadsheet {id}, but initialization was not confirmed: {e}. Your current source is unchanged. Do not use the partial copy as your source.")))?;
        self.load(&id).await
    }
    pub async fn load(&self, raw: &str) -> Result<GoogleDocument> {
        let id = spreadsheet_id(raw)?;
        for _ in 0..3 {
            let sheetdata = self
                .request(
                    Method::GET,
                    &format!("/{id}"),
                    &[("fields".into(), "sheets.properties".into())],
                    None,
                )
                .await?;
            let mut sheets = BTreeMap::new();
            for s in sheetdata["sheets"]
                .as_array()
                .ok_or_else(|| Error::Remote("Missing spreadsheet metadata".into()))?
            {
                let name = s["properties"]["title"].as_str().unwrap_or_default();
                if let Some(n) = s["properties"]["sheetId"].as_i64() {
                    sheets.insert(name.to_string(), n);
                }
            }
            let names: Vec<_> = std::iter::once("Meta")
                .chain(TABLES.iter().copied())
                .chain(std::iter::once("Commits"))
                .collect();
            if names.iter().any(|name| !sheets.contains_key(*name)) {
                return Err(Error::Corrupt("This is not an initialized Cycles spreadsheet. Import the legacy workbook into a new source.".into()));
            }
            let mut query: Vec<(String, String)> = names
                .iter()
                .map(|name| ("ranges".into(), format!("'{name}'!A1:O")))
                .collect();
            query.push(("valueRenderOption".into(), "FORMULA".into()));
            let values = self
                .request(Method::GET, &format!("/{id}/values:batchGet"), &query, None)
                .await?;
            let ranges = values["valueRanges"]
                .as_array()
                .ok_or_else(|| Error::Remote("Missing spreadsheet values".into()))?;
            if ranges.len() != names.len() {
                return Err(Error::Corrupt("Spreadsheet read was incomplete".into()));
            }
            let mut tables = BTreeMap::new();
            for (name, range) in names.iter().zip(ranges) {
                tables.insert(name.to_string(), string_rows(&range["values"])?);
            }
            let journal = parse_tables(&tables)?;
            let check = self
                .request(
                    Method::GET,
                    &format!("/{id}/values/'Commits'!A1:F"),
                    &[("valueRenderOption".into(), "FORMULA".into())],
                    None,
                )
                .await?;
            if string_rows(&check["values"])? == *tables.get("Commits").unwrap() {
                return Ok(GoogleDocument {
                    id,
                    journal,
                    sheets,
                });
            }
        }
        Err(Error::Conflict(
            "The source kept changing while it was loading. Refresh again.".into(),
        ))
    }
    pub async fn append(&self, doc: &GoogleDocument, command: &Prepared) -> Result<GoogleDocument> {
        let current = self.load(&doc.id).await?;
        let state = current.journal.replay()?;
        if state.accepted.contains(&command.commit.command_id) {
            return Ok(current);
        }
        if current
            .journal
            .commits
            .iter()
            .any(|x| x.command_id == command.commit.command_id)
        {
            return Err(Error::Conflict("This operation is already recorded as a conflict. Review it before creating a new intent.".into()));
        }
        if state.head != command.commit.parent {
            return Err(Error::Conflict(
                "Source changed before saving. Review this edit against the latest source.".into(),
            ));
        }
        let mut requests = vec![];
        for table in TABLES {
            let rows = command
                .records
                .iter()
                .filter(|r| r.table == *table)
                .map(record_cells)
                .collect::<Result<Vec<_>>>()?;
            if !rows.is_empty() {
                requests.push(json!({"appendCells":{"sheetId":current.sheets[*table],"rows":grid_rows(&rows),"fields":"userEnteredValue"}}));
            }
        }
        let commit = json!({"appendCells":{"sheetId":current.sheets["Commits"],"rows":grid_rows(&[commit_cells(&command.commit)?]),"fields":"userEnteredValue"}});
        // Large operations stage immutable row chunks; only the final manifest publishes them.
        if serde_json::to_vec(&requests)?.len() > 1_500_000 {
            for request in requests {
                let rows = request["appendCells"]["rows"]
                    .as_array()
                    .ok_or_else(|| Error::Corrupt("Invalid append payload".into()))?;
                for chunk in rows.chunks(200) {
                    let mut part = request.clone();
                    part["appendCells"]["rows"] = Value::Array(chunk.to_vec());
                    self.request(
                        Method::POST,
                        &format!("/{}:batchUpdate", doc.id),
                        &[],
                        Some(json!({"requests":[part]})),
                    )
                    .await?;
                }
            }
            requests = vec![commit];
        } else {
            requests.push(commit);
        }
        self.request(
            Method::POST,
            &format!("/{}:batchUpdate", doc.id),
            &[],
            Some(json!({"requests":requests})),
        )
        .await?;
        let loaded = self.load(&doc.id).await?;
        if !loaded
            .journal
            .replay()?
            .accepted
            .contains(&command.commit.command_id)
        {
            return Err(Error::Conflict(
                "A competing save was accepted first. This edit is retained for review.".into(),
            ));
        }
        Ok(loaded)
    }
}
fn grid_rows(rows: &[Vec<String>]) -> Vec<Value> {
    rows.iter().map(|row|json!({"values":row.iter().map(|s|json!({"userEnteredValue":{"stringValue":s}})).collect::<Vec<_>>()})).collect()
}
fn string_rows(value: &Value) -> Result<Vec<Vec<String>>> {
    if value.is_null() {
        return Ok(vec![]);
    }
    value
        .as_array()
        .ok_or_else(|| Error::Corrupt("Invalid sheet rows".into()))?
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or_else(|| Error::Corrupt("Invalid sheet row".into()))?
                .iter()
                .map(|x| {
                    x.as_str().map(String::from).ok_or_else(|| {
                        Error::Corrupt("Canonical spreadsheet values must be literal text".into())
                    })
                })
                .collect()
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };
    #[test]
    fn id_rejects_paths_and_other_hosts_as_identifiers() {
        assert!(spreadsheet_id("../../secret").is_err());
        assert!(spreadsheet_id("https://example.com/d/abc").is_err());
        assert_eq!(
            spreadsheet_id("https://docs.google.com/spreadsheets/d/abc_DEF/edit").unwrap(),
            "abc_DEF"
        );
    }
    #[tokio::test]
    async fn auth_errors_are_actionable_and_do_not_echo_secrets() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/abc"))
            .respond_with(ResponseTemplate::new(401).set_body_string("access_token=secret"))
            .mount(&server)
            .await;
        let mut client = GoogleSheets::new("secret".into()).unwrap();
        client.endpoint = server.uri();
        let err = client.load("abc").await.unwrap_err().to_string();
        assert!(err.contains("Reconnect"));
        assert!(!err.contains("secret"));
    }
    #[tokio::test]
    async fn a_lost_write_acknowledgement_is_resolved_by_replay_before_retry() {
        use std::sync::{Arc, Mutex};
        let journal = Journal::from_portfolio(&sample_portfolio()).unwrap();
        let names: Vec<String> = std::iter::once("Meta")
            .chain(TABLES.iter().copied())
            .chain(std::iter::once("Commits"))
            .map(String::from)
            .collect();
        let tables = Arc::new(Mutex::new(journal_tables(&journal).unwrap()));
        let writes = Arc::new(Mutex::new(0usize));
        let captured = tables.clone();
        let calls = writes.clone();
        let ns = names.clone();
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::any()).respond_with(move |request:&wiremock::Request|{
            let path=request.url.path();let mut data=captured.lock().unwrap();
            if request.method==Method::POST {
                let v:Value=serde_json::from_slice(&request.body).unwrap();for edit in v["requests"].as_array().unwrap(){let append=&edit["appendCells"];let name=&ns[append["sheetId"].as_u64().unwrap() as usize];for row in append["rows"].as_array().unwrap(){let strings=row["values"].as_array().unwrap().iter().map(|c|{assert!(c["userEnteredValue"]["formulaValue"].is_null());c["userEnteredValue"]["stringValue"].as_str().unwrap().to_string()}).collect();data.get_mut(name).unwrap().push(strings);}}
                *calls.lock().unwrap()+=1;return ResponseTemplate::new(503);
            }
            if path.contains("batchGet"){return ResponseTemplate::new(200).set_body_json(json!({"valueRanges":ns.iter().map(|name|json!({"values":data[name]})).collect::<Vec<_>>()}));}
            if path.contains("Commits"){return ResponseTemplate::new(200).set_body_json(json!({"values":data["Commits"]}));}
            ResponseTemplate::new(200).set_body_json(json!({"sheets":ns.iter().enumerate().map(|(n,name)|json!({"properties":{"title":name,"sheetId":n}})).collect::<Vec<_>>()}))
        }).mount(&server).await;
        let mut client = GoogleSheets::new("test-token".into()).unwrap();
        client.endpoint = server.uri();
        let doc = client.load("abc").await.unwrap();
        let state = doc.journal.replay().unwrap();
        let change = Change::upsert(
            "Cycles",
            json!({"id":"next","name":"Next","strategy":"","start_date":"2026-11-09","state":"active"}),
        );
        let command = prepare(&state, vec![change], &portfolio_core::new_id(), "test").unwrap();
        assert!(client.append(&doc, &command).await.is_err());
        let recovered = client.append(&doc, &command).await.unwrap();
        assert!(recovered
            .journal
            .replay()
            .unwrap()
            .accepted
            .contains(&command.commit.command_id));
        assert_eq!(*writes.lock().unwrap(), 1);
        assert_eq!(
            recovered.journal.replay().unwrap().portfolio.cycles.len(),
            2
        );
    }
}
