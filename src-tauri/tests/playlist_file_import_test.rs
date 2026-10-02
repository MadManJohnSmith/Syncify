//! FE-6 regression tests: import de playlists desde archivo (.m3u/.m3u8/.csv/.txt).
//!
//! Cubre el parser contra fixtures reales y `import_playlist_from_file_core`
//! contra el esquema real de migraciones: matching por ISRC, por ruta de
//! downloads, por título+artista y por título; dedupe intra-import; playlist
//! creada con posiciones 1..N y track_count honesto; entradas sin match se
//! reportan (nunca se inventan pistas).

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    import_playlist_from_file_core, parse_m3u_content, parse_playlist_file, ParsedPlaylistEntry,
};

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/playlists_import")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    let path = fixture_path(name);
    assert!(
        path.is_file(),
        "fixture {} debe existir en el checkout",
        path.display()
    );
    std::fs::read_to_string(&path).expect("no se pudo leer el fixture")
}

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migration failed in test");

    pool
}

/// Siembra: cuenta activa + biblioteca mínima para ejercitar cada vía de matching.
/// Devuelve (track_id_isrc, track_id_path, track_id_title_artist, track_id_title).
async fn seed_library(pool: &sqlx::Pool<sqlx::Sqlite>) -> (i64, i64, i64, i64) {
    // Las migraciones ya siembran services (spotify incluido); no re-insertar.
    let (service_id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'spotify'")
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, is_active, credentials_json) \
         VALUES (1, ?, 'Test User', 1, '{}')",
    )
    .bind(service_id)
    .execute(pool)
    .await
    .unwrap();

    // Track A: matched por ISRC (también por título+artista; el ISRC gana).
    sqlx::query(
        "INSERT INTO tracks (id, title, isrc) VALUES (10, 'One More Time', 'USUM71100999')",
    )
    .execute(pool)
    .await
    .unwrap();
    // Track B: matched por file_path de downloads.
    sqlx::query("INSERT INTO tracks (id, title) VALUES (11, 'Real Path Song')")
        .execute(pool)
        .await
        .unwrap();
    // Track C: solo por título+artista primario.
    sqlx::query("INSERT INTO tracks (id, title) VALUES (12, 'Title Artist Match')")
        .execute(pool)
        .await
        .unwrap();
    // Track D: solo por título (artista del archivo no coincide).
    sqlx::query("INSERT INTO tracks (id, title) VALUES (13, 'Only Title Song')")
        .execute(pool)
        .await
        .unwrap();

    for (track_id, artist_name) in [
        (10, "Daft Punk"),
        (11, "Real Path Artist"),
        (12, "Collab Artist"),
        (13, "Some Other Band"),
    ] {
        sqlx::query("INSERT INTO artists (id, name) VALUES (?, ?)")
            .bind(track_id)
            .bind(artist_name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(track_id)
        .execute(pool)
        .await
        .unwrap();
    }

    // downloads.file_path para el track B (misma relación que el export M3U).
    sqlx::query(
        "INSERT INTO downloads (track_id, file_path) VALUES (11, '/music/library/real_path_song.mp3')",
    )
    .execute(pool)
    .await
    .unwrap();

    (10, 11, 12, 13)
}

