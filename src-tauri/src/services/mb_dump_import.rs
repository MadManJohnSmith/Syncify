//! Bounded, streaming importer for the legacy MusicBrainz mbdump catalog.
//!
//! Column order checked against musicbrainz-server/admin/sql/CreateTables.sql at
//! 3d4e576c82d75f3505f2b1586abdfd469f8a7ad4 (pre-2026 schema change):
//! recording: id,gid,name,artist_credit,length,comment,edits_pending,last_updated,video;
//! isrc: id,recording,isrc,source,edits_pending,created (6 columns).
//! The measured five-column dump is an older projection: id,recording,isrc,source,
//! last_updated. Its fifth timestamp is mandatory. Other layouts fail closed.
//! artist: id,gid,name,sort_name,...,begin_area,end_area (19 columns);
//! artist_credit_name: artist_credit,position,artist,name,join_phrase.

use sqlx::{Row, SqlitePool};
use std::{path::Path, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::UnboundedSender,
};

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
const CHUNK: usize = 10_000;
#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
const SCHEMA_VERSION: i64 = 1;
#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
const TABLES: [&str; 4] = ["recording", "artist", "artist_credit_name", "isrc"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportCounts {
    pub recording: u64,
    pub artist: u64,
    pub artist_credit_name: u64,
    pub isrc: u64,
}

// Capa de ingesta: las lee `tests/mb_dump_import_test.rs` y el emisor de eventos,
// todavía no la invoca el flujo de web/UI.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ImportProgress {
    pub table: &'static str,
    pub rows: u64,
    pub expected: u64,
}

#[allow(dead_code)] // Leído por `tests/mb_dump_import_test.rs`; la UI consumirá `import_state` después.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportState {
    pub dump_path: String,
    pub dump_timestamp: String,
    pub schema_version: i64,
    pub recording_rows: i64,
    pub isrc_rows: i64,
    pub artist_rows: i64,
    pub artist_credit_name_rows: i64,
    pub status: String,
    pub error: Option<String>,
    pub imported_at: Option<String>,
}

/// Read the last import attempt; `None` means this catalog has never been imported.
#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
pub async fn import_state(db: &SqlitePool) -> Result<Option<ImportState>, String> {
    sqlx::query_as::<_, ImportState>("SELECT dump_path,dump_timestamp,schema_version,recording_rows,isrc_rows,artist_rows,artist_credit_name_rows,status,error,imported_at FROM mb_dump_state WHERE id=1")
        .fetch_optional(db).await.map_err(|e| e.to_string())
}

