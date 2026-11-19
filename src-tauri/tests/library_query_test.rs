//! Tests de integración de `query_library` — L01 (issue #98, spec #96).
//!
//! Fixture determinista contra SQLite `:memory:` recién migrada (el fixture
//! base compartido de la serie L02-L12; L00 lo extendió con pertenencias de
//! álbum/artista en `library_memberships` y el materializado
//! `library_added_at`). La costura probada es la función con el pool, no el
//! comando de Tauri.
//!
//! Los planes de consulta se verifican con `query_plan`
//! (EXPLAIN QUERY PLAN): entran por los índices de pertenencia de L00/L01
//! (`idx_library_track`, `idx_playlist_tracks_track`,
//! `idx_library_memberships_*`); este test es el punto de re-verificación.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library_query::{
    query_library, query_plan, CollectionFilter, DownloadStatusFilter, DuplicatesFilter,
    LibraryQuery, LibrarySortKey, QualityBucketFilter, QuarantineMode, SortDirection,
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
/// verificación de `verify_page_files` exige que exista para mantener el
/// estado `downloaded` en la fila devuelta.
fn real_download_file(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("syncify-l01-{}-{tag}.flac", std::process::id()));
    if !path.exists() {
        std::fs::write(&path, b"L01 fixture audio bytes").unwrap();
    }
    path.to_string_lossy().to_string()
}

