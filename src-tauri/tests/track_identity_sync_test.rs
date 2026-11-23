//! Identidad de pista en el sync (issue #73, ADR-0006 decisión 16).
//!
//! **Caracterización** del enlace anterior a #73, medida sobre
//! `EnrichmentEngine::enrich_and_persist_sync_track` y el esquema migrado de
//! verdad:
//!
//!   * un ISRC idéntico enlaza dos ítems de servicios distintos al mismo Track
//!     y conserva una fuente por servicio;
//!   * el título por sí solo nunca enlaza (mismo título y duración, artista
//!     distinto → dos Tracks);
//!   * sin ISRC no había huella: mismo artista + título + duración duplicaba el
//!     Track. Es el hueco que cierra la huella; la expectativa se invierte abajo
//!     con la decisión nueva y el cambio queda en el diff;
//!   * el esquema no tiene columna de huella de identidad: la identidad se
//!     apoya en `tracks.isrc` (único NOCASE), `track_artists.role`,
//!     `idx_tracks_title_duration` (0032) y el ledger de ediciones (0097).
//!
//! **Contrato nuevo**, verificado contra el mismo entry point del sync: la
//! huella enlaza dentro de la tolerancia y no fuera; dos ISRC distintos nunca
//! se fusionan; lo ambiguo no se fusiona, se importa como Track propio y deja
//! la evidencia de ambos lados en `track_identity_reviews` (0115).
//!
//! **Escala de referencia** (AC5): el fixture sintético usa las cardinalidades
//! de la biblioteca real medidas el 2026-11-09 — 1.557 pistas sin ISRC y 834
//! ítems de Apple sin `track_sources` — sin tocar datos personales (mina de
//! AGENTS: la biblioteca real no se usa en tests).
//!
//! **Upgrade real** de 0114 a 0115 sobre migraciones de verdad, truncadas en
//! 0114 sobre una BD limpia y sembradas antes de aplicar la nueva.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Acquire, Row};
use std::borrow::Cow;
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};
use syncify_tauri_lib::services::track_identity::{
    resolve_identity, IdentityBasis, IdentityOutcome, IdentityRequest,
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

/// AC2: sin ISRC, la huella enlaza al mismo Track cuando artista, título y
/// duración coinciden dentro de la tolerancia (±2 s). Antes de #73 el mismo
/// caso duplicaba el Track; esta expectativa es la que se invierte.
#[tokio::test]
async fn sin_isrc_la_huella_enlaza_dentro_de_la_tolerancia() {
    let pool = setup().await;
    let tidal = service_id(&pool, "tidal").await;
    let qobuz = service_id(&pool, "qobuz").await;
    let acc = account(&pool, tidal).await;
    let qobuz_acc = account(&pool, qobuz).await;
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
                qobuz,
                qobuz_acc,
                TrackFixture {
                    service: "qobuz",
                    item: "qobuz-c",
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

    assert_eq!(
        first.track_id, second.track_id,
        "artista + título + duración dentro de la tolerancia enlazan"
    );
    assert!(!second.is_new_global_track);
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(first.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sources, 2, "el Track enlazado conserva las dos fuentes");

    // Fuera de la tolerancia la misma huella no enlaza.
    let late = engine
        .enrich_and_persist_sync_track(
            &pool,
            input(
                tidal,
                acc,
                TrackFixture {
                    service: "tidal",
                    item: "tidal-e",
                    artist: "Fingerprint Artist",
                    title: "Fingerprint Song",
                    album: "Album",
                    duration_ms: Some(243_000),
                    isrc: None,
                },
            ),
        )
        .await
        .unwrap();
    assert_ne!(
        first.track_id, late.track_id,
        "fuera de la tolerancia no es la misma grabación"
    );
}

/// AC3: dos grabaciones locales con el mismo artista+título+duración y sin
/// ISRC dejan al ítem ambiguo. No se fusiona ninguna: el ítem entra como Track
/// propio y los dos candidatos quedan en la cola con la evidencia de ambos
/// lados. Reintentar el mismo ítem no duplica la revisión.
#[tokio::test]
async fn ambiguo_no_se_fusiona_y_queda_en_revision_con_evidencia() {
    let pool = setup().await;
    let tidal = service_id(&pool, "tidal").await;
    let acc = account(&pool, tidal).await;
    let engine = EnrichmentEngine::new();

    let fixture = |item: &'static str, isrc: Option<&'static str>| {
        input(
            tidal,
            acc,
            TrackFixture {
                service: "tidal",
                item,
                artist: "Ambiguous Artist",
                title: "Ambiguous Song",
                album: "Ambiguous Album",
                duration_ms: Some(200_000),
                isrc,
            },
        )
    };
    // Dos ISRC distintos: son dos grabaciones y no se fusionan entre sí.
    let first = engine
        .enrich_and_persist_sync_track(&pool, fixture("amb-1", Some("ED73AMB00001")))
        .await
        .unwrap();
    let second = engine
        .enrich_and_persist_sync_track(&pool, fixture("amb-2", Some("ED73AMB00002")))
        .await
        .unwrap();
    assert_ne!(first.track_id, second.track_id);

    // El ítem sin ISRC coincide con las dos: ambiguo.
    let ambiguous = engine
        .enrich_and_persist_sync_track(&pool, fixture("amb-3", None))
        .await
        .unwrap();
    assert_ne!(
        ambiguous.track_id, first.track_id,
        "no fusiona con el primero"
    );
    assert_ne!(
        ambiguous.track_id, second.track_id,
        "no fusiona con el segundo"
    );
    assert!(
        ambiguous.is_new_global_track,
        "se importa como Track propio"
    );
    assert_eq!(ambiguous.availability_status, "needs_review");

    let review: (i64, String, String, Option<i64>) = sqlx::query_as(
        "SELECT id, reason, incoming_artist, incoming_duration_ms
         FROM track_identity_reviews WHERE service_track_id = 'amb-3' AND status = 'pending'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(review.1, "fingerprint_multiple_candidates");
    assert_eq!(review.2, "Ambiguous Artist");
    assert_eq!(review.3, Some(200_000));

    let candidates: Vec<(i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT track_id, artist, isrc FROM track_identity_review_candidates
         WHERE review_id = ? ORDER BY track_id",
    )
    .bind(review.0)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(candidates.len(), 2, "la evidencia congela ambos candidatos");
    assert_eq!(candidates[0].0, first.track_id.min(second.track_id));
    assert_eq!(candidates[0].1.as_deref(), Some("Ambiguous Artist"));
    assert!(
        candidates.iter().any(|(_, _, isrc)| isrc.is_some()),
        "el ISRC del candidato forma parte de la evidencia"
    );

    // Reintento del mismo ítem: la evidencia ya enlaza el Track nuevo, no se
    // reabre otra revisión ni se crea otro Track.
    let repeated = engine
        .enrich_and_persist_sync_track(&pool, fixture("amb-3", None))
        .await
        .unwrap();
    assert_eq!(repeated.track_id, ambiguous.track_id);
    let open_reviews: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM track_identity_reviews WHERE status = 'pending'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(open_reviews, 1);
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 3, "dos candidatos y el ítem ambiguo, nada más");
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

/// AC5: el fixture reproduce las cardinalidades medidas en la biblioteca real
/// el 2026-11-09 (1.557 pistas sin ISRC y 834 ítems de Apple sin
/// `track_sources`). No se leen datos personales: las filas son sintéticas y
/// solo se comprueba el contrato de la huella.
///
///   * los 834 ítems de Apple enlazan al Track sin ISRC que ya existe;
///   * 100 ítems de otra edición (misma huella, +30 s) no enlazan;
///   * 3 pares duplicados locales dejan sus ítems en revisión, sin fusionar.
#[tokio::test]
async fn escala_de_referencia_1557_sin_isrc_y_834_de_apple() {
    const LOCAL_TRACKS: i64 = 1_557;
    const APPLE_ITEMS: i64 = 834;
    const OTHER_EDITION_ITEMS: i64 = 100;
    const AMBIGUOUS_PAIRS: i64 = 3;

    let pool = setup().await;
    let apple = service_id(&pool, "apple_music").await;
    let mut conn = pool.acquire().await.unwrap();

    // 1.557 pistas locales sin ISRC, cada una con su artista primario.
    let mut tx = conn.begin().await.unwrap();
    let mut local_ids = Vec::with_capacity(LOCAL_TRACKS as usize);
    for i in 0..LOCAL_TRACKS {
        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                .bind(format!("Reference Artist {i:04}"))
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, duration_ms) VALUES (?, ?) RETURNING id",
        )
        .bind(format!("Reference Song {i:04}"))
        .bind(180_000 + (i % 60) * 1_000)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(&mut *tx)
        .await
        .unwrap();
        local_ids.push(track_id);
    }
    tx.commit().await.unwrap();

    // Los 834 ítems de Apple con la misma huella enlazan al Track existente.
    let mut linked = 0;
    for i in 0..APPLE_ITEMS {
        let artist = format!("Reference Artist {i:04}");
        let title = format!("Reference Song {i:04}");
        let request = IdentityRequest {
            service_id: apple,
            service_track_id: "apple-item",
            account_id: None,
            title: &title,
            artist: &artist,
            album: None,
            duration_ms: Some(180_000 + (i % 60) * 1_000 + (i % 3) * 500),
            isrc: None,
            explicit: None,
        };
        match resolve_identity(&mut conn, &request).await.unwrap() {
            IdentityOutcome::Linked {
                track_id,
                basis: IdentityBasis::Fingerprint,
            } => {
                assert_eq!(track_id, local_ids[i as usize], "ítem {i} enlaza al suyo");
                linked += 1;
            }
            other => panic!("el ítem {i} de Apple debía enlazar por huella: {other:?}"),
        }
    }
    assert_eq!(linked, APPLE_ITEMS);

    // Otra edición de la misma canción: fuera de la tolerancia, Track nuevo.
    for i in 0..OTHER_EDITION_ITEMS {
        let artist = format!("Reference Artist {i:04}");
        let title = format!("Reference Song {i:04}");
        let request = IdentityRequest {
            service_id: apple,
            service_track_id: "apple-item",
            account_id: None,
            title: &title,
            artist: &artist,
            album: None,
            duration_ms: Some(180_000 + (i % 60) * 1_000 + 30_000),
            isrc: None,
            explicit: None,
        };
        assert_eq!(
            resolve_identity(&mut conn, &request).await.unwrap(),
            IdentityOutcome::New,
            "la edición larga del ítem {i} no es la misma grabación"
        );
    }

    for pair in 0..AMBIGUOUS_PAIRS {
        let artist = format!("Ambiguous Pair {pair}");
        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                .bind(&artist)
                .fetch_one(&mut *conn)
                .await
                .unwrap();
        for copy in 0..2 {
            let track_id: i64 = sqlx::query_scalar(
                "INSERT INTO tracks (title, duration_ms) VALUES (?, ?) RETURNING id",
            )
            .bind(format!("Ambiguous Pair Song {pair}"))
            .bind(200_000 + copy * 400)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
            )
            .bind(track_id)
            .bind(artist_id)
            .execute(&mut *conn)
            .await
            .unwrap();
        }
        let title = format!("Ambiguous Pair Song {pair}");
        let request = IdentityRequest {
            service_id: apple,
            service_track_id: "ambiguous-apple-item",
            account_id: None,
            title: &title,
            artist: &artist,
            album: None,
            duration_ms: Some(200_400),
            isrc: None,
            explicit: None,
        };
        let IdentityOutcome::Ambiguous { candidates } =
            resolve_identity(&mut conn, &request).await.unwrap()
        else {
            panic!("el par {pair} tenía que ser ambiguo");
        };
        assert_eq!(candidates.len(), 2, "los dos duplicados son candidatos");
    }

    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(
        tracks,
        LOCAL_TRACKS + AMBIGUOUS_PAIRS * 2,
        "la huella no creó ni fusionó Tracks fuera de lo previsto"
    );
}

