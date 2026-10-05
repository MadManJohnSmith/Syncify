//! Recovery must describe the file it actually found, and only claim success
//! once that description is persisted (audit 16 and 17).
//!
//! Two regressions are covered:
//! 1. The reconciliation used to hardcode `FLAC / 16 bit / 44.1 kHz` for every
//!    recovered download, so a transfer that crashed right after landing as AAC
//!    produced a ledger row that contradicts the library on disk.
//! 2. The worker used to reset `downloading` rows while the post-crash
//!    reconciliation was still running in its own task, so the queue could be
//!    re-queued and failed at the same time. The worker now runs both passes
//!    itself before it looks at the queue.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use syncify_tauri_lib::services::operation_recovery::reconcile_downloads_on_startup;
use tempfile::TempDir;

/// A minimal but genuinely valid ISOBMFF/M4A container: 8-byte box header with
/// an `ftyp` box type, which is what the byte validator and the audio inspector
/// both key on.
fn write_m4a(path: &Path) {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&[0u8, 0u8, 0u8, 0x18]); // box size
    bytes.extend_from_slice(b"ftyp");
    bytes.extend_from_slice(b"M4A ");
    bytes.extend_from_slice(&[0u8; 8]);
    let mut f = File::create(path).expect("create m4a");
    f.write_all(&bytes).expect("write m4a");
    f.flush().expect("flush m4a");
}

/// A minimal but genuinely parseable FLAC file: `fLaC` magic followed by a
/// complete last-metadata STREAMINFO block declaring 44.1 kHz / 16-bit / stereo.
fn write_flac(path: &Path) {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"fLaC");
    // Last-metadata-block flag (0x80) | block type 0 (STREAMINFO), 34 bytes long.
    bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);

    bytes.extend_from_slice(&4096u16.to_be_bytes()); // min block size
    bytes.extend_from_slice(&4096u16.to_be_bytes()); // max block size
    bytes.extend_from_slice(&[0u8; 3]); // min frame size
    bytes.extend_from_slice(&[0u8; 3]); // max frame size
    let packed: u64 = (44100u64 << 44) | (1u64 << 41) | (15u64 << 36);
    bytes.extend_from_slice(&packed.to_be_bytes()); // rate | channels-1 | bps-1 | samples
    bytes.extend_from_slice(&[0u8; 16]); // MD5 signature

    let mut f = File::create(path).expect("create flac");
    f.write_all(&bytes).expect("write flac");
    f.flush().expect("flush flac");
}

async fn seed_db(temp: &TempDir, name: &str) -> sqlx::SqlitePool {
    let db_path = temp.path().join(name);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    pool
}

async fn seed_recoverable_download(
    pool: &sqlx::SqlitePool,
    queue_id: i64,
    physical_path: &Path,
) -> i64 {
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Recovered Track', 210000) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("insert track");

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, service_name, status, progress_percent)
        VALUES (?, ?, 'tidal', 'downloading', 42.0)
        "#,
    )
    .bind(queue_id)
    .bind(tid)
    .execute(pool)
    .await
    .expect("insert queue row");

    sqlx::query(
        r#"
        INSERT INTO operation_journal (
            operation_id, operation_type, entity_id, track_id, provider,
            phase, status, expected_output_path
        ) VALUES (?, 'DownloadTidal', ?, ?, 'tidal', 'Persisting', 'persisting', ?)
        "#,
    )
    .bind(format!("op-aac-{}", queue_id))
    .bind(queue_id.to_string())
    .bind(tid)
    .bind(physical_path.to_string_lossy().to_string())
    .execute(pool)
    .await
    .expect("insert journal entry");

    tid
}

/// A recovered transfer that landed as AAC must be recorded as AAC.
#[tokio::test]
async fn recovery_records_the_container_it_actually_found() {
    let temp = TempDir::new().unwrap();
    let pool = seed_db(&temp, "recovered_aac.db").await;

    let physical = temp.path().join("01 - Recovered Track.m4a");
    write_m4a(&physical);

    let track_id = seed_recoverable_download(&pool, 9001, &physical).await;

    reconcile_downloads_on_startup(&pool).await;

    let row: Option<(Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT file_format, bit_depth, sample_rate FROM downloads WHERE track_id = ?",
    )
    .bind(track_id)
    .fetch_optional(&pool)
    .await
    .expect("query downloads");

    let (format, bit_depth, sample_rate) = row.expect("the recovered download must be recorded");
    assert_eq!(
        format.as_deref(),
        Some("AAC"),
        "recovery invented FLAC for a file that is physically AAC"
    );
    assert_eq!(bit_depth, Some(16));
    assert_eq!(sample_rate, Some(44100));
}

/// The same transfer landing as FLAC is still recorded as FLAC, and the journal
/// entry is confirmed recovered because every write landed.
#[tokio::test]
async fn recovery_confirms_a_flac_transfer_and_its_journal_status() {
    let temp = TempDir::new().unwrap();
    let pool = seed_db(&temp, "recovered_flac.db").await;

    let physical = temp.path().join("01 - Recovered Track.flac");
    write_flac(&physical);

    let track_id = seed_recoverable_download(&pool, 9002, &physical).await;

    reconcile_downloads_on_startup(&pool).await;

    let format: Option<String> =
        sqlx::query_scalar("SELECT file_format FROM downloads WHERE track_id = ?")
            .bind(track_id)
            .fetch_one(&pool)
            .await
            .expect("query file_format");
    assert_eq!(format.as_deref(), Some("FLAC"));

    let status: String =
        sqlx::query_scalar("SELECT status FROM operation_journal WHERE operation_id = ?")
            .bind("op-aac-9002")
            .fetch_one(&pool)
            .await
            .expect("query journal status");
    assert_eq!(
        status, "recovered",
        "a reconciliation whose writes landed must be confirmed"
    );

    let queue_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(9002i64)
        .fetch_one(&pool)
        .await
        .expect("query queue status");
    assert_eq!(queue_status, "complete");
}

/// The worker's startup pass must settle orphan `downloading` rows itself, not
/// wait for the detached startup task: otherwise the reset that follows can
/// re-queue a row the reconciliation had just resolved.
#[tokio::test]
async fn startup_pass_settles_orphan_downloading_rows() {
    let temp = TempDir::new().unwrap();
    let pool = seed_db(&temp, "orphan_queue.db").await;

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Orphan', 200000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert track");

    sqlx::query(
        "INSERT INTO download_queue (track_id, service_name, status) VALUES (?, 'qobuz', 'downloading')",
    )
    .bind(tid)
    .execute(&pool)
    .await
    .expect("insert orphan row");

    // Two callers race for the startup pass exactly like `main`'s detached task
    // and the worker do; the process-wide lock must let both finish cleanly.
    let a = {
        let pool = pool.clone();
        tokio::spawn(async move { reconcile_downloads_on_startup(&pool).await })
    };
    let b = {
        let pool = pool.clone();
        tokio::spawn(async move { reconcile_downloads_on_startup(&pool).await })
    };
    a.await.expect("first startup pass");
    b.await.expect("second startup pass");

    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM download_queue WHERE track_id = ?")
            .bind(tid)
            .fetch_all(&pool)
            .await
            .expect("query queue");

    assert_eq!(
        statuses,
        vec!["queued".to_string()],
        "after reconciliation every orphan row must sit in exactly one settled state"
    );
}