/// Fixture base compartido de la serie (ver cabecera del fichero).
/// Servicios: los ya sembrados por las migraciones — 1=spotify, 2=qobuz.
/// Cuentas: 1=A1 (spotify), 2=A2 (qobuz).
/// Artistas: 1=Alpha Band, 2=Beta Trio. Álbum: 1=Nightfall.
/// Pistas (una fila por tracks.id en todos los tests):
///   T1 "Sunrise"  — favorita de A1 (le 2026-01-03), source Spotify,
///                   descargada Hi-Res (fichero real), genre Rock, álbum Nightfall
///   T2 "Island"   — SOLO en playlist de A2 (added 2026-01-05), source Qobuz,
///                   genre Jazz, sin duración
///   T3 "River"    — en library_entries de A2 (added NULL), en cola, genre Rock
///   T4 "Ghost"    — is_favorite=1 pero SIN pertenencia viva: no aparece
///   T5 "Echoes"   — en le de A1 y A2 + playlist de A1 (le A1 2026-01-01),
///                   sources Spotify y Qobuz: una fila, dos badges
///   T6 "Storm"    — en le de A1 con added_at '' (centinela), source Spotify,
///                   genre Rock, no descargado
async fn seed() -> SqlitePool {
    let pool = fresh_pool().await;

    // Los servicios 1 (spotify) y 2 (qobuz) ya existen por migración.
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES \
         (1, 1, 'A1', 'a1@x'), (2, 2, 'A2', 'a2@x')",
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
        "INSERT INTO tracks (id, title, album_id, duration_ms, genre, is_favorite, created_at) VALUES \
         (1, 'Sunrise', 1, 200000, 'Rock', 1, '2025-12-01T09:00:00Z'), \
         (2, 'Island', NULL, NULL, 'Jazz', 0, '2025-12-01T09:00:00Z'), \
         (3, 'River', NULL, 180000, 'Rock', 0, '2025-12-01T09:00:00Z'), \
         (4, 'Ghost', NULL, 90000, 'Rock', 1, '2025-12-01T09:00:00Z'), \
         (5, 'Echoes', NULL, 240000, 'Pop', 0, '2025-12-01T09:00:00Z'), \
         (6, 'Storm', NULL, 150000, 'Rock', 0, '2025-12-01T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES \
         (1, 1, 'primary'), (2, 1, 'primary'), (3, 2, 'primary'), \
         (4, 2, 'primary'), (5, 1, 'primary'), (6, 2, 'primary')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Fuentes (badges de servicio): los servicios donde la pista existe.
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) VALUES \
         (1, 1, 'sp-1', 'FLAC', 1, 'available'), \
         (2, 2, 'qb-2', 'FLAC', 1, 'available'), \
         (5, 1, 'sp-5', 'FLAC', 1, 'available'), \
         (5, 2, 'qb-5', 'FLAC', 1, 'available'), \
         (6, 1, 'sp-6', 'MP3', 1, 'available')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Pertenencia viva: favoritos por cuenta.
    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) VALUES \
         (1, 1, '2026-01-03T10:00:00Z', 1), \
         (2, 3, NULL, 1), \
         (1, 5, '2026-01-01T10:00:00Z', 1), \
         (2, 5, NULL, 1), \
         (1, 6, '', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Pertenencia viva: playlists (el dueño de la playlist atribuye cuenta).
    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES \
         (1, 1, 'pl-a1', 'A1 list'), (2, 2, 'pl-a2', 'A2 list')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, added_at) VALUES \
         (2, 2, '2026-01-05T10:00:00Z'), (1, 5, '2026-02-01T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Descargas: T1 descargada Hi-Res (fichero real), T3 en cola.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate) VALUES \
         (1, 1, ?, 'FLAC', 24, 96000)",
    )
    .bind(real_download_file("t1"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (3, 'queued')")
        .execute(&pool)
        .await
        .unwrap();

    // L00 (#97): pertenencia completa por colección. Álbum 'Dawn' para T3
    // (cuenta A2) y colección artista Beta Trio para T6 (cuenta A1): los
    // filtros de colección `album`/`artist` entran por library_memberships.
    sqlx::query("INSERT INTO albums (id, title) VALUES (2, 'Dawn')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO library_memberships (account_id, track_id, collection_type, collection_id, provider_item, added_at) VALUES \
         (2, 3, 'album', 2, 'qb-alb-dawn', NULL), \
         (1, 6, 'artist', 2, 'sp-art-beta', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Materialización de library_added_at (L00): el seed simula lo que el
    // sync/importador hacen en cada escritura de pertenencia.
    for track_id in 1..=6 {
        syncify_tauri_lib::services::library_membership::refresh_track_library_added_at(
            &pool, track_id,
        )
        .await
        .unwrap();
    }

    pool
}

fn q() -> LibraryQuery {
    LibraryQuery::default()
}

/// Ids de la página en el orden devuelto (el orden lo calcula el servidor).
async fn ids(pool: &SqlitePool, query: LibraryQuery) -> Vec<i64> {
    query_library(pool, query)
        .await
        .unwrap()
        .tracks
        .into_iter()
        .map(|t| t.id)
        .collect()
}

// ==========================================================================
// Agregación: una fila por tracks.id y pertenencia viva
// ==========================================================================

#[tokio::test]
async fn una_fila_por_track_y_pertenencia_viva() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();

    // T1,T2,T3,T5,T6: T4 (is_favorite pero sin pertenencia viva) no aparece.
    assert_eq!(result.total, 5);
    let mut ids: Vec<i64> = result.tracks.iter().map(|t| t.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3, 5, 6]);

    // La misma pista en dos cuentas y dos servicios aparece UNA vez con
    // todos sus badges (T5: Spotify y Qobuz).
    let t5 = result.tracks.iter().find(|t| t.id == 5).unwrap();
    let services = t5.services.as_deref().unwrap();
    assert!(services.contains("spotify"), "services={services}");
    assert!(services.contains("qobuz"), "services={services}");

    // Los favoritos no se expanden por release: T1 sale una vez aunque la
    // consulta base agrupe por pertenencia.
    assert_eq!(result.tracks.iter().filter(|t| t.id == 1).count(), 1);
}

// ==========================================================================
// Dimensiones de filtro (AND entre dimensiones, OR dentro de la lista)
// ==========================================================================

#[tokio::test]
async fn dimension_favoritos() {
    let pool = seed().await;
    let mut query = q();
    query.favorites_only = true;
    // T4 es is_favorite pero no tiene pertenencia viva: no entra.
    assert_eq!(ids(&pool, query).await, vec![1]);
}

#[tokio::test]
async fn dimension_servicio_desde_track_sources() {
    let pool = seed().await;
    let mut query = q();
    // Qobuz: T2 y T5 tienen source; T3 (cuenta Qobuz sin source) no entra:
    // los badges de servicio salen de track_sources, no se confunden.
    query.service_ids = vec![2];
    assert_eq!(ids(&pool, query).await, vec![5, 2]);
    let mut query = q();
    query.service_ids = vec![1, 2]; // OR dentro de la lista
    assert_eq!(ids(&pool, query).await, vec![5, 2, 1, 6]);
}

