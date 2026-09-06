//! S201 regression tests for the local-playlist listing + M3U export domain.
//!
//! The owner-visible bug: opening an imported playlist failed with
//! `Database error: no column found for name: favorite_at` because
//! `get_local_playlist_tracks` decoded rows into `LibraryTrack` without
//! selecting `t.favorite_at`. `fetch_local_playlist_tracks_page` executes the
//! REAL production SQL against an in-memory schema so any drift between the
//! SELECT column list and the required fields of `LibraryTrack` fails here.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    build_m3u_content, export_playlist_m3u_core, fetch_local_playlist_tracks_page,
};

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    // Schema covering every table/column referenced by
    // fetch_local_playlist_tracks_page (mirrors migrations 0002 + ALTERs).
    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            service_id INTEGER REFERENCES services(id)
        );
        CREATE TABLE artists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE albums (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            cover_art_url TEXT
        );
        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            album_id INTEGER REFERENCES albums(id),
            duration_ms INTEGER,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            isrc TEXT,
            genre TEXT,
            bpm REAL,
            musical_key TEXT,
            release_year INTEGER,
            explicit INTEGER DEFAULT 0,
            musicbrainz_id TEXT,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            favorite_at TEXT,
            display_title TEXT,
            source_title TEXT,
            file_disambiguator TEXT
        );
        CREATE TABLE track_artists (
            track_id INTEGER REFERENCES tracks(id),
            artist_id INTEGER REFERENCES artists(id),
            role TEXT DEFAULT 'primary',
            PRIMARY KEY (track_id, artist_id, role)
        );
        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER REFERENCES tracks(id),
            service_id INTEGER REFERENCES services(id),
            service_track_id TEXT NOT NULL,
            format TEXT,
            availability_status TEXT NOT NULL DEFAULT 'unknown_unchecked',
            UNIQUE(track_id, service_id)
        );
        CREATE TABLE library_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER REFERENCES accounts(id),
            track_id INTEGER REFERENCES tracks(id),
            UNIQUE(account_id, track_id)
        );
        CREATE TABLE playlists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE playlist_tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id INTEGER REFERENCES playlists(id),
            track_id INTEGER REFERENCES tracks(id),
            position INTEGER NOT NULL,
            UNIQUE(playlist_id, track_id)
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            source_service_id INTEGER REFERENCES services(id),
            file_path TEXT NOT NULL,
            file_format TEXT,
            effective_service TEXT,
            file_disambiguator TEXT
        );
        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER REFERENCES tracks(id),
            status TEXT DEFAULT 'queued'
        );
        CREATE TABLE lyrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            content TEXT,
            sync_level TEXT DEFAULT 'none'
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Schema init must succeed");

    pool
}

/// S201 Entregable 1d: the SELECT executed for playlist detail pages MUST
/// return every non-default field of `LibraryTrack`, including
/// `t.favorite_at`. Failing to select it makes sqlx's FromRow decode blow up
/// with `no column found for name: favorite_at` (the owner-reported error).
#[tokio::test]
async fn local_playlist_tracks_page_decodes_library_track_with_favorite_at() {
    let db = create_test_db().await;

    sqlx::query("INSERT INTO playlists (id, name) VALUES (1, 'Mix Importada')")
        .execute(&db)
        .await
        .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, is_favorite, favorite_at) \
         VALUES ('Song A', 180000, 1, '2026-01-02 03:04:05') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Artist A') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, ?, 0)")
        .bind(track_id)
        .execute(&db)
        .await
        .unwrap();

    let page =
        fetch_local_playlist_tracks_page(&db, 1, 0, 50)
            .await
            .expect("playlist page must decode into LibraryTrack");

    assert_eq!(page.len(), 1);
    let track = &page[0];
    assert_eq!(track.id, track_id);
    assert_eq!(track.title, "Song A");
    assert_eq!(track.artist_name.as_deref(), Some("Artist A"));
    assert_eq!(
        track.favorite_at.as_deref(),
        Some("2026-01-02 03:04:05"),
        "favorite_at must be selected and decoded"
    );

    // Pagination contract of the command: second page is empty.
    let page2 = fetch_local_playlist_tracks_page(&db, 1, 1, 50)
        .await
        .expect("offset paging must decode as well");
    assert!(page2.is_empty());
}

/// Ordering follows playlist position (not alphabetical title).
#[tokio::test]
async fn local_playlist_tracks_page_orders_by_playlist_position() {
    let db = create_test_db().await;

    sqlx::query("INSERT INTO playlists (id, name) VALUES (1, 'Ordered')")
        .execute(&db)
        .await
        .unwrap();
    // Insert titles deliberately out of positional order.
    let t_zeta: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Zeta Song', 1000) RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let t_alpha: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Alpha Song', 2000) RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, ?, 0)")
        .bind(t_zeta)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, ?, 1)")
        .bind(t_alpha)
        .execute(&db)
        .await
        .unwrap();

    let page = fetch_local_playlist_tracks_page(&db, 1, 0, 50)
        .await
        .expect("page must decode");

    let titles: Vec<&str> = page.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["Zeta Song", "Alpha Song"]);
}

