//! Tests de integración de la pestaña de artistas — L08 (issue #105, spec #96).
//!
//! Fixture determinista contra SQLite `:memory:` recién migrada, con la misma
//! forma que el fixture base de la serie (`tests/library_query_test.rs`). La
//! costura probada es la función con el pool
//! (`query_library_artists_inner` / `query_library_artist_tracks_inner`), no
//! el comando de Tauri.
//!
//! Criterios cubiertos (issue #105):
//! - una fila por artista con pistas, álbumes, colaboraciones, cuentas y
//!   servicios; un artista sin pistas vivas no aparece;
//! - fichas de artista con MÁS de cinco pistas: el detalle las pagina todas,
//!   no solo las cinco de `top_tracks`;
//! - `top_tracks` es la primera página de la MISMA consulta del detalle;
//! - colaboraciones cross-service con sus fuentes (spotify + qobuz);
//! - filtros por texto, cuenta y servicio; orden por nombre, pistas y fecha
//!   añadida (con `library_added_at` NULL al final en ambas direcciones);
//! - paginación en servidor, total exacto y `has_more`;
//! - el contador `artists` de la cabecera cuenta artistas distintos con
//!   pistas vivas en el filtro;
//! - un solo contador de pistas en el detalle (`tracks_total`); el viejo
//!   `track_count` ya no viaja en el payload (#336);
//! - plan de consulta por índices.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library::fetch_artist;
use syncify_tauri_lib::commands::library_artists::{
    artist_query_plan, query_library_artist_tracks_inner, query_library_artists_inner,
    ArtistTracksQuery, LibraryArtistQuery, LibraryArtistSortKey, TOP_TRACKS_LIMIT,
};
use syncify_tauri_lib::commands::library_query::{query_library, LibraryQuery, SortDirection};

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