/// #73: las ediciones manuales escriben `role='main'` y las filas legacy
/// pueden tener NULL; ambos son primarios para la huella, como en el resto
/// del repo. Sin esto, un Track editado a mano se duplicaba en el siguiente
/// sync.
#[tokio::test]
async fn manual_main_and_null_roles_are_fingerprint_candidates() {
    let pool = setup().await;
    let tidal = service_id(&pool, "tidal").await;
    let acc = account(&pool, tidal).await;
    let engine = EnrichmentEngine::new();

    for (role, item) in [(Some("main"), "manual-main"), (None, "manual-null")] {
        let artist = format!("Role Artist {item}");
        let title = format!("Role Song {item}");
        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists(name) VALUES (?) RETURNING id")
                .bind(&artist)
                .fetch_one(&pool)
                .await
                .unwrap();
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks(title,duration_ms) VALUES (?, 240000) RETURNING id",
        )
        .bind(&title)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_artists(track_id, artist_id, role) VALUES (?, ?, ?)")
            .bind(track_id)
            .bind(artist_id)
            .bind(role)
            .execute(&pool)
            .await
            .unwrap();

        let imported = engine
            .enrich_and_persist_sync_track(
                &pool,
                input(
                    tidal,
                    acc,
                    TrackFixture {
                        service: "tidal",
                        item,
                        artist: &artist,
                        title: &title,
                        album: "Album",
                        duration_ms: Some(240_500),
                        isrc: None,
                    },
                ),
            )
            .await
            .unwrap();
        assert_eq!(imported.track_id, track_id, "role {role:?} debe enlazar");
        assert!(!imported.is_new_global_track);
    }
}

