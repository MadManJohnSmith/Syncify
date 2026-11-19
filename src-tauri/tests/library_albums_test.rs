//! Tests de integración de `query_library_albums` — L07 (issue #104, spec #96).
//!
//! Fixture determinista contra SQLite `:memory:` recién migrada, con la misma
//! forma que el fixture base de la serie (`tests/library_query_test.rs`). La
//! costura probada es la función con el pool
//! (`query_library_albums_inner`), no el comando de Tauri.
//!
//! Criterios cubiertos (issue #104):
//! - una fila por álbum con artista principal, año, pistas vivas,
//!   descargadas, cobertura, cuentas y servicios;
//! - la cobertura no inventa porcentaje con total cero;
//! - filtros por texto, cuenta, servicio y cobertura; órdenes por título,
//!   artista, año y fecha añadida;
//! - un álbum sin pistas vivas no aparece;
//! - paginación en servidor y total de álbumes distintos;
//! - badges agregados por página (sin N+1) y plan de consulta por índices;
//! - el detalle `/album/:id` conserva TODAS las pistas (sin truncar).

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library::fetch_album;
use syncify_tauri_lib::commands::library_albums::{
    album_coverage, album_query_plan, query_library_albums_inner, AlbumCoverageFilter,
    LibraryAlbumQuery, LibraryAlbumSortKey,
};
use syncify_tauri_lib::commands::library_query::SortDirection;

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

/// Fichero real para que la pista cuente como descargada con fichero.
fn real_download_file(tag: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "syncify-l07-{}-{}-{tag}.flac",
        std::process::id(),
        seq
    ));
    std::fs::write(&path, b"L07 fixture audio bytes").unwrap();
    path.to_string_lossy().to_string()
}

