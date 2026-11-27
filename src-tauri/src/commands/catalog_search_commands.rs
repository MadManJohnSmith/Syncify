//! P10b (#409): comandos IPC de la búsqueda de catálogo.
//!
//! El gateway de #402 (`services::catalog_search`) es la única puerta a los
//! proveedores: este módulo no hace red, no conoce tokens y no reescribe la
//! política de reintentos. Solo valida la petición, adjunta el `service_id`
//! que la cola de reproducción necesita para una entrada efímera y compone las
//! fichas de álbum y artista a partir de las mismas búsquedas del gateway.
//!
//! Las fichas no navegan el catálogo completo (ADR-0010 sigue dejándolo fuera):
//! piden una página acotada al gateway y filtran por la identidad que el
//! usuario ya está mirando — el título del álbum o el nombre del artista. Un
//! fallo del proveedor viaja dentro de la [`CatalogSection`] para que la UI no
//! convierta «no pude consultar» en «no hay resultados».

use super::*;
use crate::services::catalog_search::{
    CatalogAlbum, CatalogArtist, CatalogPage, CatalogPageRequest, CatalogProviderResponse,
    CatalogSearchGateway, CatalogSearchRequest, CatalogSection, CatalogTrack,
    DEFAULT_CATALOG_PAGE_SIZE, MAX_CATALOG_PAGE_SIZE,
};

/// Respuesta de una búsqueda con el `service_id` de cada proveedor.
///
/// El gateway identifica al proveedor por nombre; la cola efímera
/// (`playback_queue_enqueue_provider_item`) exige el id numérico. Se resuelve
/// aquí, una vez, para que la vista no tenga que cruzar dos comandos ni
/// adivinar el id.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogProviderSearchDto {
    pub provider: String,
    /// `None` si el nombre del proveedor no tiene fila en `services`.
    pub service_id: Option<i64>,
    pub account_id: Option<i64>,
    pub tracks: CatalogSection<CatalogTrack>,
    pub albums: CatalogSection<CatalogAlbum>,
    pub artists: CatalogSection<CatalogArtist>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogSearchResponseDto {
    pub query: String,
    pub page: CatalogPageRequest,
    pub providers: Vec<CatalogProviderSearchDto>,
}

/// Busca en todos los proveedores registrados (Qobuz hoy) y devuelve las tres
/// secciones de cada uno, con sus fallos parciales.
#[tauri::command]
pub async fn catalog_search(
    state: State<'_, AppState>,
    query: String,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<CatalogSearchResponseDto, String> {
    let page = CatalogPageRequest::new(
        offset.unwrap_or(0),
        limit.unwrap_or(DEFAULT_CATALOG_PAGE_SIZE),
    );
    let gateway = CatalogSearchGateway::new(state.db.clone());
    perform_catalog_search(&state.db, &gateway, &query, page).await
}

/// Núcleo testeable de `catalog_search`: misma validación y mismo reparto.
pub async fn perform_catalog_search(
    db: &DbPool,
    gateway: &CatalogSearchGateway,
    query: &str,
    page: CatalogPageRequest,
) -> Result<CatalogSearchResponseDto, String> {
    let request = CatalogSearchRequest::new(query, page).map_err(|error| error.to_string())?;
    let service_ids = load_service_ids(db).await?;
    let response = gateway.search(&request).await;

    let providers = response
        .providers
        .into_iter()
        .map(|provider| to_provider_dto(provider, &service_ids))
        .collect();

    Ok(CatalogSearchResponseDto {
        query: response.query,
        page: response.page,
        providers,
    })
}

fn to_provider_dto(
    provider: CatalogProviderResponse,
    service_ids: &std::collections::BTreeMap<String, i64>,
) -> CatalogProviderSearchDto {
    let service_id = service_ids.get(&provider.provider.to_lowercase()).copied();
    CatalogProviderSearchDto {
        provider: provider.provider,
        service_id,
        account_id: provider.account_id,
        tracks: provider.tracks,
        albums: provider.albums,
        artists: provider.artists,
    }
}

async fn load_service_ids(db: &DbPool) -> Result<std::collections::BTreeMap<String, i64>, String> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM services")
        .fetch_all(db)
        .await
        .map_err(|error| format!("Failed to resolve catalog provider ids: {error}"))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| (name.to_lowercase(), id))
        .collect())
}

/// Pistas de un álbum del catálogo: primera página acotada del gateway,
/// filtrada por el título del álbum que la ficha muestra.
#[tauri::command]
pub async fn catalog_album_tracks(
    state: State<'_, AppState>,
    provider: String,
    title: String,
    artist: Option<String>,
) -> Result<CatalogSection<CatalogTrack>, String> {
    let gateway = CatalogSearchGateway::new(state.db.clone());
    perform_catalog_album_tracks(&gateway, &provider, &title, artist.as_deref()).await
}

/// Núcleo testeable de `catalog_album_tracks`.
pub async fn perform_catalog_album_tracks(
    gateway: &CatalogSearchGateway,
    provider: &str,
    title: &str,
    artist: Option<&str>,
) -> Result<CatalogSection<CatalogTrack>, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Catalog album title cannot be empty".to_string());
    }

    let query = match artist.map(str::trim).filter(|artist| !artist.is_empty()) {
        Some(artist) => format!("{artist} {title}"),
        None => title.to_string(),
    };
    let request =
        CatalogSearchRequest::new(query, CatalogPageRequest::new(0, MAX_CATALOG_PAGE_SIZE))
            .map_err(|error| error.to_string())?;
    let response = gateway.search(&request).await;
    let provider_response = provider_response(&response, provider)?;

    Ok(filter_album_tracks(provider_response.tracks.clone(), title))
}

