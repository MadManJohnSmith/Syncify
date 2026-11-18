//! Tests de la pertenencia de biblioteca por cuenta, track y colección —
//! L00 (issue #97, spec #96).
//!
//! Fixture determinista (los requisitos del issue): dos cuentas del mismo
//! servicio, una de otro, pista compartida entre cuentas, pista SOLO en
//! playlist, pista SOLO en álbum, colaboración de artista, pista descargada,
//! pista en cola de descarga, `added_at` de 2024, 2020, 1970-01-01 y NULL, y
//! las dos formas de cuarentena que #72 marcará: una pista cuyo único
//! soporte es un favorito (cuarentena TOTAL: perderlo la saca de la
//! biblioteca) y una con favorito Y playlist (cuarentena PARCIAL: perder el
//! favorito la deja visible por la playlist).
//!
//! Se ejercita la costura real de escritura (`services/library_membership.rs`
//! y el motor `enrich_and_persist_sync_track`) y la lectura
//! (`query_library` / `get_track_detail`). La prueba de UPGRADE sobre base
//! existente está en `library_membership_migration_test.rs`.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library_query::{
    get_track_detail, query_library, CollectionFilter, LibraryQuery,
};
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};
use syncify_tauri_lib::services::library_membership::{
    insert_membership, record_artist_memberships, record_membership,
    refresh_track_library_added_at, MembershipCollection, MembershipWrite,
};

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

/// Crea el fichero real que el fixture declara como descargado: la
/// verificación de ficheros de la vista exige que exista.
fn real_download_file(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("syncify-l00-{}-{tag}.flac", std::process::id()));
    if !path.exists() {
        std::fs::write(&path, b"L00 fixture audio bytes").unwrap();
    }
    path.to_string_lossy().to_string()
}