/// Fixture (servicios 1=spotify, 2=qobuz ya sembrados por las migraciones;
/// cuentas 1=A1 spotify, 2=A2 qobuz):
///
/// Álbumes vivos:
///   1 "Nightfall" (1999, Alpha Band): T1 (A1 favorito, descargada),
///     T2 (A2 playlist), T3 (A1 library_membership, en cola) — 3 pistas,
///     1 descargada; cuentas A1+A2; servicios spotify+qobuz.
///   2 "Dawn" (2005, Beta Trio): T4 (A2 membership) y T5 (A2 favorito) —
///     2 pistas, 0 descargadas; servicios qobuz+spotify.
///   3 "Unreleased" (sin release_date; año de pista 2012, Gamma Ray):
///     T9 (A1 favorito) — 1 pista, 0 descargadas.
///   4 "Full Coverage" (2010, Gamma Ray): T7 (A1) y T8 (A2) descargadas —
///     100 % de cobertura.
///
/// Álbumes que NO aparecen:
///   5 "Ghost Album": solo T6, que es `is_favorite` pero no tiene ninguna
///     pertenencia viva.
///   6 "Empty Album": sin pistas.
async fn seed() -> SqlitePool {
    let pool = fresh_pool().await;

    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES \
         (1, 1, 'A1', 'a1@x'), (2, 2, 'A2', 'a2@x')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO artists (id, name) VALUES \
         (1, 'Alpha Band'), (2, 'Beta Trio'), (3, 'Gamma Ray')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO albums (id, title, release_date, cover_art_url) VALUES \
         (1, 'Nightfall', '1999-05-01', 'https://x/nightfall.jpg'), \
         (2, 'Dawn', '2005-01-01', NULL), \
         (3, 'Unreleased', NULL, NULL), \
         (4, 'Full Coverage', '2010-01-01', NULL), \
         (5, 'Ghost Album', '2000-01-01', NULL), \
         (6, 'Empty Album', '2011-01-01', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Artista principal del álbum: la fuente preferente de la fila.
    sqlx::query(
        "INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES \
         (1, 1, 1), (2, 2, 1), (4, 3, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, release_year, is_favorite, created_at) VALUES \
         (1, 'Sunrise', 1, 200000, 1999, 1, '2025-12-01T09:00:00Z'), \
         (2, 'Moonlight', 1, 210000, 1999, 0, '2025-12-01T09:00:00Z'), \
         (3, 'Starlight', 1, 220000, 1999, 0, '2025-12-01T09:00:00Z'), \
         (4, 'Island', 2, 180000, 2005, 0, '2025-12-01T09:00:00Z'), \
         (5, 'River', 2, 190000, 2005, 0, '2025-12-01T09:00:00Z'), \
         (6, 'Ghost', 5, 90000, 2000, 1, '2025-12-01T09:00:00Z'), \
         (7, 'One', 4, 170000, 2010, 0, '2025-12-01T09:00:00Z'), \
         (8, 'Two', 4, 175000, 2010, 0, '2025-12-01T09:00:00Z'), \
         (9, 'Yearless', 3, 160000, 2012, 0, '2025-12-01T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES \
         (1, 1, 'primary'), (2, 1, 'primary'), (3, 1, 'primary'), \
         (4, 2, 'primary'), (5, 2, 'primary'), (6, 3, 'primary'), \
         (7, 3, 'primary'), (8, 3, 'primary'), (9, 3, 'primary')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) VALUES \
         (1, 1, 'sp-1', 'FLAC', 1, 'available'), \
         (2, 2, 'qb-2', 'FLAC', 1, 'available'), \
         (3, 1, 'sp-3', 'FLAC', 1, 'available'), \
         (3, 2, 'qb-3', 'FLAC', 1, 'available'), \
         (4, 2, 'qb-4', 'FLAC', 1, 'available'), \
         (5, 1, 'sp-5', 'MP3', 1, 'available'), \
         (7, 1, 'sp-7', 'FLAC', 1, 'available'), \
         (8, 2, 'qb-8', 'FLAC', 1, 'available'), \
         (9, 1, 'sp-9', 'FLAC', 1, 'available')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Pertenencia viva: favoritos por cuenta, playlist con dueño y
    // library_memberships (L00) para álbum.
    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) VALUES \
         (1, 1, '2026-03-01T10:00:00Z', 1), \
         (2, 5, NULL, 1), \
         (1, 7, '2026-01-01T10:00:00Z', 1), \
         (1, 9, '2025-12-01T10:00:00Z', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES \
         (1, 1, 'pl-a1', 'A1 list'), (2, 2, 'pl-a2', 'A2 list')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, added_at) VALUES \
         (2, 2, '2026-02-15T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_memberships (account_id, track_id, collection_type, collection_id, provider_item, added_at) VALUES \
         (1, 3, 'album', 1, 'sp-alb-nightfall', '2026-01-15T10:00:00Z'), \
         (2, 4, 'album', 2, 'qb-alb-dawn', '2026-02-01T10:00:00Z'), \
         (2, 8, 'album', 4, 'qb-alb-full', '2026-01-02T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Descargas: T1/T7/T8 con fichero; T3 en cola (no cuenta como cobertura).
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES \
         (1, 1, ?, 'FLAC'), (7, 1, ?, 'FLAC'), (8, 2, ?, 'FLAC')",
    )
    .bind(real_download_file("t1"))
    .bind(real_download_file("t7"))
    .bind(real_download_file("t8"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (3, 'queued')")
        .execute(&pool)
        .await
        .unwrap();

    for track_id in 1..=9 {
        syncify_tauri_lib::services::library_membership::refresh_track_library_added_at(
            &pool, track_id,
        )
        .await
        .unwrap();
    }

    pool
}

fn q() -> LibraryAlbumQuery {
    LibraryAlbumQuery::default()
}

async fn album_ids(pool: &SqlitePool, query: LibraryAlbumQuery) -> Vec<i64> {
    query_library_albums_inner(pool, query)
        .await
        .unwrap()
        .albums
        .into_iter()
        .map(|album| album.album_id)
        .collect()
}

// ==========================================================================
// Una fila por álbum con sus contadores y badges
// ==========================================================================

#[tokio::test]
async fn una_fila_por_album_y_solo_albumes_con_pistas_vivas() {
    let pool = seed().await;
    let result = query_library_albums_inner(&pool, q()).await.unwrap();

    // Álbumes 1, 2, 3 y 4 tienen pistas vivas. El 5 (Ghost) solo tiene una
    // pista is_favorite sin pertenencia y el 6 no tiene pistas: no salen.
    assert_eq!(result.total, 4);
    let mut ids: Vec<i64> = result.albums.iter().map(|a| a.album_id).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3, 4]);

    let nightfall = result.albums.iter().find(|a| a.album_id == 1).unwrap();
    assert_eq!(nightfall.title, "Nightfall");
    assert_eq!(nightfall.artist_name.as_deref(), Some("Alpha Band"));
    assert_eq!(nightfall.release_year, Some(1999));
    assert_eq!(nightfall.track_count, 3);
    assert_eq!(nightfall.downloaded_count, 1);
    let ratio = nightfall.coverage.unwrap();
    assert!((ratio - 1.0 / 3.0).abs() < 1e-9, "coverage={ratio}");
    assert_eq!(
        nightfall.date_added.as_deref(),
        Some("2026-03-01T10:00:00Z")
    );

    // Cuentas: A2 (qobuz) antes que A1 (spotify), igual que los contadores
    // de cuenta de L01; una vez cada una aunque aporten varias pistas.
    let accounts: Vec<(&str, i64)> = nightfall
        .accounts
        .iter()
        .map(|a| (a.display_name.as_deref().unwrap_or(""), a.account_id))
        .collect();
    assert_eq!(accounts, vec![("A2", 2), ("A1", 1)]);

    // Servicios: spotify y qobuz, una vez cada uno pese a las tres fuentes.
    let services: Vec<&str> = nightfall
        .services
        .iter()
        .map(|s| s.service_name.as_str())
        .collect();
    assert_eq!(services, vec!["qobuz", "spotify"]);

    // La descarga en cola de T3 no cuenta como descargada.
    let dawn = result.albums.iter().find(|a| a.album_id == 2).unwrap();
    assert_eq!(dawn.track_count, 2);
    assert_eq!(dawn.downloaded_count, 0);
    assert_eq!(dawn.coverage, Some(0.0));
    assert_eq!(dawn.accounts.len(), 1);
    assert_eq!(dawn.accounts[0].account_id, 2);

    let full = result.albums.iter().find(|a| a.album_id == 4).unwrap();
    assert_eq!(full.downloaded_count, 2);
    assert_eq!(full.coverage, Some(1.0));
}

#[tokio::test]
async fn cobertura_cero_no_inventa_porcentaje_con_total_cero() {
    // Con total cero no hay porcentaje que mostrar: None, nunca 0 %.
    assert_eq!(album_coverage(0, 0), None);
    assert_eq!(album_coverage(0, -1), None);
    // Con total > 0, cero descargadas es un 0 % honesto.
    assert_eq!(album_coverage(0, 3), Some(0.0));
}

// ==========================================================================
// Filtros (texto, cuenta, servicio, cobertura)
// ==========================================================================

#[tokio::test]
async fn filtra_por_texto_como_la_consulta_de_pistas() {
    let pool = seed().await;
    let mut query = q();
    query.text = Some("Dawn".into());
    assert_eq!(album_ids(&pool, query).await, vec![2]);

    let mut query = q();
    query.text = Some("Sunrise".into());
    assert_eq!(album_ids(&pool, query).await, vec![1]);
}

#[tokio::test]
async fn filtra_por_cuenta_y_servicio_con_la_misma_pertenencia() {
    let pool = seed().await;
    let mut query = q();
    query.account_ids = vec![1];
    // A1 aporta a los álbumes 1 (T1/T3), 4 (T7) y 3 (T9).
    assert_eq!(album_ids(&pool, query).await, vec![1, 4, 3]);

    let mut query = q();
    query.service_ids = vec![2];
    // Fuentes Qobuz: T2 (álbum 1), T4 (álbum 2), T8 (álbum 4).
    assert_eq!(album_ids(&pool, query).await, vec![1, 2, 4]);

    // Cuenta + servicio sobre la MISMA pertenencia: T3 tiene source Qobuz
    // pero su pertenencia de A1 es Spotify; A1 no está en Qobuz.
    let mut query = q();
    query.account_ids = vec![1];
    query.service_ids = vec![2];
    assert!(album_ids(&pool, query).await.is_empty());
}

#[tokio::test]
async fn filtra_por_cobertura() {
    let pool = seed().await;
    let mut query = q();
    query.coverage = vec![AlbumCoverageFilter::Complete];
    assert_eq!(album_ids(&pool, query).await, vec![4]);

    let mut query = q();
    query.coverage = vec![AlbumCoverageFilter::Partial];
    assert_eq!(album_ids(&pool, query).await, vec![1]);

    let mut query = q();
    query.coverage = vec![AlbumCoverageFilter::None];
    assert_eq!(album_ids(&pool, query).await, vec![2, 3]);

    // OR dentro de la lista.
    let mut query = q();
    query.coverage = vec![AlbumCoverageFilter::Complete, AlbumCoverageFilter::Partial];
    assert_eq!(album_ids(&pool, query).await, vec![1, 4]);
}

// ==========================================================================
// Orden
// ==========================================================================

#[tokio::test]
async fn ordena_por_titulo_artista_anio_y_fecha_anadida() {
    let pool = seed().await;

    // Default: fecha añadida DESC (álbum 1, 2, 4, 3).
    assert_eq!(album_ids(&pool, q()).await, vec![1, 2, 4, 3]);

    let mut query = q();
    query.sort = LibraryAlbumSortKey::Title;
    query.direction = SortDirection::Asc;
    assert_eq!(album_ids(&pool, query).await, vec![2, 4, 1, 3]);

    let mut query = q();
    query.sort = LibraryAlbumSortKey::Artist;
    query.direction = SortDirection::Asc;
    // Alpha Band (1), Beta Trio (2) y Gamma Ray (4 y 3; desempate por
    // título: Full Coverage antes que Unreleased).
    assert_eq!(album_ids(&pool, query).await, vec![1, 2, 4, 3]);

    let mut query = q();
    query.sort = LibraryAlbumSortKey::Year;
    query.direction = SortDirection::Desc;
    // El álbum 3 no tiene release_date: usa el año de sus pistas (2012).
    assert_eq!(album_ids(&pool, query).await, vec![3, 4, 2, 1]);

    let mut query = q();
    query.sort = LibraryAlbumSortKey::DateAdded;
    query.direction = SortDirection::Asc;
    assert_eq!(album_ids(&pool, query).await, vec![3, 4, 2, 1]);
}

// ==========================================================================
// Paginación en servidor
// ==========================================================================

#[tokio::test]
async fn pagina_en_servidor_con_total_de_albumes_distintos() {
    let pool = seed().await;

    let mut query = q();
    query.limit = Some(2);
    let first = query_library_albums_inner(&pool, query).await.unwrap();
    assert_eq!(first.total, 4);
    assert_eq!(first.limit, 2);
    assert_eq!(first.offset, 0);
    assert!(first.has_more);
    assert_eq!(
        first.albums.iter().map(|a| a.album_id).collect::<Vec<_>>(),
        vec![1, 2]
    );

    let mut query = q();
    query.limit = Some(2);
    query.offset = 2;
    let second = query_library_albums_inner(&pool, query).await.unwrap();
    assert_eq!(second.total, 4);
    assert!(!second.has_more);
    assert_eq!(
        second.albums.iter().map(|a| a.album_id).collect::<Vec<_>>(),
        vec![4, 3]
    );

    // El límite se recorta igual que en la consulta de pistas.
    let mut query = q();
    query.limit = Some(10_000);
    let clamped = query_library_albums_inner(&pool, query).await.unwrap();
    assert_eq!(clamped.limit, 200);
}

// ==========================================================================
// Detalle con todas las pistas (sin truncar)
// ==========================================================================

#[tokio::test]
async fn detalle_de_album_devuelve_todas_sus_pistas() {
    let pool = seed().await;

    // Se añaden 120 pistas vivas más al álbum 3: el detalle no pagina ni
    // trunca la lista, y la lista de álbumes no duplica filas.
    for id in 100..220 {
        sqlx::query(
            "INSERT INTO tracks (id, title, album_id, duration_ms, release_year, is_favorite) \
             VALUES (?, ?, 3, 100000, 2012, 0)",
        )
        .bind(id)
        .bind(format!("Track {id}"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (1, ?, 1)",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let detail = fetch_album(&pool, 3).await.unwrap();
    assert_eq!(detail.id, 3);
    assert_eq!(detail.tracks.len(), 121);

    let result = query_library_albums_inner(&pool, q()).await.unwrap();
    let third = result
        .albums
        .iter()
        .find(|album| album.album_id == 3)
        .unwrap();
    assert_eq!(third.track_count, 121);
}

// ==========================================================================
// Plan de consulta (EXPLAIN QUERY PLAN)
// ==========================================================================

#[tokio::test]
async fn plan_de_consulta_usa_indices() {
    let pool = seed().await;
    let plan = album_query_plan(&pool, &q()).await.unwrap().join("\n");
    assert!(
        plan.contains("idx_library_track")
            || plan.contains("idx_playlist_tracks_track")
            || plan.contains("idx_library_memberships_track_added")
            || plan.contains("uq_library_memberships_identity"),
        "plan={plan}"
    );
    assert!(
        plan.contains("idx_tracks_album") || plan.contains("idx_tracks_search_filters"),
        "plan={plan}"
    );
}
