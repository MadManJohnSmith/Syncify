//! P10b (#409): contrato de los comandos de búsqueda de catálogo.
//!
//! El gateway de #402 se prueba con un proveedor falso que sustituye a Qobuz
//! por el mismo `service_name`: ninguna prueba abre red. La BD es `:memory:`
//! recién migrada porque el gateway resuelve la cuenta preferida/activa en
//! `services` + `accounts`, igual que en producción.

use std::sync::Arc;

use sqlx::SqlitePool;
use syncify_tauri_lib::commands::catalog_search_commands::{
    perform_catalog_album_tracks, perform_catalog_artist_albums, perform_catalog_search,
};
use syncify_tauri_lib::services::catalog_search::{
    CatalogAlbum, CatalogArtist, CatalogPage, CatalogPageRequest, CatalogSearchError,
    CatalogSearchErrorKind, CatalogSearchFuture, CatalogSearchGateway, CatalogSearchProvider,
    CatalogTrack,
};

async fn test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("fresh migrations");
    pool
}

struct FakeQobuz {
    tracks: Result<CatalogPage<CatalogTrack>, CatalogSearchError>,
    albums: Result<CatalogPage<CatalogAlbum>, CatalogSearchError>,
    artists: Result<CatalogPage<CatalogArtist>, CatalogSearchError>,
}

impl CatalogSearchProvider for FakeQobuz {
    fn service_name(&self) -> &'static str {
        "qobuz"
    }

    fn search_tracks<'a>(
        &'a self,
        _account_id: Option<i64>,
        _query: &'a str,
        _page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogTrack>> {
        Box::pin(async move { self.tracks.clone() })
    }

    fn search_albums<'a>(
        &'a self,
        _account_id: Option<i64>,
        _query: &'a str,
        _page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogAlbum>> {
        Box::pin(async move { self.albums.clone() })
    }

    fn search_artists<'a>(
        &'a self,
        _account_id: Option<i64>,
        _query: &'a str,
        _page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogArtist>> {
        Box::pin(async move { self.artists.clone() })
    }
}

async fn gateway_with_fake(
    db: &SqlitePool,
    tracks: Result<CatalogPage<CatalogTrack>, CatalogSearchError>,
    albums: Result<CatalogPage<CatalogAlbum>, CatalogSearchError>,
    artists: Result<CatalogPage<CatalogArtist>, CatalogSearchError>,
) -> CatalogSearchGateway {
    let mut gateway = CatalogSearchGateway::new(db.clone());
    gateway.register_provider(Arc::new(FakeQobuz {
        tracks,
        albums,
        artists,
    }));
    gateway
}

fn page_of<T>(items: Vec<T>) -> CatalogPage<T> {
    let len = items.len() as u64;
    CatalogPage {
        items,
        total: Some(len),
        offset: 0,
        limit: 20,
    }
}

fn track(id: &str, album: Option<&str>) -> CatalogTrack {
    CatalogTrack {
        id: id.to_string(),
        title: format!("Track {id}"),
        artist: "Daft Punk".to_string(),
        album: album.map(str::to_string),
        isrc: None,
        duration_ms: 200_000,
        bit_depth: Some(24),
        sample_rate: Some(96.0),
    }
}

fn album(id: &str, artist: Option<&str>) -> CatalogAlbum {
    CatalogAlbum {
        id: id.to_string(),
        title: format!("Album {id}"),
        artist: artist.map(str::to_string),
        track_count: Some(14),
        released_at: None,
        cover_url: None,
    }
}

fn artist(id: &str, name: &str) -> CatalogArtist {
    CatalogArtist {
        id: id.to_string(),
        name: name.to_string(),
        image_url: None,
        album_count: Some(8),
    }
}

#[tokio::test]
async fn partial_failures_stay_visible_and_do_not_hide_the_healthy_sections() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Ok(page_of(vec![track("1", Some("Discovery"))])),
        Err(CatalogSearchError::auth_expired("expired")),
        Ok(page_of(vec![artist("501", "Daft Punk")])),
    )
    .await;

    let response =
        perform_catalog_search(&db, &gateway, "daft punk", CatalogPageRequest::default())
            .await
            .expect("search response");

    assert_eq!(response.query, "daft punk");
    assert_eq!(response.providers.len(), 1);
    let qobuz = &response.providers[0];
    assert_eq!(qobuz.provider, "qobuz");
    assert!(
        qobuz.service_id.is_some_and(|id| id > 0),
        "el service_id alimenta la entrada efímera de la cola"
    );
    assert_eq!(
        qobuz.tracks.page.as_ref().map(|page| page.items.len()),
        Some(1),
        "el fallo de álbumes no oculta las pistas"
    );
    assert_eq!(
        qobuz.albums.error.as_ref().map(|error| error.kind),
        Some(CatalogSearchErrorKind::AuthExpired)
    );
    assert!(qobuz.albums.page.is_none());
    assert_eq!(
        qobuz.artists.page.as_ref().map(|page| page.items.len()),
        Some(1),
        "el fallo de álbumes no oculta los artistas"
    );
}

