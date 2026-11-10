use crate::*;
use calamine::{Data, Reader, Xlsx};
use fs2::FileExt;
use rust_xlsxwriter::{Format, Workbook};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
#[derive(Debug, Clone)]
pub struct LocalDocument {
    pub journal: Journal,
    pub fingerprint: String,
}
pub fn fingerprint(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub fn load_local(path: &Path) -> Result<LocalDocument> {
    if fs::metadata(path)?.len() > 100 * 1024 * 1024 {
        return Err(Error::Corrupt(
            "Workbook exceeds the 100 MB local import limit".into(),
        ));
    }
    let before = fingerprint(path)?;
    let mut book: Xlsx<_> = calamine::open_workbook(path)
        .map_err(|e: calamine::XlsxError| Error::Corrupt(e.to_string()))?;
    let mut tables = BTreeMap::new();
    for name in ["Meta", "Commits"]
        .iter()
        .copied()
        .chain(TABLES.iter().copied())
    {
        let formulas = book
            .worksheet_formula(name)
            .map_err(|e| Error::Corrupt(format!("{name}: {e}")))?;
        if formulas.used_cells().any(|(_, _, x)| !x.is_empty()) {
            return Err(Error::Corrupt(format!(
                "Formula cells are not allowed in {name}"
            )));
        }
        let range = book
            .worksheet_range(name)
            .map_err(|e| Error::Corrupt(format!("{name}: {e}")))?;
        let mut rows = vec![];
        for r in range.rows() {
            let row = r
                .iter()
                .map(|v| match v {
                    Data::String(s) => Ok(s.clone()),
                    Data::Empty => Ok(String::new()),
                    _ => Err(Error::Corrupt(format!(
                        "Canonical {name} inputs must remain literal text"
                    ))),
                })
                .collect::<Result<Vec<_>>>()?;
            if row.iter().any(|x| !x.is_empty()) {
                rows.push(row);
            }
        }
        tables.insert(name.to_string(), rows);
    }
    let journal = parse_tables(&tables)?;
    let after = fingerprint(path)?;
    if before != after {
        return Err(Error::Conflict(
            "The workbook changed while it was being read".into(),
        ));
    }
    Ok(LocalDocument {
        journal,
        fingerprint: after,
    })
}
fn write_book(path: &Path, j: &Journal) -> Result<()> {
    let mut book = Workbook::new();
    let header = Format::new().set_bold().set_background_color("#E8EDE7");
    for (name, rows) in journal_tables(j)? {
        let sheet = book.add_worksheet();
        sheet
            .set_name(&name)
            .map_err(|e| Error::Corrupt(e.to_string()))?;
        sheet
            .set_freeze_panes(1, 0)
            .map_err(|e| Error::Corrupt(e.to_string()))?;
        sheet
            .set_column_width(0, 38)
            .map_err(|e| Error::Corrupt(e.to_string()))?;
        if name != "Meta" {
            sheet
                .set_column_width(4, 70)
                .map_err(|e| Error::Corrupt(e.to_string()))?;
        } else {
            sheet
                .set_column_width(1, 100)
                .map_err(|e| Error::Corrupt(e.to_string()))?;
        }
        for (r, row) in rows.iter().enumerate() {
            for (c, s) in row.iter().enumerate() {
                if s.chars().count() > 32767 {
                    return Err(Error::Corrupt(
                        "A raw record exceeds Excel's cell text limit".into(),
                    ));
                }
                if r == 0 {
                    sheet.write_string_with_format(r as u32, c as u16, s, &header)
                } else {
                    sheet.write_string(r as u32, c as u16, s)
                }
                .map_err(|e| Error::Corrupt(e.to_string()))?;
            }
        }
        if rows.len() > 1 {
            sheet
                .autofilter(
                    0,
                    0,
                    (rows.len() - 1) as u32,
                    rows[0].len().saturating_sub(1) as u16,
                )
                .map_err(|e| Error::Corrupt(e.to_string()))?;
        }
    }
    book.save(path).map_err(|e| Error::Corrupt(e.to_string()))?;
    Ok(())
}
pub fn save_local(path: &Path, j: &Journal, expected: Option<&str>) -> Result<String> {
    j.replay()?;
    let parent = path
        .parent()
        .filter(|x| !x.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.exists() {
        return Err(Error::Corrupt(
            "Destination directory does not exist".into(),
        ));
    }
    let name = path
        .file_name()
        .ok_or_else(|| Error::Corrupt("Choose an XLSX filename".into()))?
        .to_string_lossy();
    let lock_path = parent.join(format!(".{name}.cycles-lock"));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.try_lock_exclusive()
        .map_err(|_| Error::Conflict("Another Cycles process is saving this workbook".into()))?;
    let check = || -> Result<()> {
        match (expected, path.exists()) {
            (None, false) => Ok(()),
            (Some(hash), true) if fingerprint(path)? == hash => Ok(()),
            _ => Err(Error::Conflict(
                "The destination changed. Reload it or choose a new file.".into(),
            )),
        }
    };
    check()?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".cycles-")
        .suffix(".xlsx")
        .tempfile_in(parent)?;
    write_book(tmp.path(), j)?;
    tmp.as_file_mut().flush()?;
    tmp.as_file().sync_all()?;
    let reopened = load_local(tmp.path())?;
    if reopened.journal.replay()?.head != j.replay()?.head {
        return Err(Error::Corrupt(
            "Backup read-back did not match the saved revision".into(),
        ));
    }
    check()?;
    if path.exists() {
        let previous = parent.join(format!(".{name}.previous.xlsx"));
        fs::copy(path, previous)?;
    }
    tmp.persist(path).map_err(|e| Error::Io(e.error))?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    let result = fingerprint(path)?;
    FileExt::unlock(&lock)?;
    Ok(result)
}
pub fn export_backup(path: &Path, j: &Journal) -> Result<String> {
    save_local(path, j, None)
}