#[tokio::test]
async fn dimension_cuenta_incluye_dueno_de_playlist() {
    let pool = seed().await;
    let mut query = q();
    query.account_ids = vec![1]; // A1: T1, T5, T6 (le) + T5 (playlist suya)
    assert_eq!(ids(&pool, query).await, vec![5, 1, 6]);
    let mut query = q();
    query.account_ids = vec![2]; // A2: T2 (playlist), T3, T5
    assert_eq!(ids(&pool, query).await, vec![5, 2, 3]);
    let mut query = q();
    query.account_ids = vec![1, 2]; // OR dentro de la lista: todas
    assert_eq!(ids(&pool, query).await, vec![5, 2, 1, 3, 6]);
}

#[tokio::test]
async fn cuenta_y_servicio_sobre_la_misma_pertenencia() {
    let pool = seed().await;
    let mut query = q();
    query.account_ids = vec![1];
    query.service_ids = vec![1]; // A1 es cuenta Spotify: T1, T5 y T6
    assert_eq!(ids(&pool, query).await, vec![5, 1, 6]);

    let mut query = q();
    query.account_ids = vec![1];
    query.service_ids = vec![2]; // A1 no tiene pertenencia en Qobuz: aunque
                                 // T5 tenga source Qobuz, la pertenencia de A1 es Spotify — no cuenta.
    assert!(ids(&pool, query).await.is_empty());

    let mut query = q();
    query.account_ids = vec![2];
    query.service_ids = vec![2]; // A2 (Qobuz): T2, T3, T5
    assert_eq!(ids(&pool, query).await, vec![5, 2, 3]);
}

#[tokio::test]
async fn dimension_colecciones() {
    let pool = seed().await;
    let mut query = q();
    query.collections = vec![CollectionFilter::Favorite];
    assert_eq!(ids(&pool, query).await, vec![5, 1, 3, 6]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Playlist];
    assert_eq!(ids(&pool, query).await, vec![5, 2]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Favorite, CollectionFilter::Playlist];
    assert_eq!(ids(&pool, query).await, vec![5, 2, 1, 3, 6]);

    // L00 (#97): `album`/`artist` entran por la pertenencia completa de
    // library_memberships (álbum 'Dawn' para T3, Beta Trio para T6).
    let mut query = q();
    query.collections = vec![CollectionFilter::Album];
    assert_eq!(ids(&pool, query).await, vec![3]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Artist];
    assert_eq!(ids(&pool, query).await, vec![6]);
    let mut query = q();
    query.collections = vec![CollectionFilter::Album, CollectionFilter::Artist];
    // Orden DateAdded DESC: T3 (respaldo created_at 2025-12-01) delante de
    // T6 (le con centinela '' → bloque de nulos al final).
    assert_eq!(ids(&pool, query).await, vec![3, 6]);
}

#[tokio::test]
async fn dimension_genero_or_dentro_de_lista() {
    let pool = seed().await;
    let mut query = q();
    query.genre = vec!["Rock".into(), "Jazz".into()];
    assert_eq!(ids(&pool, query).await, vec![2, 1, 3, 6]);
}

#[tokio::test]
async fn dimension_en_playlist() {
    let pool = seed().await;
    let mut query = q();
    query.in_playlists = Some(true);
    assert_eq!(ids(&pool, query).await, vec![5, 2]);
    let mut query = q();
    query.in_playlists = Some(false);
    assert_eq!(ids(&pool, query).await, vec![1, 3, 6]);
}

#[tokio::test]
async fn dimension_estado_de_descarga() {
    let pool = seed().await;
    let mut query = q();
    query.download_statuses = vec![DownloadStatusFilter::Downloaded];
    assert_eq!(ids(&pool, query).await, vec![1]);
    let mut query = q();
    query.download_statuses = vec![DownloadStatusFilter::Queued];
    assert_eq!(ids(&pool, query).await, vec![3]);
    let mut query = q();
    query.download_statuses = vec![DownloadStatusFilter::NotDownloaded];
    assert_eq!(ids(&pool, query).await, vec![5, 2, 6]);
    let mut query = q();
    query.download_statuses = vec![
        DownloadStatusFilter::Queued,
        DownloadStatusFilter::NotDownloaded,
    ]; // OR dentro de la lista
    assert_eq!(ids(&pool, query).await, vec![5, 2, 3, 6]);
}

