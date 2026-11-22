//! Identidad de pista en el sync (issue #73, ADR-0006 decisión 16).
//!
//! Primera mitad: **caracterización** del enlace anterior a #73, medida sobre
//! `EnrichmentEngine::enrich_and_persist_sync_track` y el esquema migrado de
//! verdad.
//!
//!   * un ISRC idéntico enlaza dos ítems de servicios distintos al mismo Track
//!     y conserva una fuente por servicio;
//!   * el título por sí solo nunca enlaza (mismo título y duración, artista
//!     distinto → dos Tracks);
//!   * sin ISRC no había huella: mismo artista + título + duración duplicaba el
//!     Track. Es el hueco que cierra la huella; el test se invierte cuando
//!     aterriza la decisión de identidad, y ese cambio queda en el diff.
//!   * el esquema no tiene columna de huella: la identidad se apoya en
//!     `tracks.isrc` (único NOCASE), `track_artists.role`,
//!     `idx_tracks_title_duration` (0032) y el ledger de ediciones (0097).

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};

async fn setup() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

async fn service_id(pool: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn account(pool: &sqlx::SqlitePool, sid: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO accounts(service_id, display_name) VALUES (?, 'fixture') RETURNING id",
    )
    .bind(sid)
    .fetch_one(pool)
    .await
    .unwrap()
}

struct TrackFixture<'a> {
    service: &'a str,
    item: &'a str,
    artist: &'a str,
    title: &'a str,
    album: &'a str,
    duration_ms: Option<i64>,
    isrc: Option<&'a str>,
}

fn input(sid: i64, account_id: i64, fixture: TrackFixture<'_>) -> SyncTrackInput {
    SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some(fixture.title.to_string()),
            artist: Some(fixture.artist.to_string()),
            album: Some(fixture.album.to_string()),
            isrc: fixture.isrc.map(str::to_owned),
            source_name: fixture.service.to_string(),
            ..Default::default()
        },
        service_track_id: fixture.item.to_string(),
        service_name: fixture.service.to_string(),
        service_id: sid,
        account_id,
        include_appearances: false,
        is_favorite: false,
        is_purchased: false,
        format: None,
        bit_depth: None,
        sample_rate: None,
        quality_score: None,
        audio_quality: None,
        cover_art_url: None,
        duration_ms: fixture.duration_ms,
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    }
}

/// AC1: un ISRC idéntico enlaza automáticamente y el Track conserva las dos
/// fuentes, una por servicio.
#[tokio::test]
async fn caracterizacion_isrc_identico_enlaza_y_conserva_las_dos_fuentes() {
    let pool = setup().await;
    let qobuz = service_id(&pool, "qobuz").await;
    let tidal = service_id(&pool, "tidal").await;
    let qobuz_account = account(&pool, qobuz).await;
    let tidal_account = account(&pool, tidal).await;
    let engine = EnrichmentEngine::new();

    let first = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                qobuz,
                qobuz_account,
                TrackFixture {
                    service: "qobuz",
                    item: "qobuz-1",
                    artist: "The Same Artist",
                    title: "Same Recording",
                    album: "Shared Album",
                    duration_ms: Some(201_000),
                    isrc: Some("ED73ISRC0001"),
                },
            ),
        )
        .await
        .expect("el primer ítem entra");

    let second = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                tidal,
                tidal_account,
                TrackFixture {
                    service: "tidal",
                    item: "tidal-1",
                    artist: "The Same Artist",
                    title: "Same Recording",
                    album: "Shared Album",
                    duration_ms: Some(201_400),
                    isrc: Some("ED73ISRC0001"),
                },
            ),
        )
        .await
        .expect("el segundo ítem entra");

    assert_eq!(
        first.track_id, second.track_id,
        "el ISRC idéntico enlaza el mismo Track"
    );
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(first.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sources, 2, "cada servicio conserva su fuente");
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 1, "no se duplica el Track");
}

