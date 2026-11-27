//! #402 · gateway de búsqueda de catálogo: cuenta preferida por servicio,
//! fallback a la activa y aislamiento de fallos entre proveedores.
//!
//! Los proveedores son falsos y la base es `:memory:`: ninguna prueba abre
//! red. El adaptador Qobuz tiene sus propias pruebas de fixtures en
//! `services::catalog_search`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::services::catalog_search::{
    CatalogAlbum, CatalogArtist, CatalogPage, CatalogPageRequest, CatalogSearchError,
    CatalogSearchErrorKind, CatalogSearchGateway, CatalogSearchProvider, CatalogSearchRequest,
    CatalogTrack,
};

type Calls = Arc<Mutex<Vec<Option<i64>>>>;

struct FakeProvider {
    service: &'static str,
    calls: Calls,
    fail_tracks: bool,
}

impl FakeProvider {
    fn new(service: &'static str, fail_tracks: bool) -> (Self, Calls) {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                service,
                calls: calls.clone(),
                fail_tracks,
            },
            calls,
        )
    }

    fn record(&self, account_id: Option<i64>) {
        self.calls
            .lock()
            .expect("fake provider calls")
            .push(account_id);
    }
}

fn empty_page<T>(page: CatalogPageRequest, items: Vec<T>) -> CatalogPage<T> {
    CatalogPage {
        total: Some(items.len() as u64),
        items,
        offset: page.offset,
        limit: page.limit,
    }
}

impl CatalogSearchProvider for FakeProvider {
    fn service_name(&self) -> &'static str {
        self.service
    }

    fn search_tracks<'a>(
        &'a self,
        account_id: Option<i64>,
        _query: &'a str,
        page: CatalogPageRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<CatalogPage<CatalogTrack>, CatalogSearchError>> + Send + 'a>,
    > {
        self.record(account_id);
        let fail = self.fail_tracks;
        Box::pin(async move {
            if fail {
                return Err(CatalogSearchError::auth_expired(
                    "fixture: track search requires authentication",
                ));
            }
            Ok(empty_page(
                page,
                vec![CatalogTrack {
                    id: "track-1".to_string(),
                    title: "One More Time".to_string(),
                    artist: "Daft Punk".to_string(),
                    album: Some("Discovery".to_string()),
                    isrc: Some("GBDUW0000059".to_string()),
                    duration_ms: 320_000,
                    bit_depth: Some(24),
                    sample_rate: Some(96.0),
                }],
            ))
        })
    }

    fn search_albums<'a>(
        &'a self,
        account_id: Option<i64>,
        _query: &'a str,
        page: CatalogPageRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<CatalogPage<CatalogAlbum>, CatalogSearchError>> + Send + 'a>,
    > {
        self.record(account_id);
        Box::pin(async move { Ok(empty_page(page, Vec::new())) })
    }

    fn search_artists<'a>(
        &'a self,
        account_id: Option<i64>,
        _query: &'a str,
        page: CatalogPageRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CatalogPage<CatalogArtist>, CatalogSearchError>> + Send + 'a,
        >,
    > {
        self.record(account_id);
        Box::pin(async move { Ok(empty_page(page, Vec::new())) })
    }
}

async fn setup_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    pool
}

async fn seed_qobuz_account(pool: &SqlitePool, is_active: bool) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("qobuz service row");

    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'fixture', ?) RETURNING id",
    )
    .bind(service_id)
    .bind(i64::from(is_active))
    .fetch_one(pool)
    .await
    .expect("fixture account")
}

async fn prefer_account(pool: &SqlitePool, account_id: i64) {
    sqlx::query(
        "INSERT INTO service_preferred_accounts (service_name, account_id) VALUES ('qobuz', ?)",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .expect("preferred account");
}

fn request() -> CatalogSearchRequest {
    CatalogSearchRequest::new("daft punk", CatalogPageRequest::default()).expect("valid query")
}

#[tokio::test]
async fn preferred_account_wins_even_when_it_is_not_the_active_one() {
    let pool = setup_db().await;
    let preferred = seed_qobuz_account(&pool, false).await;
    let _active = seed_qobuz_account(&pool, true).await;
    prefer_account(&pool, preferred).await;

    let (provider, calls) = FakeProvider::new("qobuz", false);
    let mut gateway = CatalogSearchGateway::new(pool);
    gateway.register_provider(Arc::new(provider));

    let response = gateway.search(&request()).await;

    assert_eq!(response.providers.len(), 1);
    let qobuz = &response.providers[0];
    assert_eq!(qobuz.account_id, Some(preferred));
    assert_eq!(qobuz.tracks.page.as_ref().map(|p| p.items.len()), Some(1));
    let recorded = calls.lock().expect("recorded calls").clone();
    assert_eq!(recorded, vec![Some(preferred); 3]);
}

#[tokio::test]
async fn falls_back_to_the_first_active_account_when_there_is_no_preference() {
    let pool = setup_db().await;
    let _inactive = seed_qobuz_account(&pool, false).await;
    let active = seed_qobuz_account(&pool, true).await;

    let (provider, calls) = FakeProvider::new("qobuz", false);
    let mut gateway = CatalogSearchGateway::new(pool);
    gateway.register_provider(Arc::new(provider));

    let response = gateway.search(&request()).await;

    assert_eq!(response.providers[0].account_id, Some(active));
    let recorded = calls.lock().expect("recorded calls").clone();
    assert_eq!(recorded, vec![Some(active); 3]);
}

#[tokio::test]
async fn one_failing_provider_does_not_hide_or_blanket_the_others() {
    let pool = setup_db().await;
    let active = seed_qobuz_account(&pool, true).await;

    let (alpha, _alpha_calls) = FakeProvider::new("alpha", true);
    let (qobuz, _qobuz_calls) = FakeProvider::new("qobuz", false);
    let mut gateway = CatalogSearchGateway::new(pool);
    gateway.register_provider(Arc::new(alpha));
    gateway.register_provider(Arc::new(qobuz));

    let response = gateway.search(&request()).await;

    assert_eq!(response.providers.len(), 2);
    let alpha = response
        .providers
        .iter()
        .find(|provider| provider.provider == "alpha")
        .expect("alpha entry");
    let qobuz = response
        .providers
        .iter()
        .find(|provider| provider.provider == "qobuz")
        .expect("qobuz entry");

    // Fallo parcial visible: la sección de pistas de `alpha` falla y las
    // otras dos siguen respondiendo.
    assert_eq!(
        alpha.tracks.error.as_ref().map(|error| error.kind),
        Some(CatalogSearchErrorKind::AuthExpired)
    );
    assert!(alpha.albums.is_ok());
    assert!(alpha.artists.is_ok());

    // El proveedor sano conserva su cuenta y sus resultados.
    assert_eq!(qobuz.account_id, Some(active));
    let tracks = qobuz.tracks.page.as_ref().expect("qobuz tracks");
    assert_eq!(tracks.items.len(), 1);
    assert_eq!(tracks.items[0].title, "One More Time");
    assert!(!qobuz.has_failures());
}