/// #73: una duración 0 o negativa no es evidencia; hay rutas que pasan
/// `Some(0)` sin filtrar y no deben enlazar por artista + título.
#[tokio::test]
async fn zero_or_negative_duration_is_not_fingerprint_evidence() {
    let pool = setup().await;
    let tidal = service_id(&pool, "tidal").await;
    let acc = account(&pool, tidal).await;
    let engine = EnrichmentEngine::new();

    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists(name) VALUES ('No Duration Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks(title,duration_ms) VALUES ('No Duration Song', 240000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists(track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    for duration in [0_i64, -1_i64] {
        let imported = engine
            .enrich_and_persist_sync_track(
                &pool,
                input(
                    tidal,
                    acc,
                    TrackFixture {
                        service: "tidal",
                        item: if duration == 0 {
                            "zero-duration"
                        } else {
                            "negative-duration"
                        },
                        artist: "No Duration Artist",
                        title: "No Duration Song",
                        album: "Album",
                        duration_ms: Some(duration),
                        isrc: None,
                    },
                ),
            )
            .await
            .unwrap();
        assert_ne!(
            imported.track_id, track_id,
            "duración {duration} no puede enlazar"
        );
        assert!(imported.is_new_global_track);
    }
}

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");
const PREVIOUS_VERSION: i64 = 114;
const NEW_VERSION: i64 = 115;