/// Pertenencias (cuenta, tipo, colección, added_at) de una pista.
async fn memberships_of(
    pool: &SqlitePool,
    track_id: i64,
) -> Vec<(i64, String, Option<i64>, Option<String>)> {
    sqlx::query_as(
        "SELECT account_id, collection_type, collection_id, added_at \
         FROM library_memberships WHERE track_id = ? ORDER BY account_id, collection_type, collection_id",
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Fecha de añadido materializada de una pista.
async fn library_added_at(pool: &SqlitePool, track_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT library_added_at FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Ids de la página en el orden devuelto.
async fn ids(pool: &SqlitePool, query: LibraryQuery) -> Vec<i64> {
    query_library(pool, query)
        .await
        .unwrap()
        .tracks
        .into_iter()
        .map(|t| t.id)
        .collect()
}

/// Fixture determinista del issue #97.
///
/// Servicios (por migración): 1=spotify, 2=qobuz.
/// Cuentas: 1=A1 (spotify), 2=A2 (spotify — DOS cuentas del mismo servicio),
/// 3=A3 (qobuz — otro servicio).
/// Pistas:
///   T1 "Shared"    — compartida: favorito de A1 (2024) y de A2 (2020)
///   T2 "PlaylistOnly" — SOLO en playlist 1 de A1 (added 2024-06-01)
///   T3 "AlbumOnly"    — SOLO pertenencia de álbum Nightfall (added NULL)
///   T4 "Collab"       — colaboración: Alpha Band (primary) y Beta Trio
///                       (featured); pertenencias de artista por cuenta
///   T5 "Downloaded"   — favorito de A3/qobuz (2020) + descarga real
///   T6 "Queued"       — pertenencia de biblioteca local + en cola
///   T7 "EpochFav"     — favorito con added_at 1970-01-01
///   T8 "NullFav"      — favorito con added_at NULL
///   T9 "QuarantineTotal"   — su ÚNICO soporte es un favorito (2024)
///   T10 "QuarantinePartial" — favorito (2024) Y playlist de A2 (2024)
///   T11 "Orphan"      — fila de tracks SIN pertenencia viva: no aparece
///
/// Las colecciones de artista (A1=Alpha Band, A2=Beta Trio) cubren TODAS
/// las pistas conocidas de cada artista — así escribe el sync tras el
/// favorito de artista.
async fn seed() -> SqlitePool {
    let pool = fresh_pool().await;

    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES \
         (1, 1, 'A1', 'a1@x'), (2, 1, 'A2', 'a2@x'), (3, 2, 'A3', 'a3@x')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Alpha Band'), (2, 'Beta Trio')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Nightfall')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, isrc, created_at) VALUES \
         (1, 'Shared', 1, 200000, 'USUM71700101', '2025-01-01T00:00:00Z'), \
         (2, 'PlaylistOnly', NULL, NULL, NULL, '2025-01-01T00:00:00Z'), \
         (3, 'AlbumOnly', 1, 180000, NULL, '2025-01-01T00:00:00Z'), \
         (4, 'Collab', NULL, 210000, NULL, '2025-01-01T00:00:00Z'), \
         (5, 'Downloaded', NULL, 190000, NULL, '2025-01-01T00:00:00Z'), \
         (6, 'Queued', NULL, 150000, NULL, '2025-01-01T00:00:00Z'), \
         (7, 'EpochFav', NULL, 140000, NULL, '2025-01-01T00:00:00Z'), \
         (8, 'NullFav', NULL, 130000, NULL, '2025-01-01T00:00:00Z'), \
         (9, 'QuarantineTotal', NULL, 120000, NULL, '2025-01-01T00:00:00Z'), \
         (10, 'QuarantinePartial', NULL, 110000, NULL, '2025-01-01T00:00:00Z'), \
         (11, 'Orphan', NULL, 100000, NULL, '2025-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    // Colaboración: Alpha Band primario, Beta Trio featured; el resto con
    // su artista primario para los badges de fila.
    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES \
         (1, 1, 'primary'), (2, 1, 'primary'), (3, 1, 'primary'), \
         (4, 1, 'primary'), (4, 2, 'featured'), \
         (5, 2, 'primary'), (6, 2, 'primary'), (7, 1, 'primary'), \
         (8, 1, 'primary'), (9, 1, 'primary'), (10, 1, 'primary')",
    )
    .execute(&pool)
    .await
    .unwrap();
    // Fuentes (badges de servicio).
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) VALUES \
         (1, 1, 'sp-1', 'FLAC', 1, 'available'), \
         (2, 1, 'sp-2', 'FLAC', 1, 'available'), \
         (3, 2, 'qb-3', 'FLAC', 1, 'available'), \
         (4, 1, 'sp-4', 'FLAC', 1, 'available'), \
         (4, 2, 'qb-4', 'FLAC', 1, 'available'), \
         (5, 2, 'qb-5', 'FLAC', 1, 'available'), \
         (9, 1, 'sp-9', 'MP3', 1, 'available')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Playlists: 1 de A1, 2 de A2.
    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES \
         (1, 1, 'pl-a1', 'A1 list'), (2, 2, 'pl-a2', 'A2 list')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, added_at) VALUES \
         (1, 2, '2024-06-01T10:00:00Z'), (2, 10, '2024-08-01T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Descargas: T5 descargada (fichero real), T6 en cola.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate) VALUES \
         (5, 2, ?, 'FLAC', 24, 96000)",
    )
    .bind(real_download_file("t5"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (6, 'queued')")
        .execute(&pool)
        .await
        .unwrap();

    // Pertenencias (L00). added_at CRUDO: 2024, 2020, 1970-01-01 y NULL.
    for write in [
        // T1 compartida: favorito por cuenta (2024 y 2020).
        MembershipWrite {
            account_id: 1,
            track_id: 1,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-1".into()),
            added_at: Some("2024-03-01T10:00:00Z".into()),
        },
        MembershipWrite {
            account_id: 2,
            track_id: 1,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-1b".into()),
            added_at: Some("2020-01-01T10:00:00Z".into()),
        },
        // T2 SOLO en playlist.
        MembershipWrite {
            account_id: 1,
            track_id: 2,
            collection: MembershipCollection::Playlist,
            collection_id: Some(1),
            provider_item: Some("sp-2".into()),
            added_at: Some("2024-06-01T10:00:00Z".into()),
        },
        // T3 SOLO en álbum (added_at NULL).
        MembershipWrite {
            account_id: 2,
            track_id: 3,
            collection: MembershipCollection::Album,
            collection_id: Some(1),
            provider_item: Some("qb-3".into()),
            added_at: None,
        },
        // T5 favorito de la cuenta del otro servicio (2020).
        MembershipWrite {
            account_id: 3,
            track_id: 5,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("qb-5".into()),
            added_at: Some("2020-02-02T10:00:00Z".into()),
        },
        // T6 importación local del importador.
        MembershipWrite {
            account_id: 1,
            track_id: 6,
            collection: MembershipCollection::Library,
            collection_id: None,
            provider_item: None,
            added_at: Some("2024-09-01T10:00:00Z".into()),
        },
        // T7 favorito con 1970; T8 favorito con NULL.
        MembershipWrite {
            account_id: 1,
            track_id: 7,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-7".into()),
            added_at: Some("1970-01-01T00:00:00Z".into()),
        },
        MembershipWrite {
            account_id: 1,
            track_id: 8,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-8".into()),
            added_at: None,
        },
        // Cuarentena total: T9 cuelga de UN solo favorito.
        MembershipWrite {
            account_id: 1,
            track_id: 9,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-9".into()),
            added_at: Some("2024-10-01T10:00:00Z".into()),
        },
        // Cuarentena parcial: T10 cuelga de favorito Y playlist.
        MembershipWrite {
            account_id: 1,
            track_id: 10,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-10".into()),
            added_at: Some("2024-11-01T10:00:00Z".into()),
        },
        MembershipWrite {
            account_id: 2,
            track_id: 10,
            collection: MembershipCollection::Playlist,
            collection_id: Some(2),
            provider_item: Some("sp-10b".into()),
            added_at: Some("2024-08-01T10:00:00Z".into()),
        },
    ] {
        record_membership(&pool, &write).await.unwrap();
    }

    // Colaboración de artista: la colección artista de A1 (Alpha Band) y la
    // de A2 (Beta Trio) alimentan pertenencia para sus pistas conocidas.
    record_artist_memberships(&pool, 1, 1, Some("sp-art-alpha"))
        .await
        .unwrap();
    record_artist_memberships(&pool, 2, 2, Some("sp-art-beta"))
        .await
        .unwrap();

    pool
}

fn q() -> LibraryQuery {
    LibraryQuery::default()
}

// ==========================================================================
// Visibilidad: pertenencia viva
// ==========================================================================

#[tokio::test]
async fn una_fila_sin_pertenencia_viva_no_aparece_y_las_que_la_tienen_si() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();
    // T11 (sin pertenencia) no aparece; las diez con pertenencia sí.
    let got = result.tracks.iter().map(|t| t.id).collect::<Vec<_>>();
    assert_eq!(result.total, 10, "got={got:?}");
    assert!(
        !got.contains(&11),
        "la fila huérfana no aparece: got={got:?}"
    );
    // La pista SOLO en playlist (T2) y la SOLO en álbum (T3) aparecen.
    assert!(got.contains(&2) && got.contains(&3), "got={got:?}");
}