/// Álbumes de un artista del catálogo: primera página acotada del gateway,
/// filtrada por el nombre del artista que la ficha muestra.
#[tauri::command]
pub async fn catalog_artist_albums(
    state: State<'_, AppState>,
    provider: String,
    name: String,
) -> Result<CatalogSection<CatalogAlbum>, String> {
    let gateway = CatalogSearchGateway::new(state.db.clone());
    perform_catalog_artist_albums(&gateway, &provider, &name).await
}

/// Núcleo testeable de `catalog_artist_albums`.
pub async fn perform_catalog_artist_albums(
    gateway: &CatalogSearchGateway,
    provider: &str,
    name: &str,
) -> Result<CatalogSection<CatalogAlbum>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Catalog artist name cannot be empty".to_string());
    }

    let request =
        CatalogSearchRequest::new(name, CatalogPageRequest::new(0, MAX_CATALOG_PAGE_SIZE))
            .map_err(|error| error.to_string())?;
    let response = gateway.search(&request).await;
    let provider_response = provider_response(&response, provider)?;

    Ok(filter_artist_albums(provider_response.albums.clone(), name))
}

fn provider_response<'a>(
    response: &'a crate::services::catalog_search::CatalogSearchResponse,
    provider: &str,
) -> Result<&'a CatalogProviderResponse, String> {
    response
        .providers
        .iter()
        .find(|entry| entry.provider.eq_ignore_ascii_case(provider.trim()))
        .ok_or_else(|| format!("Catalog provider '{provider}' is not registered"))
}

/// Recorta la sección a las pistas cuyo álbum declarado es el de la ficha. Si
/// la lectura falló, la sección conserva el fallo tal cual: la ficha muestra
/// «no se pudo consultar», no una lista vacía.
fn filter_album_tracks(
    section: CatalogSection<CatalogTrack>,
    album_title: &str,
) -> CatalogSection<CatalogTrack> {
    let Some(page) = section.page else {
        return section;
    };
    let items: Vec<CatalogTrack> = page
        .items
        .into_iter()
        .filter(|track| {
            track
                .album
                .as_deref()
                .is_some_and(|album| same_text(album, album_title))
        })
        .collect();

    CatalogSection::ok(CatalogPage {
        items,
        // La ficha solo reclama los ítems que ha verificado en esta página; no
        // hereda el total de la búsqueda textual, que cuenta discos ajenos.
        total: None,
        offset: 0,
        limit: page.limit,
    })
}

fn filter_artist_albums(
    section: CatalogSection<CatalogAlbum>,
    artist_name: &str,
) -> CatalogSection<CatalogAlbum> {
    let Some(page) = section.page else {
        return section;
    };
    let items: Vec<CatalogAlbum> = page
        .items
        .into_iter()
        .filter(|album| {
            album
                .artist
                .as_deref()
                .is_some_and(|artist| same_text(artist, artist_name))
        })
        .collect();

    CatalogSection::ok(CatalogPage {
        items,
        total: None,
        offset: 0,
        limit: page.limit,
    })
}

fn same_text(left: &str, right: &str) -> bool {
    left.trim().to_lowercase() == right.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::catalog_search::{CatalogSearchErrorKind, CatalogSearchFailure};

    #[test]
    fn album_filter_keeps_only_the_declared_album() {
        let section = CatalogSection::ok(CatalogPage {
            items: vec![
                track("One More Time", Some("Discovery")),
                track("Aerodynamic", Some("Discovery")),
                track("Around the World", Some("Homework")),
                track("Untitled", None),
            ],
            total: Some(40),
            offset: 0,
            limit: 100,
        });

        let filtered = filter_album_tracks(section, "DISCOVERY");
        let page = filtered.page.expect("filtered page");
        assert_eq!(page.items.len(), 2);
        assert_eq!(
            page.total, None,
            "no hereda el total de la búsqueda textual"
        );
        assert!(page
            .items
            .iter()
            .all(|item| item.album.as_deref() == Some("Discovery")));
    }

    #[test]
    fn album_filter_preserves_provider_failure() {
        let failure = CatalogSearchFailure {
            provider: "qobuz".to_string(),
            kind: CatalogSearchErrorKind::AuthExpired,
            message: "expired".to_string(),
            retry_after_sec: None,
        };
        let filtered = filter_album_tracks(CatalogSection::failed(failure.clone()), "Discovery");
        assert_eq!(filtered.error, Some(failure));
        assert!(filtered.page.is_none());
    }

    #[test]
    fn artist_filter_keeps_only_the_declared_artist() {
        let section = CatalogSection::ok(CatalogPage {
            items: vec![
                album("Discovery", Some("Daft Punk")),
                album("Homework", Some("Daft Punk")),
                album("Random Access Memories", Some("Daft Punk feat. Panda Bear")),
            ],
            total: Some(30),
            offset: 0,
            limit: 100,
        });

        let filtered = filter_artist_albums(section, "daft punk");
        let page = filtered.page.expect("filtered page");
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, None);
    }

    fn track(title: &str, album: Option<&str>) -> CatalogTrack {
        CatalogTrack {
            id: format!("id-{title}"),
            title: title.to_string(),
            artist: "Daft Punk".to_string(),
            album: album.map(str::to_string),
            isrc: None,
            duration_ms: 1_000,
            bit_depth: None,
            sample_rate: None,
        }
    }

    fn album(title: &str, artist: Option<&str>) -> CatalogAlbum {
        CatalogAlbum {
            id: format!("id-{title}"),
            title: title.to_string(),
            artist: artist.map(str::to_string),
            track_count: Some(14),
            released_at: None,
            cover_url: None,
        }
    }
}