/// Import selected dump tables into the caller's app-data SQLite pool.
// Punto de entrada público: cubierto por `tests/mb_dump_import_test.rs`; el flujo
// de UI/importación aún no lo conecta (solo ingestión).
#[allow(dead_code)]
/// Requires GNU tar and a legacy, verified column layout. Emits committed chunk counts.
pub async fn start_import(
    dump_path: impl AsRef<Path>,
    db: &SqlitePool,
    progress_tx: UnboundedSender<ImportProgress>,
) -> Result<ImportCounts, String> {
    let path = dump_path.as_ref();
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("dump metadata: {e}"))?;
    if !metadata.is_file() {
        return Err("dump path is not a file".into());
    }
    let path_text = path.to_string_lossy().into_owned();
    let timestamp = metadata
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    let mut counts = ImportCounts::default();
    write_state(db, &path_text, &timestamp, &counts, "importing", None).await?;
    let result = async {
        for table in TABLES {
            // A complete first pass supplies the expected count. The second pass checks
            // each row's layout, then commits only complete 10k chunks; the tail is
            // committed only after a clean child exit and an exact row-count match.
            let expected = count_table(path, table).await?;
            let actual = import_table(path, table, expected, db, &progress_tx, &mut counts).await?;
            if actual != expected {
                return Err(format!("{table}: expected {expected} rows, got {actual}"));
            }
        }
        Ok::<(), String>(())
    }
    .await;
    match result {
        Ok(()) => {
            write_state(db, &path_text, &timestamp, &counts, "completed", None).await?;
            Ok(counts)
        }
        Err(error) => {
            write_state(db, &path_text, &timestamp, &counts, "failed", Some(&error))
                .await
                .map_err(|state_error| {
                    format!("{error}; additionally failed to record state: {state_error}")
                })?;
            Err(error)
        }
    }
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
async fn write_state(
    db: &SqlitePool,
    path: &str,
    timestamp: &str,
    c: &ImportCounts,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    sqlx::query("INSERT INTO mb_dump_state (id,dump_path,dump_timestamp,schema_version,recording_rows,isrc_rows,artist_rows,artist_credit_name_rows,status,error,imported_at) VALUES (1,?,?,?,?,?,?,?,?,?,CASE WHEN ?='completed' THEN datetime('now') ELSE NULL END) ON CONFLICT(id) DO UPDATE SET dump_path=excluded.dump_path,dump_timestamp=excluded.dump_timestamp,schema_version=excluded.schema_version,recording_rows=excluded.recording_rows,isrc_rows=excluded.isrc_rows,artist_rows=excluded.artist_rows,artist_credit_name_rows=excluded.artist_credit_name_rows,status=excluded.status,error=excluded.error,imported_at=excluded.imported_at")
        .bind(path).bind(timestamp).bind(SCHEMA_VERSION)
        .bind(c.recording as i64).bind(c.isrc as i64).bind(c.artist as i64).bind(c.artist_credit_name as i64)
        .bind(status).bind(error).bind(status).execute(db).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
fn command(path: &Path, table: &str) -> tokio::process::Command {
    let mut cmd = crate::cmd_utils::create_tokio_command("tar");
    cmd.arg("-xjOf")
        .arg(path)
        .arg(format!("mbdump/{table}"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    cmd
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
async fn count_table(path: &Path, table: &str) -> Result<u64, String> {
    let mut child = command(path, table)
        .spawn()
        .map_err(|e| format!("tar {table} spawn: {e}"))?;
    let stdout = child.stdout.take().ok_or("tar stdout missing")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut count = 0u64;
    let result = async {
        while lines
            .next_line()
            .await
            .map_err(|e| format!("tar {table} read: {e}"))?
            .is_some()
        {
            count += 1;
        }
        let status = child
            .wait()
            .await
            .map_err(|e| format!("tar {table} wait: {e}"))?;
        if !status.success() {
            return Err(format!("tar {table} exited {status}"));
        }
        Ok(count)
    }
    .await;
    if result.is_err() {
        let _ = child.kill().await;
    }
    result
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
fn field<'a>(cols: &'a [&'a str], index: usize, table: &str) -> Result<&'a str, String> {
    match cols.get(index).copied() {
        Some("\\N") | Some("") | None => Err(format!(
            "{table}: required column {index} is null or missing"
        )),
        Some(v) => Ok(v),
    }
}
#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
fn number(cols: &[&str], index: usize, table: &str) -> Result<i64, String> {
    field(cols, index, table)?
        .parse::<i64>()
        .map_err(|e| format!("{table}: invalid integer in column {index}: {e}"))
}
#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
fn optional_number(cols: &[&str], index: usize, table: &str) -> Result<Option<i64>, String> {
    if cols[index] == "\\N" {
        Ok(None)
    } else {
        number(cols, index, table).map(Some)
    }
}
#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
fn validate(table: &str, cols: &[&str]) -> Result<(), String> {
    let expected = match table {
        "recording" => 9,
        "artist" => 19,
        "artist_credit_name" => 5,
        "isrc" => 5,
        _ => return Err("unknown table".into()),
    };
    if cols.len() != expected {
        return Err(format!(
            "{table}: unsupported schema: expected {expected} columns, found {}",
            cols.len()
        ));
    }
    match table {
        "recording" => {
            number(cols, 0, table)?;
            uuid::Uuid::parse_str(field(cols, 1, table)?)
                .map_err(|e| format!("recording gid: {e}"))?;
            field(cols, 2, table)?;
            number(cols, 3, table)?;
            optional_number(cols, 4, table)?;
            number(cols, 6, table)?;
            field(cols, 7, table)?;
            number(cols, 8, table)?;
        }
        "isrc" => {
            number(cols, 0, table)?;
            number(cols, 1, table)?;
            let code = field(cols, 2, table)?;
            if code.len() != 12
                || !code
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            {
                return Err(format!("invalid ISRC: {code}"));
            }
            optional_number(cols, 3, table)?;
            if !field(cols, 4, table)?.contains('-') {
                return Err("isrc: expected timestamp in column 4".into());
            }
        }
        "artist" => {
            number(cols, 0, table)?;
            uuid::Uuid::parse_str(field(cols, 1, table)?)
                .map_err(|e| format!("artist gid: {e}"))?;
            field(cols, 2, table)?;
        }
        "artist_credit_name" => {
            number(cols, 0, table)?;
            number(cols, 1, table)?;
            number(cols, 2, table)?;
            field(cols, 3, table)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
async fn import_table(
    path: &Path,
    table: &'static str,
    expected: u64,
    db: &SqlitePool,
    progress_tx: &UnboundedSender<ImportProgress>,
    counts: &mut ImportCounts,
) -> Result<u64, String> {
    let mut child = command(path, table)
        .spawn()
        .map_err(|e| format!("tar {table} spawn: {e}"))?;
    let stdout = child.stdout.take().ok_or("tar stdout missing")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut rows = Vec::with_capacity(CHUNK);
    let mut seen = 0u64;
    let result = async {
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|e| format!("tar {table} read: {e}"))?
        {
            let cols: Vec<&str> = line.split('\t').collect();
            validate(table, &cols).map_err(|e| format!("{e} at row {}", seen + 1))?;
            seen += 1;
            if seen > expected {
                return Err(format!("{table}: more rows than counted ({expected})"));
            }
            rows.push(line);
            if rows.len() == CHUNK {
                commit_chunk(db, table, &rows).await?;
                rows.clear();
                *count_for(counts, table) += CHUNK as u64;
                let _ = progress_tx.send(ImportProgress {
                    table,
                    rows: seen,
                    expected,
                });
            }
        }
        let status = child
            .wait()
            .await
            .map_err(|e| format!("tar {table} wait: {e}"))?;
        if !status.success() {
            return Err(format!("tar {table} exited {status}"));
        }
        if seen != expected {
            return Err(format!("{table}: expected {expected} rows, got {seen}"));
        }
        if !rows.is_empty() {
            commit_chunk(db, table, &rows).await?;
            *count_for(counts, table) += rows.len() as u64;
            let _ = progress_tx.send(ImportProgress {
                table,
                rows: seen,
                expected,
            });
        }
        Ok(seen)
    }
    .await;
    if result.is_err() {
        let _ = child.kill().await;
    }
    result
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
fn count_for<'a>(c: &'a mut ImportCounts, table: &str) -> &'a mut u64 {
    match table {
        "recording" => &mut c.recording,
        "artist" => &mut c.artist,
        "artist_credit_name" => &mut c.artist_credit_name,
        "isrc" => &mut c.isrc,
        _ => unreachable!(),
    }
}

#[allow(dead_code)] // Cubierto por `tests/mb_dump_import_test.rs`.
async fn commit_chunk(db: &SqlitePool, table: &str, rows: &[String]) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    for line in rows {
        let c: Vec<&str> = line.split('\t').collect();
        let result = match table {
            "recording" => sqlx::query("INSERT INTO mb_dump_recording(recording_id,mbid,name,artist_credit,length) VALUES (?,?,?,?,?) ON CONFLICT(recording_id) DO UPDATE SET mbid=excluded.mbid,name=excluded.name,artist_credit=excluded.artist_credit,length=excluded.length")
                .bind(number(&c,0,table)?).bind(c[1]).bind(c[2]).bind(number(&c,3,table)?).bind(optional_number(&c,4,table)?).execute(&mut *tx).await,
            "artist" => sqlx::query("INSERT INTO mb_dump_artist(artist_id,mbid,name) VALUES (?,?,?) ON CONFLICT(artist_id) DO UPDATE SET mbid=excluded.mbid,name=excluded.name")
                .bind(number(&c,0,table)?).bind(c[1]).bind(c[2]).execute(&mut *tx).await,
            "artist_credit_name" => sqlx::query("INSERT INTO mb_dump_artist_credit_name(artist_credit,position,artist,name,join_phrase) VALUES (?,?,?,?,?) ON CONFLICT(artist_credit,position) DO UPDATE SET artist=excluded.artist,name=excluded.name,join_phrase=excluded.join_phrase")
                .bind(number(&c,0,table)?).bind(number(&c,1,table)?).bind(number(&c,2,table)?).bind(c[3]).bind(if c[4] == "\\N" { "" } else { c[4] }).execute(&mut *tx).await,
            "isrc" => {
                let recording_id = number(&c,1,table)?;
                let mbid: Option<String> = sqlx::query("SELECT mbid FROM mb_dump_recording WHERE recording_id=?")
                    .bind(recording_id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?
                    .map(|r| r.get(0));
                let mbid = mbid.ok_or_else(|| format!("isrc: missing recording id {recording_id}"))?;
                sqlx::query("INSERT INTO mb_dump_recording_isrc(isrc_id,isrc,recording_id,recording_mbid) VALUES (?,?,?,?) ON CONFLICT(isrc_id) DO UPDATE SET isrc=excluded.isrc,recording_id=excluded.recording_id,recording_mbid=excluded.recording_mbid")
                    .bind(number(&c,0,table)?).bind(c[2]).bind(recording_id).bind(mbid).execute(&mut *tx).await
            }
            _ => unreachable!(),
        };
        result.map_err(|e| format!("{table} insert: {e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("{table} commit: {e}"))
}