#[tokio::test]
async fn la_pista_compartida_aparece_una_vez_con_dos_cuentas_del_mismo_servicio() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();
    // La misma grabación produce UNA fila por track aunque dos cuentas del
    // mismo servicio la tengan (no se expande por cuenta ni por release).
    assert_eq!(result.tracks.iter().filter(|t| t.id == 1).count(), 1);
    // Pertenencias: dos favoritos (uno por cuenta) y la colección artista
    // de A1 (Alpha Band); ninguna pertenencia por release.
    let m = memberships_of(&pool, 1).await;
    assert_eq!(
        m.iter()
            .filter(|(_, kind, _, _)| kind == "favorite")
            .count(),
        2,
        "m={m:?}"
    );
    assert!(
        !m.iter().any(|(_, kind, _, _)| kind == "album"),
        "la pertenencia de álbum de T1 no existe: m={m:?}"
    );
}

// ==========================================================================
// La cuenta sale SOLO de las pertenencias
// ==========================================================================

#[tokio::test]
async fn la_cuenta_de_una_pista_sale_solo_de_sus_pertenencias() {
    let pool = seed().await;
    // A2: T1 (favorito), T3 (álbum), T4/T5/T6 (colección artista Beta Trio)
    // y T10 (playlist de A2). Nada de esto se infiere de la cuenta activa:
    // son pertenencias explícitas. Orden DateAdded DESC sobre el
    // materializado (los respaldos created_at de T3/T4 van delante).
    let mut query = q();
    query.account_ids = vec![2];
    let got = ids(&pool, query).await;
    assert_eq!(got, vec![3, 4, 10, 6, 1, 5], "got={got:?}");

    // A3 (otro servicio): solo T5.
    let mut query = q();
    query.account_ids = vec![3];
    assert_eq!(ids(&pool, query).await, vec![5]);

    // La pista SOLO en álbum atribuye cuenta por su pertenencia de álbum.
    let mut query = q();
    query.account_ids = vec![2];
    query.collections = vec![CollectionFilter::Album];
    assert_eq!(ids(&pool, query).await, vec![3]);
}