#[tokio::test]
async fn empty_query_is_rejected_before_reaching_a_provider() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Ok(page_of(Vec::new())),
        Ok(page_of(Vec::new())),
        Ok(page_of(Vec::new())),
    )
    .await;

    let error = perform_catalog_search(&db, &gateway, "   ", CatalogPageRequest::default())
        .await
        .expect_err("empty query");
    assert!(error.contains("cannot be empty"));
}

#[tokio::test]
async fn album_tracks_filter_the_page_to_the_open_album() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Ok(page_of(vec![
            track("1", Some("Discovery")),
            track("2", Some("Homework")),
            track("3", Some("Discovery")),
            track("4", None),
        ])),
        Ok(page_of(Vec::new())),
        Ok(page_of(Vec::new())),
    )
    .await;

    let section = perform_catalog_album_tracks(&gateway, "qobuz", "Discovery", Some("Daft Punk"))
        .await
        .expect("album ficha");

    let page = section.page.expect("tracks");
    assert_eq!(page.items.len(), 2);
    assert!(page
        .items
        .iter()
        .all(|item| item.album.as_deref() == Some("Discovery")));
    assert_eq!(page.total, None, "la ficha no hereda el total textual");
}

#[tokio::test]
async fn artist_albums_filter_the_page_to_the_open_artist() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Ok(page_of(Vec::new())),
        Ok(page_of(vec![
            album("1", Some("Daft Punk")),
            album("2", Some("Daft Punk feat. Panda Bear")),
            album("3", Some("Daft Punk")),
            album("4", None),
        ])),
        Ok(page_of(Vec::new())),
    )
    .await;

    let section = perform_catalog_artist_albums(&gateway, "qobuz", "Daft Punk")
        .await
        .expect("artist ficha");

    let page = section.page.expect("albums");
    assert_eq!(page.items.len(), 2);
    assert!(page
        .items
        .iter()
        .all(|item| item.artist.as_deref() == Some("Daft Punk")));
}

#[tokio::test]
async fn fiches_keep_the_section_failure_instead_of_an_empty_list() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Err(CatalogSearchError::retryable("network")),
        Err(CatalogSearchError::permanent("bad payload")),
        Ok(page_of(Vec::new())),
    )
    .await;

    let tracks = perform_catalog_album_tracks(&gateway, "qobuz", "Discovery", None)
        .await
        .expect("album section");
    assert!(tracks.page.is_none());
    assert_eq!(
        tracks.error.as_ref().map(|error| error.kind),
        Some(CatalogSearchErrorKind::Retryable)
    );

    let albums = perform_catalog_artist_albums(&gateway, "qobuz", "Daft Punk")
        .await
        .expect("artist section");
    assert!(albums.page.is_none());
    assert_eq!(
        albums.error.as_ref().map(|error| error.kind),
        Some(CatalogSearchErrorKind::Permanent)
    );
}

#[tokio::test]
async fn fiches_reject_unknown_providers_and_empty_identities() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Ok(page_of(vec![track("1", Some("Discovery"))])),
        Ok(page_of(vec![album("1", Some("Daft Punk"))])),
        Ok(page_of(Vec::new())),
    )
    .await;

    assert!(
        perform_catalog_album_tracks(&gateway, "deezer", "Discovery", None)
            .await
            .is_err(),
        "un proveedor no registrado no puede devolver una ficha vacía"
    );
    assert!(
        perform_catalog_album_tracks(&gateway, "qobuz", "  ", Some("Daft Punk"))
            .await
            .is_err()
    );
    assert!(perform_catalog_artist_albums(&gateway, "qobuz", "   ")
        .await
        .is_err());
}

#[tokio::test]
async fn search_forwards_the_page_and_clamps_the_limit() {
    let db = test_db().await;
    let gateway = gateway_with_fake(
        &db,
        Ok(page_of(Vec::new())),
        Ok(page_of(Vec::new())),
        Ok(page_of(Vec::new())),
    )
    .await;

    let response = perform_catalog_search(
        &db,
        &gateway,
        "daft punk",
        CatalogPageRequest::new(40, 1_000),
    )
    .await
    .expect("search response");

    assert_eq!(response.page.offset, 40);
    assert_eq!(response.page.limit, 100, "el techo del gateway se respeta");
}