/// Fixture (servicios 1=spotify, 2=qobuz ya sembrados por las migraciones;
/// cuentas 1=A1 spotify, 2=A2 qobuz).
///
/// Artistas:
///   1 "Alpha Band": 8 pistas vivas (T1-T7 y T11) en 3 álbumes; una
///     colaboración (T11, con Guest Vocalist).
///   2 "Guest Vocalist": 3 pistas vivas (T9, T10, T11), 1 álbum y 2
///     colaboraciones (T9 y T11).
///   3 "Beta Trio": 2 pistas vivas (T8, T9), 2 álbumes; una colaboración
///     (T9, con Guest Vocalist).
///   4 "Ghost Artist": solo T12, sin pertenencia viva → NO aparece.
///   5 "Empty Artist": sin pistas → NO aparece.
///
/// Fuentes: T9 (colaboración Beta+Guest) solo qobuz; T11 (colaboración
/// Alpha+Guest) spotify+qobuz — la colaboración cross-service con sus dos
/// fuentes.
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
        "INSERT INTO artists (id, name, image_url) VALUES \
         (1, 'Alpha Band', 'https://x/alpha.jpg'), \
         (2, 'Guest Vocalist', NULL), \
         (3, 'Beta Trio', NULL), \
         (4, 'Ghost Artist', NULL), \
         (5, 'Empty Artist', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO albums (id, title, release_date) VALUES \
         (1, 'Nightfall', '1999-05-01'), \
         (2, 'Dawn', '2005-01-01'), \
         (3, 'Beta Album', '2010-01-01'), \
         (4, 'Guest Album', '2015-01-01'), \
         (5, 'Ghost Album', '2000-01-01')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES \
         (1, 1, 1), (2, 1, 1), (3, 3, 1), (4, 2, 1), (5, 4, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, created_at) VALUES \
         (1, 'Sunrise', 1, 200000, '2025-12-01T09:00:00Z'), \
         (2, 'Moonlight', 1, 210000, '2025-12-01T09:00:00Z'), \
         (3, 'Starlight', 1, 220000, '2025-12-01T09:00:00Z'), \
         (4, 'Island', 1, 180000, '2025-12-01T09:00:00Z'), \
         (5, 'River', 1, 190000, '2025-12-01T09:00:00Z'), \
         (6, 'One', 2, 170000, '2025-12-01T09:00:00Z'), \
         (7, 'Two', 2, 175000, '2025-12-01T09:00:00Z'), \
         (8, 'Beta Song', 3, 160000, '2025-12-01T09:00:00Z'), \
         (9, 'Duet', 4, 230000, '2025-12-01T09:00:00Z'), \
         (10, 'Solo', 4, 150000, '2025-12-01T09:00:00Z'), \
         (11, 'Joint', 4, 240000, '2025-12-01T09:00:00Z'), \
         (12, 'Ghost Track', 5, 90000, '2025-12-01T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES \
         (1, 1, 'primary'), (2, 1, 'primary'), (3, 1, 'primary'), \
         (4, 1, 'primary'), (5, 1, 'primary'), (6, 1, 'primary'), \
         (7, 1, 'primary'), \
         (8, 3, 'primary'), \
         (9, 3, 'primary'), (9, 2, 'featured'), \
         (10, 2, 'primary'), \
         (11, 1, 'primary'), (11, 2, 'featured'), \
         (12, 4, 'primary')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) VALUES \
         (1, 1, 'sp-1', 'FLAC', 1, 'available'), \
         (2, 1, 'sp-2', 'FLAC', 1, 'available'), \
         (3, 1, 'sp-3', 'FLAC', 1, 'available'), \
         (3, 2, 'qb-3', 'FLAC', 1, 'available'), \
         (4, 2, 'qb-4', 'FLAC', 1, 'available'), \
         (5, 1, 'sp-5', 'MP3', 1, 'available'), \
         (6, 1, 'sp-6', 'FLAC', 1, 'available'), \
         (7, 2, 'qb-7', 'FLAC', 1, 'available'), \
         (8, 2, 'qb-8', 'FLAC', 1, 'available'), \
         (9, 2, 'qb-9', 'FLAC', 1, 'available'), \
         (10, 2, 'qb-10', 'FLAC', 1, 'available'), \
         (11, 1, 'sp-11', 'FLAC', 1, 'available'), \
         (11, 2, 'qb-11', 'FLAC', 1, 'available'), \
         (12, 1, 'sp-12', 'FLAC', 1, 'available')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Pertenencia viva: favoritos por cuenta, playlist con dueño y
    // library_memberships (L00). T12 (Ghost) no tiene ninguna.
    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) VALUES \
         (1, 1, '2026-03-01T10:00:00Z', 1), \
         (2, 8, '2026-02-20T10:00:00Z', 1), \
         (1, 10, '2026-01-05T10:00:00Z', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES \
         (1, 1, 'pl-a1', 'A1 list')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, added_at) VALUES \
         (1, 9, '2026-04-01T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_memberships (account_id, track_id, collection_type, collection_id, provider_item, added_at) VALUES \
         (1, 2, 'album', 1, 'sp-alb-nightfall', '2026-01-10T10:00:00Z'), \
         (2, 3, 'album', 1, 'qb-alb-nightfall', '2026-01-11T10:00:00Z'), \
         (1, 4, 'artist', 1, 'sp-art-alpha', '2026-01-12T10:00:00Z'), \
         (2, 5, 'album', 1, 'qb-alb-nightfall', '2026-01-13T10:00:00Z'), \
         (1, 6, 'album', 2, 'sp-alb-dawn', '2026-01-14T10:00:00Z'), \
         (2, 7, 'album', 2, 'qb-alb-dawn', '2026-01-15T10:00:00Z'), \
         (1, 11, 'artist', 1, 'sp-art-alpha', '2026-05-01T10:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    for track_id in 1..=12 {
        syncify_tauri_lib::services::library_membership::refresh_track_library_added_at(
            &pool, track_id,
        )
        .await
        .unwrap();
    }

    pool
}

fn q() -> LibraryArtistQuery {
    LibraryArtistQuery::default()
}

