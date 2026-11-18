//! Tests de integración del detalle de pista — L04 (issue #101, spec #96).
//!
//! Fixture determinista contra SQLite `:memory:` recién migrada, sobre el
//! fixture base de la serie (`tests/library_query_test.rs`). La costura
//! probada es `get_track_detail` / `delete_track_audio_inner` con el pool,
//! no los comandos de Tauri.
//!
//! Criterios cubiertos (issue #101):
//! - UNA llamada devuelve Track, TrackSource, LibraryItem, descarga y las
//!   facetas de contrato (cuarentena/duplicados `None`: no se simulan);
//! - una pista inexistente es un error tipado, no una fila vacía;
//! - sin consulta por fuente ni por pertenencia (facetas en bloques, no
//!   N+1: las fuentes y pertenencias son consultas únicas por faceta);
//! - borrar audio elimina solo el Download, la cola y el fichero, nunca el
//!   Track, sus TrackSource ni su pertenencia.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library::delete_track_audio_inner;
use syncify_tauri_lib::commands::library_query::{get_track_detail, TrackDetailError};

/// Crea un pool `:memory:` recién migrado.
async fn fresh_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

/// Crea el fichero real que el fixture declara como descargado:
/// `verify_page_files` exige que exista para mantener el estado `downloaded`.
///
/// El nombre lleva un contador además del pid: varios tests de este binario
/// siembran en paralelo y comparten `temp_dir()`, así que reutilizar el mismo
/// fichero hacía que uno borrara el de otro entre `exists` y `remove_file`.
fn real_download_file(tag: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "syncify-l04-{}-{}-{}.flac",
        std::process::id(),
        seq,
        tag
    ));
    std::fs::write(&path, b"L04 fixture audio bytes").unwrap();
    path.to_string_lossy().to_string()
}