#[tokio::test]
async fn test_fixture_m3u_matches_every_mechanism() {
    let pool = create_test_db().await;
    let (isrc_id, path_id, title_artist_id, title_id) = seed_library(&pool).await;

    let content = read_fixture("sample.m3u");
    let entries = parse_m3u_content(&content);
    assert_eq!(
        entries.len(),
        5,
        "fixture m3u: 3 rutas + 1 suelta + 1 EXTINF final sin ruta"
    );
    assert_eq!(entries[0].isrc.as_deref(), Some("USUM71100999"));
    assert!(entries[4].artist.is_none(), "EXTINF final no lleva artista");

    let result = import_playlist_from_file_core(&pool, "sample.m3u", &content, None, None)
        .await
        .expect("import must succeed");

    // Nombre por defecto = stem del archivo.
    assert_eq!(result.playlist_name, "sample");
    assert_eq!(result.total_entries, 5);
    assert_eq!(result.matched_count, 4);
    assert_eq!(
        result.unmatched,
        vec![syncify_tauri_lib::commands::UnmatchedImportedEntry {
            title: "Loose Song".to_string(),
            artist: Some("Loose Artist".to_string()),
        }]
    );

    // La playlist tiene exactamente las 4 pistas matched en orden 1..N.
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC")
            .bind(result.playlist_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![
            (isrc_id, 1),
            (path_id, 2),
            (title_artist_id, 3),
            (title_id, 4),
        ],
        "cada vía de matching debe resolver al track correcto y en orden"
    );

    let (track_count, service_pid): (i64, String) =
        sqlx::query_as("SELECT track_count, service_playlist_id FROM playlists WHERE id = ?")
            .bind(result.playlist_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(track_count, 4, "track_count honesto");
    assert!(
        service_pid.starts_with("file_"),
        "service_playlist_id de import por archivo, got {}",
        service_pid
    );
}

#[tokio::test]
async fn test_fixture_dedupe_within_import() {
    let pool = create_test_db().await;
    seed_library(&pool).await;

    // El ISRC y el título+artista resuelven al MISMO track 10; además la misma
    // línea duplicada. La playlist debe quedar con una sola fila del track 10.
    let content = "#EXTM3U\n\
                   #EXTINF:304,Daft Punk - One More Time\n\
                   # ISRC: USUM71100999\n\
                   /music/library/one_more_time.flac\n\
                   Daft Punk - One More Time\n\
                   Daft Punk - One More Time\n";
    let result = import_playlist_from_file_core(&pool, "dupes.m3u", content, None, None)
        .await
        .expect("import must succeed");

    assert_eq!(result.total_entries, 3);
    assert_eq!(result.matched_count, 1, "3 entradas -> 1 pista única");
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(result.playlist_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_fixture_csv_with_header_and_quotes() {
    let pool = create_test_db().await;
    let (isrc_id, _path_id, title_artist_id, _title_id) = seed_library(&pool).await;

    let content = read_fixture("sample.csv");
    let entries = parse_playlist_file("sample.csv", &content).expect("parse ok");
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries[0].title, "Song, With Comma",
        "los campos entre comillas conservan comas"
    );

    let result = import_playlist_from_file_core(
        &pool,
        "sample.csv",
        &content,
        Some("Desde CSV".into()),
        None,
    )
    .await
    .expect("import must succeed");
    assert_eq!(
        result.playlist_name, "Desde CSV",
        "el nombre explícito gana"
    );
    assert_eq!(result.matched_count, 2, "isrc + título/artista");
    assert_eq!(result.unmatched.len(), 1);

    let first: (i64,) = sqlx::query_as(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC LIMIT 1",
    )
    .bind(result.playlist_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first.0, isrc_id);
    let second: (i64,) = sqlx::query_as(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC LIMIT 1 OFFSET 1",
    )
    .bind(result.playlist_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(second.0, title_artist_id);
}

#[tokio::test]
async fn test_fixture_csv_headerless_and_txt() {
    let pool = create_test_db().await;
    let (_isrc_id, path_id, _title_artist_id, _title_id) = seed_library(&pool).await;

    // CSV sin cabecera: title,artist.
    let csv = read_fixture("headerless.csv");
    let result = import_playlist_from_file_core(&pool, "headerless.csv", &csv, None, None)
        .await
        .expect("import must succeed");
    assert_eq!(result.total_entries, 2);
    assert_eq!(result.matched_count, 1);
    assert_eq!(result.playlist_name, "headerless");

    // TXT: una pista por línea, con comentario y ruta de archivo.
    let txt = read_fixture("sample.txt");
    let entries = parse_playlist_file("sample.txt", &txt).expect("parse ok");
    assert_eq!(entries.len(), 4, "3 títulos + 1 ruta (comentario ignorado)");
    assert_eq!(entries[0].title, "Title Artist Match");
    assert_eq!(
        entries[3].file_path.as_deref(),
        Some("/music/library/real_path_song.mp3")
    );

    let result = import_playlist_from_file_core(&pool, "sample.txt", &txt, None, None)
        .await
        .expect("import must succeed");
    assert_eq!(
        result.matched_count, 3,
        "2 por título(+artista) + 1 por ruta"
    );
    let last: (i64,) = sqlx::query_as(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position DESC LIMIT 1",
    )
    .bind(result.playlist_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(last.0, path_id, "la ruta del txt enlaza via downloads");
}

#[tokio::test]
async fn test_fixture_m3u8_titles_only_and_name_default() {
    let pool = create_test_db().await;
    let (_isrc_id, _path_id, _ta_id, _t_id) = seed_library(&pool).await;

    let content = read_fixture("titles_only.m3u8");
    let entries = parse_m3u_content(&content);
    assert_eq!(entries.len(), 3, "3 líneas de título, sin rutas");
    assert!(entries.iter().all(|e| e.file_path.is_none()));

    let result = import_playlist_from_file_core(&pool, "titles_only.m3u8", &content, None, None)
        .await
        .expect("import must succeed");
    assert_eq!(result.playlist_name, "titles_only");
    assert_eq!(result.total_entries, 3);
    assert_eq!(result.matched_count, 2);
    assert_eq!(result.unmatched.len(), 1);
    assert_eq!(result.unmatched[0].title, "Ghost Track");
}

#[tokio::test]
async fn test_import_errors_are_honest() {
    let pool = create_test_db().await;
    seed_library(&pool).await;

    // Formato no soportado.
    let err = import_playlist_from_file_core(&pool, "list.json", "[]", None, None)
        .await
        .unwrap_err();
    assert!(err.contains("no soportado"), "got: {}", err);

    // Archivo sin pistas.
    let err = import_playlist_from_file_core(&pool, "empty.txt", "\n   \n", None, None)
        .await
        .unwrap_err();
    assert!(err.contains("No se encontraron pistas"), "got: {}", err);

    // Cuenta inexistente.
    let err = import_playlist_from_file_core(
        &pool,
        "sample.txt",
        read_fixture("sample.txt").as_str(),
        None,
        Some(999),
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("No hay ninguna cuenta"),
        "debe explicar que no hay cuenta, got: {}",
        err
    );
}

#[tokio::test]
async fn test_import_without_account_falls_back_to_first_active() {
    let pool = create_test_db().await;
    seed_library(&pool).await;

    let content = "Daft Punk - One More Time\n";
    let result = import_playlist_from_file_core(&pool, "first_active.m3u", content, None, None)
        .await
        .expect("import must succeed");
    let (account_id,): (i64,) = sqlx::query_as("SELECT account_id FROM playlists WHERE id = ?")
        .bind(result.playlist_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(account_id, 1, "usa la primera cuenta activa");
}

#[tokio::test]
async fn test_import_respects_explicit_account() {
    let pool = create_test_db().await;
    seed_library(&pool).await;

    // Segunda cuenta (inactiva) para comprobar que el account_id explícito gana.
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, is_active, credentials_json) VALUES (2, (SELECT id FROM services WHERE name = 'spotify'), 'Other User', 0, '{}')")
        .execute(&pool)
        .await
        .unwrap();

    let content = "Daft Punk - One More Time\n";
    let result = import_playlist_from_file_core(&pool, "explicit.m3u", content, None, Some(2))
        .await
        .expect("import must succeed");
    let (account_id,): (i64,) = sqlx::query_as("SELECT account_id FROM playlists WHERE id = ?")
        .bind(result.playlist_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(account_id, 2, "el account_id explícito se respeta");
}

#[test]
fn test_fixture_parse_rejects_unknown_extension() {
    let err = parse_playlist_file("playlist.json", "[{}]").unwrap_err();
    assert!(err.contains("no soportado"));
}

#[test]
fn test_parsed_entry_shape() {
    // Documenta el contrato serde camelCase consumido por la UI.
    let entry = ParsedPlaylistEntry {
        title: "T".into(),
        artist: Some("A".into()),
        duration_ms: Some(1000),
        isrc: None,
        file_path: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"durationMs\":1000"));
    assert!(json.contains("\"artist\":\"A\""));
}