async fn artist_names(pool: &SqlitePool, query: LibraryArtistQuery) -> Vec<String> {
    query_library_artists_inner(pool, query)
        .await
        .unwrap()
        .artists
        .into_iter()
        .map(|artist| artist.name)
        .collect()
}

// ==========================================================================
// Una fila por artista con sus contadores y badges
// ==========================================================================

#[tokio::test]
async fn una_fila_por_artista_y_solo_artistas_con_pistas_vivas() {
    let pool = seed().await;
    let result = query_library_artists_inner(&pool, q()).await.unwrap();

    // Alpha, Guest y Beta tienen pistas vivas. Ghost (T12 sin pertenencia) y
    // Empty (sin pistas) no aparecen.
    assert_eq!(result.total, 3);
    let names: Vec<&str> = result.artists.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha Band", "Beta Trio", "Guest Vocalist"]);

    let alpha = result.artists.iter().find(|a| a.artist_id == 1).unwrap();
    assert_eq!(alpha.image_url.as_deref(), Some("https://x/alpha.jpg"));
    assert_eq!(alpha.track_count, 8);
    assert_eq!(alpha.album_count, 3);
    assert_eq!(alpha.collaboration_count, 1);
    assert_eq!(alpha.date_added.as_deref(), Some("2026-05-01T10:00:00Z"));

    let guest = result.artists.iter().find(|a| a.artist_id == 2).unwrap();
    assert_eq!(guest.track_count, 3);
    assert_eq!(guest.album_count, 1);
    assert_eq!(guest.collaboration_count, 2);

    let beta = result.artists.iter().find(|a| a.artist_id == 3).unwrap();
    assert_eq!(beta.track_count, 2);
    assert_eq!(beta.album_count, 2);
    assert_eq!(beta.collaboration_count, 1);

    // Badges de Alpha: A2 (qobuz) antes que A1 (spotify), una vez cada una;
    // servicios spotify y qobuz una vez cada uno pese a varias fuentes.
    let accounts: Vec<(&str, i64)> = alpha
        .accounts
        .iter()
        .map(|a| (a.display_name.as_deref().unwrap_or(""), a.account_id))
        .collect();
    assert_eq!(accounts, vec![("A2", 2), ("A1", 1)]);
    let services: Vec<&str> = alpha
        .services
        .iter()
        .map(|s| s.service_name.as_str())
        .collect();
    assert_eq!(services, vec!["qobuz", "spotify"]);
}