#[tokio::test]
async fn el_badge_imported_from_incluye_las_colecciones_nuevas() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();
    // T3 no tiene library_entries: su cuenta llega SOLO de las pertenencias
    // nuevas (álbum con A2 y artista con A1, ambas spotify en el fixture).
    // Sin L00 el badge sería NULL.
    let t3 = result.tracks.iter().find(|t| t.id == 3).unwrap();
    assert_eq!(t3.imported_from.as_deref(), Some("spotify"), "t3={t3:?}");
}

// ==========================================================================
// Colecciones: favorito, playlist, álbum y artista
// ==========================================================================

#[tokio::test]
async fn dimension_colecciones_l00() {
    let pool = seed().await;
    let mut query = q();
    query.collections = vec![CollectionFilter::Album];
    // T1 comparte el álbum Nightfall pero su pertenencia de álbum no
    // existe: NO entra por el álbum, entra por favorito.
    assert_eq!(ids(&pool, query).await, vec![3]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Artist];
    // La colección artista de A1 (Alpha Band) cubre sus pistas conocidas
    // y la de A2 (Beta Trio) las suyas; T4 entra por ambas; T11 no tiene
    // relación de artista y no entra.
    assert_eq!(ids(&pool, query).await, vec![3, 4, 7, 8, 10, 9, 6, 2, 1, 5]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Favorite];
    // Orden DateAdded DESC: los respaldos created_at (T7, T8) delante,
    // después 2024, después 2020.
    assert_eq!(ids(&pool, query).await, vec![7, 8, 10, 9, 1, 5]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Playlist];
    assert_eq!(ids(&pool, query).await, vec![10, 2]);
}