/// Fixture (servicios 1=spotify, 2=qobuz ya siembran las migraciones;
/// cuentas 1=A1 spotify, 2=A2 qobuz; álbum 1=Nightfall):
///   T1 "Sunrise" — ficha completa: source Spotify FLAC Hi-Res disponible y
///                  source Qobuz MP3 stale_404, favorita de A1 (2026-01-03),
///                  en playlist de A2 (2026-01-05), descargada con fichero
///                  real y servicio efectivo Qobuz.
///   T2 "Island"  — en cola de descarga, source Qobuz, favorita de A1.
///   T3 "River"   — descargada con file_path que NO existe en disco.
///   T4 "Ghost"   — solo la fila tracks: sin fuentes, pertenencias ni
///                  descarga (la pista mínima que sí existe).
async fn seed() -> SqlitePool {
    let pool = fresh_pool().await;

    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES \
         (1, 1, 'A1', 'a1@x'), (2, 2, 'A2', 'a2@x')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Alpha Band')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO albums (id, title, cover_art_url) VALUES (1, 'Nightfall', 'https://x/cover.jpg')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, isrc, genre, release_year, musicbrainz_id, is_favorite) VALUES \
         (1, 'Sunrise', 1, 200000, 'USABC1234567', 'Rock', 1999, 'mb-rec-1', 1), \
         (2, 'Island', NULL, 180000, NULL, 'Jazz', NULL, NULL, 0), \
         (3, 'River', NULL, 150000, NULL, 'Rock', NULL, NULL, 0), \
         (4, 'Ghost', NULL, 90000, NULL, NULL, NULL, NULL, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (1, 1, 'primary'), (2, 1, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, bitrate, quality_score, available, availability_status, availability_reason) VALUES \
         (1, 1, 'sp-1', 'FLAC', 24, 96000, 4000, 95, 1, 'available', NULL), \
         (1, 2, 'qb-1', 'MP3', NULL, NULL, 320, 60, 0, 'stale_404', 'Provider returned 404'), \
         (2, 2, 'qb-2', 'FLAC', 16, 44100, NULL, 80, 1, 'available', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) VALUES \
         (1, 1, '2026-01-03T10:00:00Z', 1), \
         (1, 2, NULL, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES (1, 2, 'pl-a2', 'A2 list')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, added_at) VALUES (1, 1, '2026-01-05T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, effective_service, file_path, file_format, bit_depth, sample_rate, downloaded_at) VALUES \
         (1, 1, 'qobuz', ?, 'FLAC', 24, 96000, '2026-01-06T09:30:00Z'), \
         (3, 2, NULL, '/nonexistent/path/l04-river.flac', 'FLAC', 16, 44100, '2026-01-07T09:30:00Z')",
    )
    .bind(real_download_file("t1"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (2, 'queued')")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

// ==========================================================================
// UNA llamada con todas las facetas
// ==========================================================================

#[tokio::test]
async fn una_llamada_devuelve_todas_las_facetas() {
    let pool = seed().await;
    let detail = get_track_detail(&pool, 1).await.unwrap();

    // Track canónico: título, artista, álbum, duración, ISRC, género, año y
    // MusicBrainz.
    let track = &detail.track;
    assert_eq!(track.id, 1);
    assert_eq!(track.title, "Sunrise");
    assert_eq!(track.artist_name.as_deref(), Some("Alpha Band"));
    assert_eq!(track.album_name.as_deref(), Some("Nightfall"));
    assert_eq!(track.duration_ms, Some(200000));
    assert_eq!(track.isrc.as_deref(), Some("USABC1234567"));
    assert_eq!(track.genre.as_deref(), Some("Rock"));
    assert_eq!(track.release_year, Some(1999));
    assert_eq!(track.musicbrainz_id.as_deref(), Some("mb-rec-1"));

    // TrackSource por servicio con formato, profundidad, frecuencia,
    // bitrate y disponibilidad. Mejor calidad primero.
    assert_eq!(detail.sources.len(), 2);
    let spotify = &detail.sources[0];
    assert_eq!(spotify.service_name, "spotify");
    assert_eq!(spotify.service_track_id.as_deref(), Some("sp-1"));
    assert_eq!(spotify.format.as_deref(), Some("FLAC"));
    assert_eq!(spotify.bit_depth, Some(24));
    assert_eq!(spotify.sample_rate, Some(96000));
    assert_eq!(spotify.bitrate, Some(4000));
    assert!(spotify.available);
    assert_eq!(spotify.availability_status, "available");
    let qobuz = &detail.sources[1];
    assert_eq!(qobuz.service_name, "qobuz");
    assert_eq!(qobuz.format.as_deref(), Some("MP3"));
    assert_eq!(qobuz.bitrate, Some(320));
    assert!(!qobuz.available);
    assert_eq!(qobuz.availability_status, "stale_404");
    assert_eq!(
        qobuz.availability_reason.as_deref(),
        Some("Provider returned 404")
    );

    // LibraryItem por cuenta y colección: favorito de A1 y playlist de A2.
    // (La faceta viene ordenada por servicio; las aserciones no se acoplan a
    // esa collation.)
    assert_eq!(detail.memberships.len(), 2);
    let favorite = detail
        .memberships
        .iter()
        .find(|m| m.collection == "favorite")
        .unwrap();
    assert_eq!(favorite.account_id, 1);
    assert_eq!(favorite.display_name.as_deref(), Some("A1"));
    assert_eq!(favorite.service_name, "spotify");
    assert_eq!(favorite.collection_id, None);
    assert_eq!(favorite.collection_name, None);
    assert_eq!(favorite.added_at.as_deref(), Some("2026-01-03T10:00:00Z"));
    let playlist = detail
        .memberships
        .iter()
        .find(|m| m.collection == "playlist")
        .unwrap();
    assert_eq!(playlist.account_id, 2);
    assert_eq!(playlist.service_name, "qobuz");
    assert_eq!(playlist.collection_id, Some(1));
    assert_eq!(playlist.collection_name.as_deref(), Some("A2 list"));
    assert_eq!(playlist.added_at.as_deref(), Some("2026-01-05T10:00:00Z"));

    // Descarga: estado, servicio efectivo, calidad y ruta.
    let download = &detail.download;
    assert_eq!(download.status, "downloaded");
    assert_eq!(download.effective_service.as_deref(), Some("qobuz"));
    assert_eq!(download.file_format.as_deref(), Some("FLAC"));
    assert_eq!(download.bit_depth, Some(24));
    assert_eq!(download.sample_rate, Some(96000));
    assert!(download
        .file_path
        .as_deref()
        .unwrap_or("")
        .contains("syncify-l04"));
    assert_eq!(
        download.downloaded_at.as_deref(),
        Some("2026-01-06T09:30:00Z")
    );

    // Facetas de contrato sin backend (#72/#73): None, no simuladas.
    assert!(detail.quarantine.is_none());
    assert!(detail.duplicate_group.is_none());
}

/// La pista mínima (solo fila `tracks`) devuelve facetas vacías, no error.
#[tokio::test]
async fn pista_sin_facetas_devuelve_listas_vacias() {
    let pool = seed().await;
    let detail = get_track_detail(&pool, 4).await.unwrap();
    assert_eq!(detail.track.title, "Ghost");
    assert!(detail.sources.is_empty());
    assert!(detail.memberships.is_empty());
    // La faceta de descarga existe siempre; sin fila ni cola, not_downloaded.
    let download = &detail.download;
    assert_eq!(download.status, "not_downloaded");
    assert_eq!(download.effective_service, None);
    assert_eq!(download.file_path, None);
    assert_eq!(download.file_format, None);
}

#[tokio::test]
async fn en_cola_refleja_el_estado_de_la_lista() {
    let pool = seed().await;
    let detail = get_track_detail(&pool, 2).await.unwrap();
    let download = &detail.download;
    assert_eq!(download.status, "queued");
}

/// Un download cuyo fichero desapareció baja a not_downloaded, igual que en
/// la lista (`verify_page_files`).
#[tokio::test]
async fn fichero_perdido_baja_a_not_downloaded() {
    let pool = seed().await;
    let detail = get_track_detail(&pool, 3).await.unwrap();
    let download = &detail.download;
    assert_eq!(download.status, "not_downloaded");
    // La fila `downloads` sigue existiendo y sus datos se muestran; es el
    // estado el que manda.
    assert_eq!(download.file_format.as_deref(), Some("FLAC"));
}

// ==========================================================================
// Error tipado
// ==========================================================================

#[tokio::test]
async fn pista_inexistente_devuelve_error_tipado() {
    let pool = seed().await;
    let error = get_track_detail(&pool, 999).await.unwrap_err();
    assert_eq!(error, TrackDetailError::TrackNotFound { track_id: 999 });

    // El error cruza el IPC como objeto discriminado por `kind`, no como
    // texto: la UI puede distinguirlo.
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["kind"], "track_not_found");
    assert_eq!(json["trackId"], 999);
}

// ==========================================================================
// Borrar audio: solo Download + cola + fichero
// ==========================================================================

#[tokio::test]
async fn borrar_audio_elimina_solo_el_download() {
    let pool = seed().await;
    let file_path = real_download_file("t1-delete");
    sqlx::query("UPDATE downloads SET file_path = ? WHERE track_id = 1")
        .bind(&file_path)
        .execute(&pool)
        .await
        .unwrap();

    let summary = delete_track_audio_inner(&pool, 1).await.unwrap();
    assert_eq!(summary.queue_entries_deleted, 0);
    assert!(summary.download_deleted);
    assert!(summary.audio_file_deleted);
    assert!(!std::path::Path::new(&file_path).exists());

    // El Track, sus TrackSource y su pertenencia NUNCA se tocan.
    let (title,): (String,) = sqlx::query_as("SELECT title FROM tracks WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, "Sunrise");
    let (sources,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM track_sources WHERE track_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sources, 2);
    let (memberships,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM library_entries WHERE track_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(memberships, 1);

    // La cola ajena no se toca: T2 sigue en cola.
    let (queued,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM download_queue WHERE track_id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(queued, 1);
}

/// Idempotente: una pista sin descarga devuelve el resumen vacío sin error.
#[tokio::test]
async fn borrar_audio_sin_descarga_es_neutro() {
    let pool = seed().await;
    let summary = delete_track_audio_inner(&pool, 4).await.unwrap();
    assert_eq!(summary.queue_entries_deleted, 0);
    assert!(!summary.download_deleted);
    assert!(!summary.audio_file_deleted);
}

/// Repetir el borrado no encuentra nada que borrar (idempotente).
#[tokio::test]
async fn borrar_audio_repetido_es_neutro() {
    let pool = seed().await;
    delete_track_audio_inner(&pool, 1).await.unwrap();
    let summary = delete_track_audio_inner(&pool, 1).await.unwrap();
    assert!(!summary.download_deleted);
    assert!(!summary.audio_file_deleted);
}

// La ausencia de N+1 no se mide desde este fichero (no hay contador de
// consultas fuera del pool): la prueba `una_llamada_devuelve_todas_las_facetas`
// fija que las facetas llegan materializadas en UNA llamada, y el spec de UI
// (`LibraryViewTrackDetailPanel.spec.ts`) cuenta que el panel abre con
// exactamente una invocación de `get_library_track_detail`.