/// El migrador real truncado en `max_version` ejecuta la misma ruta de
/// producción que el arranque (incluida `_sqlx_migrations`). El `MIGRATOR`
/// completo después revalida los checksums ya registrados y aplica 0115.
fn migrator_through(max_version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| migration.version <= max_version)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

/// Memoria con nombre único por pool: `sqlite::memory:` sin nombre comparte
/// espacio entre pools del proceso y este test necesita un esquema PREVIO
/// (0001-0114) limpio.
fn unique_memory_uri() -> String {
    format!(
        "sqlite:file:#73upgrade_{}?mode=memory&cache=shared",
        uuid::Uuid::new_v4().simple()
    )
}

/// Upgrade real de 0114 a 0115: la cola aparece sin tocar pistas, fuentes ni
/// ledger, y el número nuevo no se reutiliza.
#[tokio::test]
async fn upgrade_0115_crea_la_cola_sin_tocar_el_esquema_previo() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&unique_memory_uri())
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    migrator_through(PREVIOUS_VERSION)
        .run(&pool)
        .await
        .expect("migraciones reales hasta 0114");

    // Siembra del esquema vigente antes del upgrade: una pista, su fuente y
    // una ligadura del ledger de ediciones (0097).
    let qobuz: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (id, title, isrc) VALUES (1, 'Legacy Song', 'ED73LEGACY01')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id)
         VALUES (1, ?, 'legacy-item')",
    )
    .bind(qobuz)
    .execute(&pool)
    .await
    .unwrap();
    let item_id: i64 = sqlx::query_scalar(
        "INSERT INTO edition_provider_items (service_id, service_track_id, provenance)
         VALUES (?, 'legacy-item', 'fixture') RETURNING id",
    )
    .bind(qobuz)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO edition_item_tracks (item_id, track_id, validation_status, provenance)
         VALUES (?, 1, 'unknown', 'fixture')",
    )
    .bind(item_id)
    .execute(&pool)
    .await
    .unwrap();

    let tables_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
         AND name IN ('track_identity_reviews', 'track_identity_review_candidates')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables_before, 0, "el esquema previo no conoce la cola");

    MIGRATOR.run(&pool).await.expect("0115 se aplica");

    let applied: (i64, i64) = sqlx::query_as(
        "SELECT MAX(version), COUNT(*) FROM _sqlx_migrations WHERE version IN (114, 115)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        applied,
        (NEW_VERSION, 2),
        "0114 sigue registrada y 0115 es la última"
    );

    let preserved: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM tracks WHERE id = 1),
                (SELECT COUNT(*) FROM track_sources WHERE service_track_id = 'legacy-item'),
                (SELECT COUNT(*) FROM edition_item_tracks WHERE item_id = ?)",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        preserved,
        (1, 1, 1),
        "la pista, su fuente y el ledger sobreviven al upgrade"
    );

    let identity_duration_index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_tracks_duration_identity'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(identity_duration_index, 1, "la huella acota por duración");

    // Una revisión pending por (servicio, ítem, motivo); una decisión cerrada
    // deja abrir otra.
    sqlx::query(
        "INSERT INTO track_identity_reviews (service_id, service_track_id, reason)
         VALUES (?, 'legacy-item', 'fingerprint_multiple_candidates')",
    )
    .bind(qobuz)
    .execute(&pool)
    .await
    .unwrap();
    let duplicate = sqlx::query(
        "INSERT INTO track_identity_reviews (service_id, service_track_id, reason)
         VALUES (?, 'legacy-item', 'fingerprint_multiple_candidates')",
    )
    .bind(qobuz)
    .execute(&pool)
    .await;
    assert!(duplicate.is_err(), "la revisión abierta es única");

    let review_id: i64 = sqlx::query_scalar(
        "SELECT id FROM track_identity_reviews WHERE service_track_id = 'legacy-item'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_identity_review_candidates (review_id, track_id, title, duration_delta_ms)
         VALUES (?, 1, 'Legacy Song', 400)",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE track_identity_reviews
         SET status = 'resolved_separate', resolved_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_identity_reviews (service_id, service_track_id, reason)
         VALUES (?, 'legacy-item', 'fingerprint_multiple_candidates')",
    )
    .bind(qobuz)
    .execute(&pool)
    .await
    .expect("una revisión cerrada no bloquea la siguiente");

    sqlx::query("DELETE FROM track_identity_reviews WHERE id = ?")
        .bind(review_id)
        .execute(&pool)
        .await
        .unwrap();
    let cascaded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_identity_review_candidates WHERE review_id = ?",
    )
    .bind(review_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cascaded, 0, "la evidencia muere con su revisión");
}