#[tokio::test]
async fn los_favoritos_no_se_expanden_por_release() {
    let pool = seed().await;
    // Dos ítems de proveedor de la MISMA grabación (mismo ISRC, distinto
    // release) llegan como favoritos del engine: la pertenencia por cuenta
    // y colección es UNA; los ítems quedan en favorites (Route 2).
    let engine = EnrichmentEngine::new();
    for (item, album) in [
        ("sp-release-a", "Sunrise (Deluxe Edition)"),
        ("sp-release-b", "Sunrise (Remaster)"),
    ] {
        let res = engine
            .enrich_and_persist_sync_track(
                &pool,
                SyncTrackInput {
                    origin_meta: OriginTrackMetadata {
                        title: Some("Shared".into()),
                        artist: Some("Alpha Band".into()),
                        album: Some(album.into()),
                        isrc: Some("USUM71700101".into()),
                        added_at: Some("2024-05-05T10:00:00Z".into()),
                        source_name: "spotify".into(),
                        ..Default::default()
                    },
                    service_track_id: item.into(),
                    service_name: "spotify".into(),
                    service_id: 1,
                    account_id: 1,
                    is_favorite: true,
                    query_musicbrainz: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            res.track_id, 1,
            "el ISRC resuelve al track canónico del fixture"
        );
    }
    let m = memberships_of(&pool, 1).await;
    let favorites = m
        .iter()
        .filter(|(_, kind, _, _)| kind == "favorite")
        .count();
    assert_eq!(
        favorites, 2,
        "A1 y A2 ya tenían su favorito; NO se añade una pertenencia por release: m={m:?}"
    );
}

// ==========================================================================
// library_added_at materializado
// ==========================================================================

#[tokio::test]
async fn library_added_at_materializa_el_maximo_vivo_con_respaldo_created_at() {
    let pool = seed().await;
    // T1: el máximo added_at vivo no nulo y distinto de 1970 es el favorito
    // de 2024 (el de 2020 es menor).
    assert_eq!(
        library_added_at(&pool, 1).await.as_deref(),
        Some("2024-03-01T10:00:00Z")
    );
    // T2: la playlist de 2024-06-01.
    assert_eq!(
        library_added_at(&pool, 2).await.as_deref(),
        Some("2024-06-01T10:00:00Z")
    );
    // T3: el álbum tiene added_at NULL → respaldo created_at.
    assert_eq!(
        library_added_at(&pool, 3).await.as_deref(),
        Some("2025-01-01T00:00:00Z")
    );
    // T7: added_at 1970-01-01 se descarta → respaldo created_at.
    assert_eq!(
        library_added_at(&pool, 7).await.as_deref(),
        Some("2025-01-01T00:00:00Z")
    );
    // T8: added_at NULL se descarta → respaldo created_at.
    assert_eq!(
        library_added_at(&pool, 8).await.as_deref(),
        Some("2025-01-01T00:00:00Z")
    );
    // T5: favorito de 2020 materializa aunque sea antiguo.
    assert_eq!(
        library_added_at(&pool, 5).await.as_deref(),
        Some("2020-02-02T10:00:00Z")
    );
}

#[tokio::test]
async fn library_added_at_se_actualiza_al_cambiar_las_pertenencias() {
    let pool = seed().await;
    assert_eq!(
        library_added_at(&pool, 1).await.as_deref(),
        Some("2024-03-01T10:00:00Z")
    );

    // Cambiar las pertenencias (p. ej. la conciliación de #72 retira el
    // favorito de 2024) y refrescar: el máximo vivo baja al de 2020.
    sqlx::query("DELETE FROM library_memberships WHERE track_id = 1 AND account_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    refresh_track_library_added_at(&pool, 1).await.unwrap();
    assert_eq!(
        library_added_at(&pool, 1).await.as_deref(),
        Some("2020-01-01T10:00:00Z"),
        "el materializado sigue al máximo vivo restante"
    );
}

#[tokio::test]
async fn el_orden_por_fecha_de_anadido_lee_el_materializado() {
    let pool = seed().await;
    // Descendente sobre el materializado: los respaldos created_at
    // (T3, T4, T7, T8 — 2025-01-01, desempate título NOCASE) delante,
    // después las fechas de 2024 por valor, después 2020 (T5).
    let got = ids(&pool, q()).await;
    assert_eq!(got, vec![3, 4, 7, 8, 10, 9, 6, 2, 1, 5], "got={got:?}");
}

// ==========================================================================
// Escritura: el sync y sus costuras
// ==========================================================================

#[tokio::test]
async fn el_engine_escribe_pertenencia_de_favorito_con_added_at_crudo() {
    let pool = seed().await;
    let engine = EnrichmentEngine::new();
    let res = engine
        .enrich_and_persist_sync_track(
            &pool,
            SyncTrackInput {
                origin_meta: OriginTrackMetadata {
                    title: Some("EngineFav".into()),
                    artist: Some("Alpha Band".into()),
                    album: Some("Nightfall".into()),
                    added_at: Some("2024-07-07T10:00:00Z".into()),
                    source_name: "spotify".into(),
                    ..Default::default()
                },
                service_track_id: "sp-engine-1".into(),
                service_name: "spotify".into(),
                service_id: 1,
                account_id: 1,
                is_favorite: true,
                query_musicbrainz: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let m = memberships_of(&pool, res.track_id).await;
    assert_eq!(
        m,
        vec![(
            1,
            "favorite".to_string(),
            None,
            Some("2024-07-07T10:00:00Z".into())
        )],
        "pertenencia de favorito con el added_at CRUDO del proveedor: m={m:?}"
    );
    // El materializado se mantiene en la misma transacción.
    assert_eq!(
        library_added_at(&pool, res.track_id).await.as_deref(),
        Some("2024-07-07T10:00:00Z")
    );
}

#[tokio::test]
async fn el_engine_escribe_pertenencia_de_album_favorito() {
    let pool = seed().await;
    let engine = EnrichmentEngine::new();
    let res = engine
        .enrich_and_persist_sync_track(
            &pool,
            SyncTrackInput {
                origin_meta: OriginTrackMetadata {
                    title: Some("AlbumTrack".into()),
                    artist: Some("Alpha Band".into()),
                    album: Some("Nightfall".into()),
                    source_name: "qobuz".into(),
                    ..Default::default()
                },
                service_track_id: "qb-alb-track-1".into(),
                service_name: "qobuz".into(),
                service_id: 2,
                account_id: 3,
                is_favorite: false,
                album_is_favorite: true,
                album_provider_track_id: Some("qb-alb-99".into()),
                query_musicbrainz: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // La colección álbum guarda el id LOCAL de la fila del álbum que el
    // engine resolvió (un input con id de proveedor no se fusiona por
    // título: crea/resuelve SU fila de álbum y la pertenencia la cita), no
    // el id del proveedor.
    let m = memberships_of(&pool, res.track_id).await;
    assert_eq!(
        m,
        vec![(3, "album".to_string(), res.album_id, None)],
        "pertenencia de álbum por cuenta, con id local de colección: m={m:?}"
    );
    assert!(
        res.album_id.is_some(),
        "el engine resolvió/creó la fila del álbum"
    );
}

#[tokio::test]
async fn la_pertenencia_de_artista_cubre_la_colaboracion_y_es_idempotente() {
    let pool = seed().await;
    // El fixture ya llamó a record_artist_memberships para Alpha Band (A1)
    // y Beta Trio (A2). T4 es colaboración de ambas: tiene pertenencia por
    // cada cuenta/artista; re-llamar no duplica.
    record_artist_memberships(&pool, 1, 1, Some("sp-art-alpha"))
        .await
        .unwrap();
    let m = memberships_of(&pool, 4).await;
    assert_eq!(
        m,
        vec![
            (1, "artist".to_string(), Some(1), None),
            (2, "artist".to_string(), Some(2), None),
        ],
        "colaboración: pertenencia por cada colección de artista: m={m:?}"
    );

    // La colección de artista cubre las pistas YA conocidas (las que tienen
    // relación de artista): T11 existe como fila pero sin relación ni
    // pertenencia — no se inventa una.
    let t11: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_memberships WHERE track_id = 11")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        t11, 0,
        "la pista sin relación de artista no recibe pertenencia"
    );
}

#[tokio::test]
async fn la_identidad_de_la_pertenencia_deduplica_por_cuenta_y_coleccion() {
    let pool = seed().await;
    // Reinsertar el favorito A1 de T1 (mismo par cuenta/track/colección)
    // con OTRO ítem de proveedor no crea una segunda pertenencia.
    insert_membership(
        &pool,
        &MembershipWrite {
            account_id: 1,
            track_id: 1,
            collection: MembershipCollection::Favorite,
            collection_id: None,
            provider_item: Some("sp-otro-item".into()),
            added_at: Some("2025-01-01T00:00:00Z".into()),
        },
    )
    .await
    .unwrap();
    let favorites = memberships_of(&pool, 1)
        .await
        .into_iter()
        .filter(|(_, kind, _, _)| kind == "favorite")
        .count();
    assert_eq!(
        favorites, 2,
        "A1 y A2, una pertenencia por cuenta y colección"
    );

    // La MISMA playlist dos veces tampoco duplica (a diferencia del NULL de
    // favoritos, el id de colección participa de la identidad).
    insert_membership(
        &pool,
        &MembershipWrite {
            account_id: 1,
            track_id: 2,
            collection: MembershipCollection::Playlist,
            collection_id: Some(1),
            provider_item: Some("sp-2".into()),
            added_at: None,
        },
    )
    .await
    .unwrap();
    let playlists = memberships_of(&pool, 2)
        .await
        .into_iter()
        .filter(|(_, kind, _, _)| kind == "playlist")
        .count();
    assert_eq!(playlists, 1);
}

// ==========================================================================
// Cuarentena: los dos patrones que #72 marcará (enganche)
// ==========================================================================

#[tokio::test]
async fn patrones_de_cuarentena_total_y_parcial() {
    let pool = seed().await;
    // Cuarentena TOTAL: T9 cuelga de UN solo favorito (la colección de
    // artista no cuenta: es la misma cuenta) — si #72 lo marca, la pista
    // sale de los listados por defecto.
    let m9 = memberships_of(&pool, 9).await;
    assert_eq!(
        m9.iter()
            .filter(|(_, kind, _, _)| kind == "favorite")
            .count(),
        1,
        "T9 total: m={m9:?}"
    );
    let mut query = q();
    query.account_ids = vec![1];
    query.collections = vec![CollectionFilter::Favorite];
    assert!(ids(&pool, query).await.contains(&9));

    // Cuarentena PARCIAL: T10 cuelga de favorito Y playlist — retirar el
    // favorito la deja visible por la playlist.
    let m10 = memberships_of(&pool, 10).await;
    assert_eq!(
        m10.iter()
            .filter(|(_, kind, _, _)| kind == "favorite")
            .count(),
        1,
        "T10 parcial: m={m10:?}"
    );
    sqlx::query(
        "DELETE FROM library_memberships WHERE track_id = 10 AND collection_type = 'favorite'",
    )
    .execute(&pool)
    .await
    .unwrap();
    refresh_track_library_added_at(&pool, 10).await.unwrap();
    let mut query = q();
    query.collections = vec![CollectionFilter::Playlist];
    assert!(
        ids(&pool, query).await.contains(&10),
        "perder el favorito NO saca a T10: la pertenencia de playlist la sostiene"
    );
}

// ==========================================================================
// Detalle de pista: pertenencias por colección con nombre
// ==========================================================================

#[tokio::test]
async fn el_detalle_muestra_las_colecciones_completas() {
    let pool = seed().await;
    let detail = get_track_detail(&pool, 3).await.unwrap();
    // T3: pertenencia de álbum con su nombre y cuenta A2 (qobuz); nada se
    // infiere de la cuenta activa.
    let album = detail
        .memberships
        .iter()
        .find(|m| m.collection == "album")
        .expect("la pertenencia de álbum aparece en el detalle");
    assert_eq!(album.account_id, 2);
    assert_eq!(album.collection_id, Some(1));
    assert_eq!(album.collection_name.as_deref(), Some("Nightfall"));

    // T4: colaboración — dos pertenencias de artista con el nombre del
    // artista como colección.
    let detail = get_track_detail(&pool, 4).await.unwrap();
    let artists: Vec<&syncify_tauri_lib::commands::library_query::TrackDetailMembership> = detail
        .memberships
        .iter()
        .filter(|m| m.collection == "artist")
        .collect();
    let mut names: Vec<&str> = artists
        .iter()
        .filter_map(|m| m.collection_name.as_deref())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["Alpha Band", "Beta Trio"],
        "artists={artists:?}"
    );
}

// ==========================================================================
// Contadores por cuenta: pistas distintas, sin dobles de los espejos
// ==========================================================================

#[tokio::test]
async fn los_contadores_por_cuenta_cuentan_pistas_distintas() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();
    let counts = result.counts.as_ref().unwrap();
    let acc: Vec<(i64, i64)> = counts
        .by_account
        .iter()
        .map(|a| (a.account_id, a.count))
        .collect();
    // A1: favoritos (T1, T7, T8, T9, T10) + playlist (T2) + biblioteca
    // local (T6) + colección artista Alpha Band (T1-T4, T7-T10) = 9 pistas
    // DISTINCT (T4 y T1 con varias pertenencias cuentan UNA).
    assert!(acc.contains(&(1, 9)), "acc={acc:?}");
    // A2: favorito (T1) + álbum (T3) + playlist (T10) + artista Beta Trio
    // (T4, T5, T6) = 6.
    assert!(acc.contains(&(2, 6)), "acc={acc:?}");
    // A3 (otro servicio): T5 = 1.
    assert!(acc.contains(&(3, 1)), "acc={acc:?}");
}