#[tokio::test]
async fn dimension_calidad() {
    let pool = seed().await;
    let mut query = q();
    query.quality_buckets = vec![QualityBucketFilter::HiRes];
    assert_eq!(ids(&pool, query).await, vec![1]);
    let mut query = q();
    query.quality_buckets = vec![QualityBucketFilter::Lossy];
    assert!(ids(&pool, query).await.is_empty());
}

#[tokio::test]
async fn texto_se_combina_con_los_filtros() {
    let pool = seed().await;
    let mut query = q();
    query.text = Some("storm".into());
    assert_eq!(ids(&pool, query).await, vec![6]);
    // Por artista y por álbum.
    let mut query = q();
    query.text = Some("Beta".into());
    assert_eq!(ids(&pool, query).await, vec![3, 6]);
    let mut query = q();
    query.text = Some("Nightfall".into());
    assert_eq!(ids(&pool, query).await, vec![1]);
    // Texto + filtro: se combinan (AND), no se sustituye la lista.
    let mut query = q();
    query.text = Some("a".into());
    query.genre = vec!["Jazz".into()];
    assert_eq!(ids(&pool, query).await, vec![2]);
    // L02 (FTS): los caracteres especiales de FTS van escapados; un texto
    // sin tokens de búsqueda ("%" solo de símbolos) se comporta como texto
    // vacío: no filtra y se usa el orden elegido. La entrada del usuario no
    // rompe la consulta.
    let mut query = q();
    query.text = Some("%".into());
    assert_eq!(ids(&pool, query).await, ids(&pool, q()).await);
}

// ==========================================================================
// Orden en servidor
// ==========================================================================

#[tokio::test]
async fn orden_date_added_desc_por_defecto_con_nulos_y_centinelas_al_final() {
    let pool = seed().await;
    // Default: DateAdded DESC sobre el materializado (L00): T5 (playlist
    // 02-01 es la pertenencia MAX), T2 (01-05), T1 (01-03), T3 (sin
    // added_at vivo: respaldo created_at 2025-12-01) y el centinela '' de
    // T6 al final, desempate título NOCASE ASC.
    assert_eq!(ids(&pool, q()).await, vec![5, 2, 1, 3, 6]);
    let mut query = q();
    query.direction = SortDirection::Asc;
    // Ascendente: la fecha más vieja primero (T3, created_at) y el
    // centinela '' de T6 al final (el guard de nulos/centinelas no cambia).
    assert_eq!(ids(&pool, query).await, vec![3, 1, 2, 5, 6]);
}

#[tokio::test]
async fn direccion_solo_en_la_clave_primaria_y_desempates_estables() {
    let pool = seed().await;
    let mut query = q();
    query.sort = LibrarySortKey::Artist;
    query.direction = SortDirection::Desc;
    // Beta Trio (T3, T6) delante de Alpha Band; dentro de cada artista el
    // desempate sigue título NOCASE ASCENDENTE pese a la dirección DESC:
    // River, Storm; Echoes, Island, Sunrise.
    assert_eq!(ids(&pool, query).await, vec![3, 6, 5, 2, 1]);
}

#[tokio::test]
async fn orden_titulo_nocase() {
    let pool = seed().await;
    let mut query = q();
    query.sort = LibrarySortKey::Title;
    query.direction = SortDirection::Desc;
    // Sunrise > Storm > River > Island > Echoes (NOCASE desc).
    assert_eq!(ids(&pool, query).await, vec![1, 6, 3, 2, 5]);
}

#[tokio::test]
async fn orden_duracion_con_nulos_al_final() {
    let pool = seed().await;
    let mut query = q();
    query.sort = LibrarySortKey::Duration;
    query.direction = SortDirection::Asc;
    // T2 sin duración al final; el resto ascendente: Storm(150k), River(180k),
    // Sunrise(200k), Echoes(240k).
    assert_eq!(ids(&pool, query).await, vec![6, 3, 1, 5, 2]);
}