// ==============================================
// S201 - MODO A: export M3U con verificación stat() real
// ==============================================

/// Inserta una pista + su posición en la playlist y (opcional) su fila en downloads.
async fn seed_playlist_track(
    db: &sqlx::Pool<sqlx::Sqlite>,
    playlist_id: i64,
    position: i64,
    title: &str,
    duration_ms: Option<i64>,
    artist: Option<&str>,
    file_path: Option<&str>,
) -> i64 {
    let track_id: i64 = match duration_ms {
        Some(ms) => sqlx::query_scalar("INSERT INTO tracks (title, duration_ms) VALUES (?, ?) RETURNING id")
            .bind(title)
            .bind(ms)
            .fetch_one(db)
            .await
            .unwrap(),
        None => sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(title)
            .fetch_one(db)
            .await
            .unwrap(),
    };
    if let Some(artist_name) = artist {
        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                .bind(artist_name)
                .fetch_one(db)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(db)
        .await
        .unwrap();
    }
    if let Some(path) = file_path {
        sqlx::query("INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'FLAC')")
            .bind(track_id)
            .bind(path)
            .execute(db)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
        .bind(playlist_id)
        .bind(track_id)
        .bind(position)
        .execute(db)
        .await
        .unwrap();
    track_id
}

/// Modo A happy path: solo las pistas con archivo REAL en disco entran al M3U;
/// las faltantes se reportan con su motivo; formato #EXTM3U/#EXTINF/ruta.
#[tokio::test]
async fn export_m3u_verifies_real_files_and_reports_missing() {
    let db = create_test_db().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let file_a = tmp.path().join("Song One.flac");
    let file_b = tmp.path().join("Song Two.flac");
    std::fs::write(&file_a, b"fakedata-a").unwrap();
    std::fs::write(&file_b, b"fakedata-b").unwrap();

    sqlx::query("INSERT INTO playlists (id, name) VALUES (7, 'Mix Local')")
        .execute(&db)
        .await
        .unwrap();

    seed_playlist_track(&db, 7, 0, "Song One", Some(180_000), Some("Artist One"), Some(file_a.to_str().unwrap())).await;
    // Archivo que NO existe en disco (fila BD huérfana).
    seed_playlist_track(&db, 7, 1, "Ghost Song", Some(200_000), Some("Artist Two"), Some(tmp.path().join("ghost.flac").to_str().unwrap())).await;
    // Sin artista y sin duración -> fallbacks "Unknown" / EXTINF:0.
    seed_playlist_track(&db, 7, 2, "Song Two", None, None, Some(file_b.to_str().unwrap())).await;
    // Sin fila en downloads -> sin_archivo_local.
    seed_playlist_track(&db, 7, 3, "Never Downloaded", Some(95_000), Some("Artist Three"), None).await;

    let result = export_playlist_m3u_core(&db, 7, None)
        .await
        .expect("export core must succeed");

    assert_eq!(result.playlist_name, "Mix Local");
    assert_eq!(result.total_tracks, 4);
    assert_eq!(result.verified_count, 2);
    assert_eq!(result.missing_count, 2);

    let mut reasons: Vec<(&str, &str)> = result
        .missing_tracks
        .iter()
        .map(|m| (m.title.as_str(), m.reason.as_str()))
        .collect();
    reasons.sort();
    assert_eq!(
        reasons,
        vec![
            ("Ghost Song", "archivo_no_encontrado"),
            ("Never Downloaded", "sin_archivo_local"),
        ]
    );

    // Contenido: SOLO verificadas, orden de posición, paridad CLI.
    let content = &result.m3u_content;
    assert!(content.starts_with("#EXTM3U\n"), "header must be #EXTM3U");
    assert!(
        content.contains(&format!("#EXTINF:180,Artist One - Song One\n{}", file_a.display())),
        "entry for verified Song One with absolute path, got:\n{}",
        content
    );
    assert!(
        content.contains(&format!("#EXTINF:0,Unknown - Song Two\n{}", file_b.display())),
        "duration NULL -> 0 and artist fallback 'Unknown', got:\n{}",
        content
    );
    assert!(!content.contains("ghost.flac"), "missing files must not appear in m3u");
    assert!(!content.contains("Never Downloaded"), "unverified entries must be excluded");

    // Orden por posición de playlist: Song One antes que Song Two.
    let idx_one = content.find("Song One").unwrap();
    let idx_two = content.find("Song Two").unwrap();
    assert!(idx_one < idx_two);
}

/// build_m3u_content es determinista y bien formado incluso vacío.
#[tokio::test]
async fn export_m3u_empty_playlist_is_header_only() {
    let db = create_test_db().await;
    sqlx::query("INSERT INTO playlists (id, name) VALUES (9, 'Vacía')")
        .execute(&db)
        .await
        .unwrap();

    let result = export_playlist_m3u_core(&db, 9, None)
        .await
        .expect("empty playlist export must succeed");
    assert_eq!(result.total_tracks, 0);
    assert_eq!(result.verified_count, 0);
    assert_eq!(result.m3u_content, "#EXTM3U\n");

    let rendered = build_m3u_content(&[]);
    assert_eq!(rendered, "#EXTM3U\n");
}

/// Con file_path el contenido verificado se escribe a disco real.
#[tokio::test]
async fn export_m3u_writes_verified_content_to_disk_when_path_given() {
    let db = create_test_db().await;
    let allowed_base = dirs::document_dir()
        .map(|d| {
            if let Ok(cwd) = std::env::current_dir() {
                if cwd.starts_with(&d) {
                    let target = cwd.join("target");
                    if target.exists() {
                        return target;
                    }
                    return cwd;
                }
            }
            if d.join("Syncify/target").exists() {
                d.join("Syncify/target")
            } else {
                d
            }
        })
        .or_else(dirs::download_dir)
        .or_else(dirs::audio_dir)
        .expect("at least one standard directory (docs/downloads/audio) must be resolvable");
    let _ = std::fs::create_dir_all(&allowed_base);
    let tmp = tempfile::Builder::new()
        .prefix("syncify_test_m3u_")
        .tempdir_in(&allowed_base)
        .expect("tempdir in allowed sandbox base");
    let file_a = tmp.path().join("real.flac");
    std::fs::write(&file_a, b"audio").unwrap();

    sqlx::query("INSERT INTO playlists (id, name) VALUES (5, 'Exportable')")
        .execute(&db)
        .await
        .unwrap();
    seed_playlist_track(&db, 5, 0, "Real Track", Some(61_000), Some("A B"), Some(file_a.to_str().unwrap())).await;

    let out_path = tmp.path().join("nested").join("playlist.m3u");
    let result = export_playlist_m3u_core(&db, 5, Some(out_path.to_string_lossy().into_owned()))
        .await
        .expect("write must succeed");

    assert_eq!(result.file_path.as_deref(), Some(out_path.to_str().unwrap()));
    let bytes = result.bytes_written.expect("bytes_written must be set");
    assert_eq!(bytes as usize, result.m3u_content.len());

    let on_disk = std::fs::read_to_string(&out_path).expect("file must exist on disk");
    assert_eq!(on_disk, result.m3u_content);
    assert_eq!(
        on_disk,
        format!("#EXTM3U\n#EXTINF:61,A B - Real Track\n{}\n", file_a.display())
    );
}

/// Protección: si NADA está verificado no se escribe un .m3u inútil/vacíó.
#[tokio::test]
async fn export_m3u_refuses_to_write_file_when_nothing_verified() {
    let db = create_test_db().await;
    let tmp = tempfile::tempdir().expect("tempdir");

    sqlx::query("INSERT INTO playlists (id, name) VALUES (6, 'Todo Falta')")
        .execute(&db)
        .await
        .unwrap();
    seed_playlist_track(
        &db,
        6,
        0,
        "Missing",
        Some(1000),
        Some("X"),
        Some(tmp.path().join("nope.flac").to_str().unwrap()),
    )
    .await;

    let out_path = tmp.path().join("out.m3u");
    let err = export_playlist_m3u_core(&db, 6, Some(out_path.to_string_lossy().into_owned()))
        .await
        .expect_err("must refuse to write when nothing verified");
    assert!(err.contains("Ninguna pista"), "unexpected error: {}", err);
    assert!(!out_path.exists(), "no file may be created");

    // Sin path (dry-run) sí es válido y reporta los conteos.
    let dry = export_playlist_m3u_core(&db, 6, None).await.unwrap();
    assert_eq!((dry.total_tracks, dry.verified_count), (1, 0));
}

/// Playlist inexistente -> error explícito.
#[tokio::test]
async fn export_m3u_unknown_playlist_errors() {
    let db = create_test_db().await;
    let err = export_playlist_m3u_core(&db, 12345, None)
        .await
        .expect_err("unknown playlist must error");
    assert!(err.contains("not found"), "unexpected error: {}", err);
}

/// S201 / TASK-03: Reordering deserialization must accept camelCase payload from frontend.
#[test]
fn playlist_track_position_serde_camel_case() {
    let json_payload = r#"{"trackId": 101, "newPosition": 3}"#;
    let pos: syncify_tauri_lib::commands::PlaylistTrackPosition =
        serde_json::from_str(json_payload).expect("must deserialize camelCase");
    assert_eq!(pos.track_id, 101);
    assert_eq!(pos.new_position, 3);
}

