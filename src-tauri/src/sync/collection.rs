//! Recorrido completo de colecciones paginadas (playlists, álbumes y
//! artistas) para los adaptadores de proveedor.
//!
//! El contrato del ADR-0006 (decisiones 11-12) exige recorrer **todas** las
//! páginas y asignar el total una sola vez por colección: los proveedores
//! repiten el mismo total en cada página, así que sumarlo por página
//! sobrecuenta. Estas funciones son el único lugar donde vive esa decisión;
//! los adaptadores solo saben pedir una página.

use super::source::{CollectionPage, SourceError};

/// Resultado de recorrer una colección entera.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectedCollection {
    /// Ítems en el orden en que el proveedor los devolvió, concatenando
    /// páginas. No hay parada temprana ni reordenación local.
    pub items: Vec<serde_json::Value>,
    /// Total publicado por el proveedor, leído una vez. `None` cuando el
    /// proveedor no lo publica: la cobertura queda desconocida.
    pub reported_total: Option<i64>,
    /// Huella por ítem, alineada con `items` (rellena con `None` si una
    /// página no trae huellas).
    pub fingerprints: Vec<Option<String>>,
    /// Número de páginas no vacías servidas.
    pub pages: usize,
}

fn assign_total(slot: &mut Option<i64>, page_total: Option<i64>) {
    if slot.unwrap_or(0) > 0 {
        return;
    }
    if let Some(total) = page_total.filter(|total| *total > 0) {
        *slot = Some(total);
    }
}

fn append_page(collected: &mut CollectedCollection, page: CollectionPage) -> usize {
    let len = page.items.len();
    collected.fingerprints.extend(
        page.fingerprints
            .into_iter()
            .chain(std::iter::repeat_n(None, len))
            .take(len),
    );
    collected.items.extend(page.items);
    collected.pages += 1;
    len
}

/// Recorre todas las páginas de una colección con paginación por offset.
///
/// - El total se toma de la primera página que publique uno positivo y no se
///   vuelve a tocar: repetirlo en cada página no lo multiplica.
/// - Avanza por la longitud REAL de la página; un total conocido manda sobre
///   una página corta (filtrado del servidor).
/// - Una página vacía cierra el recorrido.
pub async fn collect_all_pages<F, Fut>(
    page_size: i64,
    mut fetch: F,
) -> Result<CollectedCollection, SourceError>
where
    F: FnMut(i64) -> Fut,
    Fut: std::future::Future<Output = Result<CollectionPage, SourceError>>,
{
    assert!(page_size > 0, "page size must be positive");
    let mut offset: i64 = 0;
    let mut collected = CollectedCollection::default();

    loop {
        let page = fetch(offset).await?;
        assign_total(&mut collected.reported_total, page.reported_total);
        if page.items.is_empty() {
            break;
        }
        let page_len = append_page(&mut collected, page);
        match crate::services::import_pagination::next_offset(
            offset as i32,
            page_len as i32,
            page_size as i32,
            collected.reported_total,
        ) {
            Some(next) => offset = next as i64,
            None => break,
        }
    }

    Ok(collected)
}