#[tokio::test]
async fn orden_metadata_score() {
    let pool = seed().await;
    let mut query = q();
    query.sort = LibrarySortKey::MetadataScore;
    query.direction = SortDirection::Desc;
    // T1 tiene álbum + ISRC-agnostico: título, artista, álbum, cover y año
    // ausente; debe encabezar a las pistas sin álbum.
    let result = query_library(&pool, query).await.unwrap();
    assert_eq!(result.tracks[0].id, 1);
    assert!(
        result.tracks[0].metadata_score.unwrap()
            > result.tracks.last().unwrap().metadata_score.unwrap()
    );
}

// ==========================================================================
// Paginación
// ==========================================================================

#[tokio::test]
async fn paginacion_offset_limit_y_total_independiente() {
    let pool = seed().await;
    let mut query = q();
    query.limit = Some(2);

    let page1 = query_library(&pool, query.clone()).await.unwrap();
    assert_eq!(page1.total, 5);
    assert!(page1.has_more);
    assert!(page1.counts.is_some());

    let mut query = q();
    query.limit = Some(2);
    query.offset = 2;
    let page2 = query_library(&pool, query.clone()).await.unwrap();
    assert_eq!(page2.total, 5); // el total no depende de la página
    assert!(page2.has_more);
    assert!(page2.counts.is_none()); // al paginar no se recalculan contadores

    let mut query = q();
    query.limit = Some(2);
    query.offset = 4;
    let page3 = query_library(&pool, query).await.unwrap();
    assert_eq!(page3.total, 5);
    assert!(!page3.has_more);

    // Concatenar páginas no duplica ni pierde filas.
    let mut all: Vec<i64> = Vec::new();
    for page in [page1, page2, page3] {
        all.extend(page.tracks.iter().map(|t| t.id));
    }
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 5);
}

#[tokio::test]
async fn limit_se_recorta_a_200() {
    let pool = seed().await;
    let mut query = q();
    query.limit = Some(500);
    let result = query_library(&pool, query).await.unwrap();
    assert_eq!(result.limit, 200);
    let mut query = q();
    query.limit = None;
    let result = query_library(&pool, query).await.unwrap();
    assert_eq!(result.limit, 100); // default
}

// ==========================================================================
// Contadores en servidor
// ==========================================================================

#[tokio::test]
async fn contadores_sin_su_propia_condicion() {
    let pool = seed().await;
    let mut query = q();
    query.account_ids = vec![1]; // A1: T1, T5, T6
    let result = query_library(&pool, query).await.unwrap();
    assert_eq!(result.total, 3);
    let counts = result.counts.as_ref().unwrap();

    // El contador de cuentas IGNORA el filtro de cuentas: cuenta sobre el
    // conjunto sin esa dimensión, así que A2 (3 pistas: T2 playlist, T3 y
    // T5) sigue apareciendo junto a A1 (3 pistas: T1, T5 y T6 — DISTINCT
    // por pista: T5 en le Y en playlist de A1 cuenta UNA).
    let acc: Vec<(i64, i64)> = counts
        .by_account
        .iter()
        .map(|a| (a.account_id, a.count))
        .collect();
    assert!(acc.contains(&(1, 3)), "acc={acc:?}");
    assert!(acc.contains(&(2, 3)), "acc={acc:?}");

    // El contador de servicios no ve la dimensión de servicios (aquí vacía):
    // muestra los dos servicios sobre el conjunto filtrado por cuenta.
    let mut query = q();
    query.service_ids = vec![1]; // Spotify por fuentes: T1, T5, T6
    let result = query_library(&pool, query).await.unwrap();
    assert_eq!(result.total, 3);
    let counts = result.counts.as_ref().unwrap();
    let svc: Vec<(String, i64)> = counts
        .by_service
        .iter()
        .map(|s| (s.service_name.clone(), s.count))
        .collect();
    assert!(svc.contains(&("spotify".into(), 3)), "svc={svc:?}");
    assert!(svc.contains(&("qobuz".into(), 2)), "svc={svc:?}");

    assert_eq!(counts.favorites, 1);
    assert_eq!(counts.downloaded, 1);
    // Solo T5 está en playlist dentro del conjunto {T1, T5, T6}.
    assert_eq!(counts.in_playlists, 1);
}

