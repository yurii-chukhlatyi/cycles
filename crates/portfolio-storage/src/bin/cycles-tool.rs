//! Headless spreadsheet inspection and explicit sample creation.
use portfolio_core::*;
use portfolio_storage::*;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let required = |n: usize| args.get(n).map(String::as_str).ok_or("Missing argument");
    match required(0)?{
 "sample"=>{let p=sample_portfolio();let j=Journal::from_portfolio(&p)?;export_backup(Path::new(required(1)?),&j)?;},
 "report"=>{let doc=load_local(Path::new(required(1)?))?;let state=doc.journal.replay()?;let f=ReportFilter::at(args.get(2).map(String::as_str).unwrap_or(&today()));println!("{}",serde_json::to_string_pretty(&report(&state.portfolio,&f)?)?);},
 "preview-json"=>{let p=sample_portfolio();let j=Journal::from_portfolio(&p)?;let r=j.replay()?;let value=json!({"portfolio":p,"head":r.head,"revision":r.revision,"source":{"kind":"sample"},"report":report(&r.portfolio,&ReportFilter::at("2026-11-09"))?,"pending":[],"conflicts":[],"google_connected":false});std::fs::write(required(1)?,serde_json::to_vec(&value)?)?;},
 "import-preview"=>{let preview=preview_legacy(Path::new(required(1)?),required(2)?)?;let counts:BTreeMap<_,_>=preview.portfolio.accounts.iter().map(|a|(a.name.clone(),preview.portfolio.positions.iter().filter(|x|x.account_id==a.id).count())).collect();println!("{}",serde_json::to_string_pretty(&json!({"positions":preview.portfolio.positions.len(),"transactions":preview.portfolio.transactions.len(),"accounts":counts,"cash_controls":preview.cash_controls,"unsupported":preview.unsupported,"pending_settlements":preview.pending_settlements,"warnings":preview.warnings}))?);if let Some(output)=args.get(3){std::fs::write(output,serde_json::to_vec_pretty(&preview)?)?;}},
 "import"=>{let preview=preview_legacy(Path::new(required(1)?),required(2)?)?;let choices:ImportChoices=serde_json::from_slice(&std::fs::read(required(3)?)?)?;let p=apply_legacy(&Portfolio::default(),&preview,&choices)?;export_backup(Path::new(required(4)?),&Journal::from_portfolio(&p)?)?;},
 "inspect"=>{let doc=load_local(Path::new(required(1)?))?;let r=doc.journal.replay()?;let value:Value=json!({"dataset":r.portfolio.dataset_id,"head":r.head,"revision":r.revision,"positions":r.portfolio.positions.len(),"transactions":r.portfolio.transactions.len(),"conflicts":r.conflicts});println!("{}",serde_json::to_string_pretty(&value)?);},
 _=>return Err("Usage: cycles-tool sample OUTPUT.xlsx | report SOURCE.xlsx [AS_OF] | inspect SOURCE.xlsx | import-preview TRACKER.xlsx AS_OF [PREVIEW.json] | import TRACKER.xlsx AS_OF CHOICES.json OUTPUT.xlsx | preview-json OUTPUT.json".into())}
    Ok(())
}