/// Recorre todas las páginas de una colección con paginación por cursor
/// (Spotify `/me/following`): el cursor lo publica cada página y el total, si
/// existe, decide cuándo parar.
pub async fn collect_cursor_pages<F, Fut>(
    page_size: i64,
    mut fetch: F,
) -> Result<CollectedCollection, SourceError>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: std::future::Future<Output = Result<CollectionPage, SourceError>>,
{
    assert!(page_size > 0, "page size must be positive");
    let mut after: Option<String> = None;
    let mut collected = CollectedCollection::default();

    loop {
        let page = fetch(after.clone()).await?;
        assign_total(&mut collected.reported_total, page.reported_total);
        if page.items.is_empty() {
            break;
        }
        let next_cursor = page.next_cursor.clone();
        let page_len = append_page(&mut collected, page);
        after = crate::services::import_pagination::next_cursor(
            next_cursor,
            page_len,
            page_size as i32,
            collected.reported_total,
            collected.items.len() as u64,
        );
        if after.is_none() {
            break;
        }
    }

    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(items: Vec<serde_json::Value>, total: Option<i64>) -> CollectionPage {
        CollectionPage {
            items,
            reported_total: total,
            fingerprints: Vec::new(),
            next_cursor: None,
        }
    }

    #[tokio::test]
    async fn walks_every_offset_page_and_keeps_provider_order() {
        let pages = vec![
            page(vec![json!({"id": 1}), json!({"id": 2})], Some(5)),
            page(vec![json!({"id": 3}), json!({"id": 4})], Some(5)),
            page(vec![json!({"id": 5})], Some(5)),
        ];
        let mut seen = Vec::new();
        let collected = collect_all_pages(2, |offset| {
            seen.push(offset);
            let page = pages[(offset / 2) as usize].clone();
            async move { Ok(page) }
        })
        .await
        .expect("walk");

        assert_eq!(seen, vec![0, 2, 4], "recorre todas las páginas en orden");
        let ids: Vec<i64> = collected
            .items
            .iter()
            .map(|item| item["id"].as_i64().unwrap())
            .collect();
        assert_eq!(ids, vec![1, 2, 3, 4, 5]);
        assert_eq!(collected.pages, 3);
    }

    #[tokio::test]
    async fn reports_the_total_once_instead_of_accumulating_it_per_page() {
        // El proveedor repite total=4 en cada página; sumarlo daría 8.
        let pages = vec![
            page(vec![json!({"id": 1}), json!({"id": 2})], Some(4)),
            page(vec![json!({"id": 3}), json!({"id": 4})], Some(4)),
        ];
        let collected = collect_all_pages(2, |offset| {
            let page = pages[(offset / 2) as usize].clone();
            async move { Ok(page) }
        })
        .await
        .expect("walk");

        assert_eq!(collected.reported_total, Some(4));
        assert_eq!(collected.items.len(), 4);
    }

    #[tokio::test]
    async fn keeps_walking_a_short_page_when_the_total_promises_more() {
        let pages = vec![
            page(vec![json!({"id": 1})], Some(3)),
            page(vec![json!({"id": 2}), json!({"id": 3})], Some(3)),
        ];
        let mut calls = 0;
        let collected = collect_all_pages(2, |offset| {
            let page = if offset == 0 {
                pages[0].clone()
            } else {
                pages[1].clone()
            };
            calls += 1;
            async move { Ok(page) }
        })
        .await
        .expect("walk");

        assert_eq!(calls, 2, "una página corta no corta si el total dice más");
        assert_eq!(collected.items.len(), 3);
    }

    #[tokio::test]
    async fn unknown_total_stops_on_the_first_short_page() {
        let pages = vec![
            page(vec![json!({"id": 1})], None),
            page(vec![json!({"id": 2}), json!({"id": 3})], None),
        ];
        let mut calls = 0;
        let collected = collect_all_pages(2, |offset| {
            let page = pages[(offset / 2) as usize].clone();
            calls += 1;
            async move { Ok(page) }
        })
        .await
        .expect("walk");

        assert_eq!(calls, 1, "sin total, una página corta es el final");
        assert_eq!(collected.reported_total, None);
        assert_eq!(collected.items.len(), 1);
    }

    #[tokio::test]
    async fn cursor_walk_follows_every_cursor_and_reads_the_total_once() {
        let pages = vec![
            CollectionPage {
                items: vec![json!({"id": "a"}), json!({"id": "b"})],
                reported_total: Some(3),
                fingerprints: vec![None, None],
                next_cursor: Some("after-b".to_string()),
            },
            CollectionPage {
                items: vec![json!({"id": "c"})],
                reported_total: Some(3),
                fingerprints: vec![None],
                next_cursor: None,
            },
        ];
        let mut cursors: Vec<Option<String>> = Vec::new();
        let collected = collect_cursor_pages(2, |after| {
            cursors.push(after);
            let page = pages[cursors.len() - 1].clone();
            async move { Ok(page) }
        })
        .await
        .expect("walk");

        assert_eq!(cursors, vec![None, Some("after-b".to_string())]);
        assert_eq!(collected.reported_total, Some(3));
        assert_eq!(collected.items.len(), 3);
    }
}