#[tokio::test]
async fn contador_de_artistas_de_la_cabecera_cuenta_distintos_con_pistas_vivas() {
    let pool = seed().await;

    let all = query_library(&pool, LibraryQuery::default()).await.unwrap();
    let counts = all.counts.expect("la primera página trae contadores");
    assert_eq!(counts.artists, 3);
    assert_eq!(counts.total, 11, "pistas vivas: T1-T11");

    // Con el filtro de cuenta A2, los artistas del filtro son Alpha y Beta.
    let filtered = query_library(
        &pool,
        LibraryQuery {
            account_ids: vec![2],
            ..LibraryQuery::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(filtered.counts.unwrap().artists, 2);
}

// ==========================================================================
// Filtros y orden
// ==========================================================================

#[tokio::test]
async fn filtro_de_texto_combina_con_la_agregacion() {
    let pool = seed().await;

    // "Nightfall" es un álbum de Alpha: solo sus pistas vivas casan.
    let names = artist_names(
        &pool,
        LibraryArtistQuery {
            text: Some("Nightfall".to_owned()),
            ..q()
        },
    )
    .await;
    assert_eq!(names, vec!["Alpha Band"]);

    // "Duet" es la colaboración Beta+Guest: ambos artistas aparecen, Alpha no.
    let names = artist_names(
        &pool,
        LibraryArtistQuery {
            text: Some("Duet".to_owned()),
            ..q()
        },
    )
    .await;
    assert_eq!(names, vec!["Beta Trio", "Guest Vocalist"]);
}

#[tokio::test]
async fn filtros_de_cuenta_y_servicio() {
    let pool = seed().await;

    // A2 solo tiene pertenencias en T3, T5, T7 y T8 → Alpha y Beta.
    let names = artist_names(
        &pool,
        LibraryArtistQuery {
            account_ids: vec![2],
            ..q()
        },
    )
    .await;
    assert_eq!(names, vec!["Alpha Band", "Beta Trio"]);

    // Solo spotify: Beta no tiene ninguna pista con fuente spotify.
    let names = artist_names(
        &pool,
        LibraryArtistQuery {
            service_ids: vec![1],
            ..q()
        },
    )
    .await;
    assert_eq!(names, vec!["Alpha Band", "Guest Vocalist"]);

    // Cuenta y servicio sobre la MISMA pertenencia: A2 ∩ qobuz = T3, T7, T8.
    let names = artist_names(
        &pool,
        LibraryArtistQuery {
            account_ids: vec![2],
            service_ids: vec![2],
            ..q()
        },
    )
    .await;
    assert_eq!(names, vec!["Alpha Band", "Beta Trio"]);
}

#[tokio::test]
async fn orden_por_nombre_pistas_y_fecha_anadida() {
    let pool = seed().await;

    let by_name_desc = artist_names(
        &pool,
        LibraryArtistQuery {
            sort: LibraryArtistSortKey::Name,
            direction: SortDirection::Desc,
            ..q()
        },
    )
    .await;
    assert_eq!(
        by_name_desc,
        vec!["Guest Vocalist", "Beta Trio", "Alpha Band"]
    );

    let by_tracks_desc = artist_names(
        &pool,
        LibraryArtistQuery {
            sort: LibraryArtistSortKey::Tracks,
            direction: SortDirection::Desc,
            ..q()
        },
    )
    .await;
    assert_eq!(
        by_tracks_desc,
        vec!["Alpha Band", "Guest Vocalist", "Beta Trio"]
    );

    // Fecha añadida: Alpha y Guest empatan en 2026-05-01 (T11); el
    // desempate por nombre deja Alpha antes que Guest. Beta, 2026-04-01.
    let by_date_desc = artist_names(
        &pool,
        LibraryArtistQuery {
            sort: LibraryArtistSortKey::DateAdded,
            direction: SortDirection::Desc,
            ..q()
        },
    )
    .await;
    assert_eq!(
        by_date_desc,
        vec!["Alpha Band", "Guest Vocalist", "Beta Trio"]
    );
}

// ==========================================================================
// Paginación de la lista
// ==========================================================================

#[tokio::test]
async fn lista_pagina_en_servidor_con_total_exacto() {
    let pool = seed().await;

    let first = query_library_artists_inner(
        &pool,
        LibraryArtistQuery {
            offset: 0,
            limit: Some(1),
            ..q()
        },
    )
    .await
    .unwrap();
    assert_eq!(first.total, 3);
    assert_eq!(first.artists.len(), 1);
    assert_eq!(first.artists[0].name, "Alpha Band");
    assert!(first.has_more);

    let second = query_library_artists_inner(
        &pool,
        LibraryArtistQuery {
            offset: 1,
            limit: Some(1),
            ..q()
        },
    )
    .await
    .unwrap();
    assert_eq!(second.artists[0].name, "Beta Trio");
    assert!(second.has_more);

    let third = query_library_artists_inner(
        &pool,
        LibraryArtistQuery {
            offset: 2,
            limit: Some(1),
            ..q()
        },
    )
    .await
    .unwrap();
    assert_eq!(third.artists[0].name, "Guest Vocalist");
    assert!(!third.has_more);
}

#[tokio::test]
async fn orden_por_fecha_anadida_deja_los_nulos_al_final() {
    let pool = seed().await;

    // Artista con pistas vivas pero SIN fecha de añadido materializada: la
    // pertenencia existe (`library_memberships`) y `tracks.library_added_at`
    // sigue NULL porque nadie lo refrescó. Es el caso de una importación
    // antigua o de una pista sin `added_at` en el proveedor.
    sqlx::query("INSERT INTO artists (id, name) VALUES (6, 'Null Date')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tracks (id, title, created_at) VALUES (13, 'Undated', '2025-12-01T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (13, 6, 'primary')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO library_memberships (account_id, track_id, collection_type, collection_id, provider_item, added_at) \
         VALUES (1, 13, 'album', 9, 'sp-alb-undated', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let by_date_desc = artist_names(
        &pool,
        LibraryArtistQuery {
            sort: LibraryArtistSortKey::DateAdded,
            direction: SortDirection::Desc,
            ..q()
        },
    )
    .await;
    assert_eq!(
        by_date_desc,
        vec!["Alpha Band", "Guest Vocalist", "Beta Trio", "Null Date"],
        "los nulos van al final en descendente"
    );

    let by_date_asc = artist_names(
        &pool,
        LibraryArtistQuery {
            sort: LibraryArtistSortKey::DateAdded,
            direction: SortDirection::Asc,
            ..q()
        },
    )
    .await;
    assert_eq!(
        by_date_asc,
        vec!["Beta Trio", "Alpha Band", "Guest Vocalist", "Null Date"],
        "los nulos van al final también en ascendente"
    );

    // Y el campo del payload lo dice: sin fecha, `date_added` va a None.
    let result = query_library_artists_inner(&pool, q()).await.unwrap();
    let undated = result.artists.iter().find(|a| a.artist_id == 6).unwrap();
    assert_eq!(undated.track_count, 1);
    assert_eq!(undated.date_added, None);
}

// ==========================================================================
// Detalle: un solo contador de pistas (tracks_total)
// ==========================================================================

// #336: el detalle publica SOLO `tracks_total` (pistas vivas distintas). El
// viejo `track_count` contaba filas de `track_artists` —roles duplicados y
// pistas sin pertenencia viva incluidas— y divergía del total visible. Ya no
// existe en el payload: la pista repetida y la no viva no mueven la cifra.
#[tokio::test]
async fn detalle_publica_un_solo_contador_de_pistas() {
    let pool = seed().await;

    // T1 ya cuenta para Alpha: un segundo rol para el mismo artista duplica
    // la fila. T13 es de Alpha pero no tiene pertenencia viva (no es visible).
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (1, 1, 'featured')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tracks (id, title, created_at) VALUES (13, 'Hidden', '2025-12-01T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (13, 1, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    let detail = fetch_artist(&pool, 1).await.unwrap();
    assert_eq!(detail.tracks_total, 8, "pistas vivas distintas");

    let payload = serde_json::to_value(&detail).unwrap();
    assert!(
        payload.get("track_count").is_none(),
        "el detalle ya no publica un segundo contador que diverge: {payload}"
    );
    assert_eq!(payload["tracks_total"], 8);
    // La cabecera sigue prometiendo lo que la acción encola: el total vive.
    assert!(detail.top_tracks.len() <= TOP_TRACKS_LIMIT as usize);
}

// ==========================================================================
// Detalle: todas las pistas, no solo cinco
// ==========================================================================

#[tokio::test]
async fn detalle_pagina_todas_las_pistas_del_artista() {
    let pool = seed().await;

    let first = query_library_artist_tracks_inner(
        &pool,
        ArtistTracksQuery {
            artist_id: 1,
            offset: 0,
            limit: Some(5),
        },
    )
    .await
    .unwrap();
    assert_eq!(first.total, 8, "Alpha tiene 8 pistas vivas");
    assert_eq!(first.tracks.len(), 5);
    assert!(first.has_more);
    // Orden estable: título NOCASE (Island, Joint, Moonlight, One, River) y
    // luego id.
    let ids: Vec<i64> = first.tracks.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![4, 11, 2, 6, 5]);

    let second = query_library_artist_tracks_inner(
        &pool,
        ArtistTracksQuery {
            artist_id: 1,
            offset: 5,
            limit: Some(5),
        },
    )
    .await
    .unwrap();
    let ids: Vec<i64> = second.tracks.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![3, 1, 7]);
    assert!(!second.has_more);

    // Sin solapes y con TODAS las pistas: la unión de las dos páginas.
    let mut all: Vec<i64> = first
        .tracks
        .iter()
        .chain(second.tracks.iter())
        .map(|t| t.id)
        .collect();
    all.sort_unstable();
    assert_eq!(all, vec![1, 2, 3, 4, 5, 6, 7, 11]);
}

#[tokio::test]
async fn top_tracks_es_la_primera_pagina_de_la_misma_consulta() {
    let pool = seed().await;

    let detail = fetch_artist(&pool, 1).await.unwrap();
    assert_eq!(detail.tracks_total, 8);
    assert_eq!(detail.top_tracks.len(), TOP_TRACKS_LIMIT as usize);

    let page = query_library_artist_tracks_inner(
        &pool,
        ArtistTracksQuery::first_page(1, TOP_TRACKS_LIMIT),
    )
    .await
    .unwrap();
    let top_ids: Vec<i64> = detail.top_tracks.iter().map(|t| t.id).collect();
    let page_ids: Vec<i64> = page.tracks.iter().map(|t| t.id).collect();
    assert_eq!(top_ids, page_ids, "top_tracks == primera página");
    assert_eq!(page.tracks[0].id, 4);
}

#[tokio::test]
async fn colaboraciones_cross_service_muestran_sus_fuentes() {
    let pool = seed().await;

    let page = query_library_artist_tracks_inner(
        &pool,
        ArtistTracksQuery {
            artist_id: 1,
            offset: 0,
            limit: Some(5),
        },
    )
    .await
    .unwrap();

    // T11 "Joint" es la colaboración con Guest Vocalist y tiene fuente en
    // spotify y qobuz: las dos se muestran.
    let joint = page.tracks.iter().find(|t| t.id == 11).unwrap();
    assert_eq!(joint.collaborators, vec!["Guest Vocalist".to_owned()]);
    let services: Vec<&str> = joint
        .sources
        .iter()
        .map(|s| s.service_name.as_str())
        .collect();
    assert_eq!(services, vec!["qobuz", "spotify"]);

    // Una pista en solitario no inventa colaboradores; sus fuentes siguen.
    let island = page.tracks.iter().find(|t| t.id == 4).unwrap();
    assert!(island.collaborators.is_empty());
    let services: Vec<&str> = island
        .sources
        .iter()
        .map(|s| s.service_name.as_str())
        .collect();
    assert_eq!(services, vec!["qobuz"]);

    // La colaboración de Beta con Guest también aparece desde su lado.
    let beta = query_library_artist_tracks_inner(
        &pool,
        ArtistTracksQuery {
            artist_id: 3,
            offset: 0,
            limit: Some(10),
        },
    )
    .await
    .unwrap();
    let duet = beta.tracks.iter().find(|t| t.id == 9).unwrap();
    assert_eq!(duet.collaborators, vec!["Guest Vocalist".to_owned()]);
}

#[tokio::test]
async fn un_artista_sin_pistas_vivas_no_tiene_detalle() {
    let pool = seed().await;

    let ghost = query_library_artist_tracks_inner(
        &pool,
        ArtistTracksQuery {
            artist_id: 4,
            offset: 0,
            limit: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(ghost.total, 0);
    assert!(ghost.tracks.is_empty());
}

#[tokio::test]
async fn plan_de_consulta_usa_indices() {
    let pool = seed().await;
    let plan = artist_query_plan(&pool, &q()).await.unwrap().join("\n");
    assert!(
        plan.contains("idx_library_track")
            || plan.contains("idx_playlist_tracks_track")
            || plan.contains("idx_library_memberships_track_added")
            || plan.contains("uq_library_memberships_identity"),
        "plan={plan}"
    );
    assert!(
        plan.contains("idx_track_artists_artist_role")
            || plan.contains("sqlite_autoindex_track_artists"),
        "plan={plan}"
    );
}