/// AC4: el título por sí solo nunca enlaza. Mismo título y misma duración, pero
/// otro artista, son dos grabaciones distintas.
#[tokio::test]
async fn caracterizacion_el_titulo_por_si_solo_no_enlaza() {
    let pool = setup().await;
    let tidal = service_id(&pool, "tidal").await;
    let acc = account(&pool, tidal).await;
    let engine = EnrichmentEngine::new();

    let first = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                tidal,
                acc,
                TrackFixture {
                    service: "tidal",
                    item: "tidal-a",
                    artist: "Artist One",
                    title: "Shared Title",
                    album: "Album One",
                    duration_ms: Some(180_000),
                    isrc: None,
                },
            ),
        )
        .await
        .unwrap();
    let second = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                tidal,
                acc,
                TrackFixture {
                    service: "tidal",
                    item: "tidal-b",
                    artist: "Artist Two",
                    title: "Shared Title",
                    album: "Album Two",
                    duration_ms: Some(180_000),
                    isrc: None,
                },
            ),
        )
        .await
        .unwrap();

    assert_ne!(
        first.track_id, second.track_id,
        "el título no es evidencia de identidad"
    );
}

/// Hueco caracterizado antes de #73: sin ISRC no había huella y el mismo
/// artista + título + duración se importaba como un Track nuevo. La huella
/// (AC2) invierte esta expectativa; el resto de la caracterización no cambia.
#[tokio::test]
async fn caracterizacion_sin_isrc_la_huella_no_existia_y_duplicaba() {
    let pool = setup().await;
    let tidal = service_id(&pool, "tidal").await;
    let acc = account(&pool, tidal).await;
    let engine = EnrichmentEngine::new();

    let first = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                tidal,
                acc,
                TrackFixture {
                    service: "tidal",
                    item: "tidal-c",
                    artist: "Fingerprint Artist",
                    title: "Fingerprint Song",
                    album: "Album",
                    duration_ms: Some(240_000),
                    isrc: None,
                },
            ),
        )
        .await
        .unwrap();
    let second = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                tidal,
                acc,
                TrackFixture {
                    service: "tidal",
                    item: "tidal-d",
                    artist: "Fingerprint Artist",
                    title: "Fingerprint Song",
                    album: "Album",
                    duration_ms: Some(240_500),
                    isrc: None,
                },
            ),
        )
        .await
        .unwrap();

    assert_ne!(
        first.track_id, second.track_id,
        "sin huella, el enlace previo a #73 no enlazaba"
    );
    let same_title: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE title = 'Fingerprint Song'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(same_title, 2, "el ítem se duplicaba en otro Track");
}

/// El esquema del que parte la huella. No hay columna nueva que justifique una
/// migración por sí sola: la evidencia ya vive en `tracks`, `track_artists` y
/// los índices de 0032/0064. La de 0115 solo añade la cola de revisión.
#[tokio::test]
async fn caracterizacion_el_esquema_no_tiene_columna_de_huella() {
    let pool = setup().await;

    let columns: Vec<String> = sqlx::query("SELECT name FROM pragma_table_info('tracks')")
        .map(|row: sqlx::sqlite::SqliteRow| row.get::<String, _>("name"))
        .fetch_all(&pool)
        .await
        .unwrap();
    // `acoustid_fingerprint` es una huella acústica de análisis, no la huella
    // de identidad del sync: no hay columna de identidad que migrar.
    assert!(
        columns.iter().any(|name| name == "acoustid_fingerprint"),
        "la huella acústica existente es otra cosa"
    );
    assert!(
        !columns
            .iter()
            .any(|name| name == "fingerprint" || name == "identity_fingerprint"),
        "tracks no tiene columna de huella de identidad: se calcula"
    );
    assert!(columns.iter().any(|name| name == "isrc"));
    assert!(columns.iter().any(|name| name == "duration_ms"));

    let isrc_index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_tracks_isrc_unique' AND sql LIKE '%COLLATE NOCASE%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(isrc_index, 1, "el ISRC es único sin distinguir mayúsculas");

    let title_duration_index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_tracks_title_duration'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        title_duration_index, 1,
        "0032 ya indexa título + duración para las pistas sin ISRC"
    );

    let role_default: Option<String> = sqlx::query_scalar(
        "SELECT dflt_value FROM pragma_table_info('track_artists') WHERE name = 'role'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        role_default.as_deref(),
        Some("'primary'"),
        "el artista primario es la columna que compara la huella"
    );
}