#[tokio::test]
async fn contadores_por_cuenta_cuentan_pistas_distintas_y_suman_mas_que_el_global() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();
    let counts = result.counts.as_ref().unwrap();
    // Pistas DISTINTAS por cuenta (L00 deduplica las pertenencias espejo):
    // A1: T1 + T5 + T6 = 3 (T5 en le Y en playlist cuenta UNA);
    // A2: T2 + T3 + T5 = 3.
    let acc: Vec<(i64, i64)> = counts
        .by_account
        .iter()
        .map(|a| (a.account_id, a.count))
        .collect();
    assert!(acc.contains(&(1, 3)), "acc={acc:?}");
    assert!(acc.contains(&(2, 3)), "acc={acc:?}");
    // La suma puede superar el total de pistas distintas: la misma pista en
    // dos cuentas cuenta en las dos; la UI no debe presentarla como total.
    assert!(acc.iter().map(|(_, c)| c).sum::<i64>() > result.total);
    assert_eq!(counts.total, result.total);
}

#[tokio::test]
async fn contadores_solo_en_la_primera_pagina() {
    let pool = seed().await;
    let mut query = q();
    query.offset = 2;
    let result = query_library(&pool, query).await.unwrap();
    assert!(result.counts.is_none());
}

// ==========================================================================
// Contrato deshabilitado (no inventado)
// ==========================================================================

#[tokio::test]
async fn cuarentena_y_duplicados_contrato_pendiente() {
    let pool = seed().await;
    // exclude (default): no-op honesto, funciona.
    let mut query = q();
    query.quarantine = QuarantineMode::Exclude;
    assert_eq!(query_library(&pool, query).await.unwrap().total, 5);

    let mut query = q();
    query.quarantine = QuarantineMode::Only;
    let err = query_library(&pool, query).await.unwrap_err();
    assert!(err.contains("L09"), "err={err}");

    let mut query = q();
    query.quarantine = QuarantineMode::Include;
    let err = query_library(&pool, query).await.unwrap_err();
    assert!(err.contains("L09"), "err={err}");

    let mut query = q();
    query.duplicates = Some(DuplicatesFilter::Only);
    let err = query_library(&pool, query).await.unwrap_err();
    assert!(err.contains("L10"), "err={err}");
}

// ==========================================================================
// Plan de consulta (EXPLAIN QUERY PLAN) — punto de re-verificación L00
// ==========================================================================

#[tokio::test]
async fn plan_de_consulta_incluye_la_pertenencia_viva() {
    let pool = seed().await;
    let plan = query_plan(&pool, &q()).await.unwrap().join("\n");
    // La pertenencia viva entra al plan por los índices de pertenencia: los
    // de L01 (library_entries y playlist_tracks) y los de L00 en
    // library_memberships (0112). Este test es la puerta de re-verificación.
    assert!(plan.contains("idx_library_track"), "plan={plan}");
    assert!(plan.contains("idx_playlist_tracks_track"), "plan={plan}");
    assert!(
        plan.contains("idx_library_memberships_track_added")
            || plan.contains("idx_library_memberships_account_track")
            || plan.contains("uq_library_memberships_identity"),
        "plan={plan}"
    );
}

// ==========================================================================
// Contador de cabecera de álbumes (L07, #104)
// ==========================================================================

#[tokio::test]
async fn contador_de_albumes_cuenta_distintos_con_pistas_vivas() {
    let pool = seed().await;
    let result = query_library(&pool, q()).await.unwrap();
    let counts = result.counts.unwrap();

    // El fixture tiene UN álbum con pistas vivas: Nightfall (T1). T3 entra
    // por una pertenencia de álbum de L00 pero su fila `tracks` no tiene
    // `album_id`; el contador cuenta álbumes distintos de la fila, no la
    // colección de la pertenencia, y no lo inventa.
    assert_eq!(counts.albums, 1, "counts={counts:?}");

    // Un filtro que deja pistas sin álbum (T2/T5 en Qobuz) cuenta cero
    // álbumes, no las pistas.
    let mut query = q();
    query.service_ids = vec![2];
    let result = query_library(&pool, query).await.unwrap();
    assert!(result.total > 0);
    assert_eq!(result.counts.unwrap().albums, 0);
}
