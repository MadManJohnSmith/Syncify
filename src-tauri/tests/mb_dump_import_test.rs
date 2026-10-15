use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use syncify_tauri_lib::services::mb_dump_import::start_import;
use tempfile::TempDir;

fn unbounded(
) -> tokio::sync::mpsc::UnboundedSender<syncify_tauri_lib::services::mb_dump_import::ImportProgress>
{
    tokio::sync::mpsc::unbounded_channel().0
}

async fn pool() -> (sqlx::SqlitePool, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let opts = SqliteConnectOptions::new()
        .filename(dir.path().join("test.db"))
        .create_if_missing(true);
    let db = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    (db, dir)
}

fn large_rows(n: usize) -> (String, String) {
    let mut isrc = String::new();
    let mut recording = String::new();
    for i in 0..n {
        let id = i + 1;
        let gid = {
            let hex = format!("{:x}", md5::compute(id.to_string().as_bytes()));
            format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            )
        };
        isrc.push_str(&format!(
            "{}\t{id}\tGB{:010}\t0\t2011-05-16 16:08:20.288158+00\n",
            id + 1000000,
            id
        ));
        recording.push_str(&format!(
            "{id}\t{gid}\tTrack {id}\t{id}\t260000\t\t0\t2020-01-01 00:00:00.000000+00\t0\n"
        ));
    }
    (isrc, recording)
}

fn dump(dir: &TempDir, isrc_rows: &str, recording_rows: &str) -> std::path::PathBuf {
    let path = dir.path().join("mbdump.tar.bz2");
    let base = dir.path().join("mbdump");
    std::fs::create_dir(&base).unwrap();
    std::fs::write(base.join("isrc"), isrc_rows).unwrap();
    std::fs::write(base.join("recording"), recording_rows).unwrap();
    std::fs::write(base.join("artist"), "").unwrap();
    std::fs::write(base.join("artist_credit_name"), "").unwrap();
    let status = std::process::Command::new("tar")
        .arg("-cjf")
        .arg(&path)
        .arg("-C")
        .arg(dir.path())
        .args([
            "mbdump/isrc",
            "mbdump/recording",
            "mbdump/artist",
            "mbdump/artist_credit_name",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    path
}

fn valid_isrc_line() -> String {
    "75478\t10\tGBAAA9000038\t0\t2011-05-16 16:08:20.288158+00".to_string()
}

fn valid_recording_line(id: i64, ac: i64) -> String {
    let gid = {
        let hex = format!("{:x}", md5::compute(id.to_string().as_bytes()));
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    };
    format!("{id}\t{gid}\tTravelling Man\t{ac}\t260000\t\t0\t2020-01-01 00:00:00.000000+00\t0")
}

#[tokio::test]
async fn large_stream_uses_bounded_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let (isrc, rec) = large_rows(50_000);
    let path = dump(&dir, &isrc, &rec);
    drop(isrc);
    drop(rec);
    let (db, _dbdir) = pool().await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let counts = start_import(&path, &db, tx).await.unwrap();
    assert_eq!(counts.recording, 50_000);
    assert_eq!(counts.isrc, 50_000);
    let mut progress = 0;
    while let Ok(event) = rx.try_recv() {
        if event.table == "isrc" {
            progress += 1;
        }
    }
    assert!(progress >= 5, "expected bounded chunk events");
}

#[tokio::test]
async fn import_ingests_mapping_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let isrc = format!(
        "{}\n{}",
        valid_isrc_line(),
        valid_isrc_line().replace("75478\t10", "75479\t11")
    );
    let rec = format!(
        "{}\n{}",
        valid_recording_line(10, 3),
        valid_recording_line(11, 4)
    );
    let path = dump(&dir, &isrc, &rec);

    let (db, _dbdir) = pool().await;
    let counts1 = start_import(&path, &db, unbounded())
        .await
        .expect("import failed");

    assert_eq!(counts1.isrc, 2);
    assert_eq!(counts1.recording, 2);
    assert_eq!(counts1.artist, 0);
    let join: (String, i64, Option<i64>) = sqlx::query_as(
        "SELECT r.name, r.length, i.isrc_id FROM mb_dump_recording r LEFT JOIN mb_dump_recording_isrc i ON i.recording_id = r.recording_id WHERE r.recording_id = 10")
        .fetch_one(&db).await.unwrap();
    assert_eq!(join.0, "Travelling Man");
    assert_eq!(join.1, 260000);
    assert_eq!(join.2, Some(75478));
    let linked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mb_dump_recording_isrc WHERE recording_id = 11 AND isrc = 'GBAAA9000038' AND recording_mbid IS NOT NULL")
        .fetch_one(&db).await.unwrap();
    assert_eq!(linked, 1);

    let counts2 = start_import(&path, &db, unbounded())
        .await
        .expect("re-import failed");
    assert_eq!(counts2.isrc, 2);
    assert_eq!(counts2.recording, 2);
    assert_eq!((counts1.isrc, counts1.recording), (2, 2));
}

#[tokio::test]
async fn import_rejects_unknown_column_layout() {
    let dir = tempfile::tempdir().unwrap();
    let bad = "75478\t10\tGBAAA9000038\t0\t2011-05-16 16:08:20.288158+00\textra".to_string();
    let path = dump(&dir, &bad, &valid_recording_line(20937085, 3));

    let (db, _dbdir) = pool().await;
    let err = start_import(&path, &db, unbounded())
        .await
        .expect_err("must fail closed");
    assert!(
        err.contains("isrc") && err.contains("6"),
        "unexpected error: {err}"
    );
    let state = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT status, error FROM mb_dump_state WHERE id = 1",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(state.0, "failed");
    assert!(state.1.unwrap().contains("isrc"));
}

#[tokio::test]
async fn import_keeps_committed_chunks_when_late_rows_are_malformed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut isrc, rec) = large_rows(10_001);
    isrc.replace_range(isrc.rfind('\n').unwrap() + 1.., "broken\t5\n");
    let path = dump(&dir, &isrc, &rec);

    let (db, _dbdir) = pool().await;
    let err = start_import(&path, &db, unbounded())
        .await
        .expect_err("expected failure");
    assert!(err.contains("isrc"), "unexpected error: {err}");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_dump_recording_isrc")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rows, 10_000, "the 10k committed chunk must survive");
    let recording: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_dump_recording")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(
        recording, 10_001,
        "previously completed table remains committed"
    );
    let state = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT status, error FROM mb_dump_state WHERE id = 1",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(state.0, "failed");
    assert!(state.1.unwrap().contains("isrc"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mb_dump_state")
            .fetch_one(&db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn import_detects_truncated_dump_payload() {
    let dir = tempfile::tempdir().unwrap();
    let (isrc, rec) = large_rows(10_001);
    let path = dump(&dir, &isrc, &rec);

    let (db, _dbdir) = pool().await;
    let truncated = dir.path().join("mbdump.tar.bz2.truncated");
    std::fs::copy(&path, &truncated).unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&truncated)
        .unwrap();
    file.set_len(1000).unwrap();
    drop(file);

    let err = start_import(&truncated, &db, unbounded())
        .await
        .expect_err("expected failure");
    assert!(
        err.contains("recording") || err.contains("isrc"),
        "unexpected error: {err}"
    );
    assert!(state_reports_failure(&db).await);
}

async fn state_reports_failure(db: &sqlx::SqlitePool) -> bool {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT status, error FROM mb_dump_state WHERE id = 1",
    )
    .fetch_one(db)
    .await
    .unwrap()
    .0 == "failed"
}
