//! Tests de búsqueda FTS de `query_library` — L02 (issue #99, spec #96).
//!
//! Criterios del issue cubiertos aquí:
//! - búsqueda por título, artista, álbum, género e ISRC;
//! - relevancia bm25 ponderada: título > artista > álbum > género > ISRC;
//! - ISRC exacto: normalizado sin guiones, DELANTE de la relevancia, y sin
//!   búsqueda por prefijo cuando el texto tiene forma de ISRC;
//! - escape de caracteres especiales de FTS y último token en prefijo;
//! - texto vacío: se usa el orden elegido;
//! - búsqueda combinada con los filtros (no los sustituye);
//! - sin array de resultados paralelo: la búsqueda vive en `tracks` del
//!   resultado (conjunto de claves del contrato fijado).
//!
//! Fixture: SQLite `:memory:` recién migrada (migraciones reales, 0107
//! incluida — el índice FTS se llena por el rebuild de la migración y sus
//! triggers). La costura probada es `query_library` con el pool.

use serde_json::Value;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library_query::{
    query_library, LibraryQuery, LibrarySortKey, SortDirection,
};

async fn fresh_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

/// Fixture de búsqueda. Cada pista lleva el término `midnight` en UNA sola
/// columna y TODAS las filas tienen columnas de longitud uniforme (2 tokens
/// en título/artista/álbum/género, 1 en ISRC): bm25 normaliza por longitud,
/// y sin uniformidad el orden dejaría de seguir los pesos. Con pesos bm25
/// 10/5/3/2/1 el orden esperado para «Midnight» es T1..T5. Grupo aparte
/// para el ISRC exacto:
///   T1 "Midnight Rain"        — título,   álbum Alpha Set, género Folk Extra
///   T2 "Blue Song"            — ARTISTA "Midnight Jazz", álbum Blue Set
///   T3 "Third Song"           — álbum "After Midnight", género Classical Extra
///   T4 "Fourth Song"          — género "Midnight Rock"
///   T5 "Fifth Song"           — isrc "MIDNIGHT" (token en la columna isrc)
///   T6 "MDNGHT000123 Remix"   — título con el ISRC exacto de T7
///   T7 "Seventh Song"         — isrc "MD-NGHT-000123" (normalizado MDNGHT000123)
///   T8 "Eighth Song"          — isrc "MDNGHT00012399" (prefijo compartido con T7)
///   T9 "Rain Song"            — término neutro para los tests de escape
/// Todas con pertenencia viva (library_entries de la cuenta 1).
async fn seed() -> SqlitePool {
    let pool = fresh_pool().await;
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES (1, 1, 'A1', 'a1@x')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO artists (id, name) VALUES \
         (1, 'Alpha Band'), (2, 'Midnight Jazz'), (3, 'Gamma One')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO albums (id, title) VALUES \
         (1, 'Alpha Set'), (2, 'Blue Set'), (3, 'After Midnight'), (4, 'Gamma Set')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, genre, isrc, is_favorite) VALUES \
         (1, 'Midnight Rain',       1, 'Folk Extra',      'AAAAA0000001',    1), \
         (2, 'Blue Song',           2, 'Blues Extra',     'AAAAA0000002',    0), \
         (3, 'Third Song',          3, 'Classical Extra', 'AAAAA0000003',    0), \
         (4, 'Fourth Song',         4, 'Midnight Rock',   'AAAAA0000004',    0), \
         (5, 'Fifth Song',          4, 'Ambient Extra',   'MIDNIGHT',        0), \
         (6, 'MDNGHT000123 Remix',  4, 'Electro Extra',   'AAAAA0000006',    0), \
         (7, 'Seventh Song',        4, 'Electro Extra',   'MD-NGHT-000123',  0), \
         (8, 'Eighth Song',         4, 'Electro Extra',   'MDNGHT00012399',  0), \
         (9, 'Rain Song',           1, 'Folk Extra',      'AAAAA0000009',    0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES \
         (1, 1, 'primary'), (2, 2, 'primary'), (3, 3, 'primary'), \
         (4, 3, 'primary'), (5, 3, 'primary'), (6, 3, 'primary'), \
         (7, 3, 'primary'), (8, 3, 'primary'), (9, 1, 'primary')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) VALUES \
         (1, 1, '2026-01-01T10:00:00Z', 1), (1, 2, NULL, 1), (1, 3, NULL, 1), \
         (1, 4, NULL, 1), (1, 5, NULL, 1), (1, 6, NULL, 1), (1, 7, NULL, 1), \
         (1, 8, NULL, 1), (1, 9, NULL, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

fn q() -> LibraryQuery {
    LibraryQuery::default()
}

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
// Búsqueda por cada columna indexada
// ==========================================================================

#[tokio::test]
async fn busca_por_titulo_artista_album_genero_e_isrc() {
    let pool = seed().await;

    // Título (AND de tokens: «midnight» y «rain» juntos, solo T1).
    assert_eq!(ids(&pool, text("Midnight Rain")).await, vec![1]);
    // Artista: solo el artista «Midnight Jazz» reúne los tres tokens.
    assert_eq!(ids(&pool, text("Midnight Jazz")).await, vec![2]);
    // Álbum: «after» + «midnight» juntos, solo el álbum de T3.
    assert_eq!(ids(&pool, text("After Midnight")).await, vec![3]);
    // Género (término exacto de columna género).
    assert_eq!(ids(&pool, text("Ambient")).await, vec![5]);
    // ISRC con guiones: se normaliza y se encuentra.
    assert_eq!(ids(&pool, text("MD-NGHT-000123")).await.first(), Some(&7));
    assert_eq!(ids(&pool, text("md-nght-000123")).await.first(), Some(&7));
}

#[tokio::test]
async fn relevancia_bm25_titulo_artista_album_genero_isrc() {
    let pool = seed().await;
    // Un solo término, cada pista lo lleva en una sola columna: el orden de
    // bm25 con pesos (título 10, artista 5, álbum 3, género 2, isrc 1) es
    // T1 (título), T2 (artista), T3 (álbum), T4 (género), T5 (isrc).
    assert_eq!(ids(&pool, text("Midnight")).await, vec![1, 2, 3, 4, 5]);
}

fn text(s: &str) -> LibraryQuery {
    let mut query = q();
    query.text = Some(s.to_owned());
    query
}

// ==========================================================================
// ISRC exacto: delante de la relevancia y sin prefijo
// ==========================================================================

#[tokio::test]
async fn isrc_exacto_va_delante_de_la_relevancia() {
    let pool = seed().await;
    // T6 lleva el mismo token en el TÍTULO (peso 10) y T7 en el ISRC (peso
    // 1): sin el boost exacto ganaría el título; con él, el ISRC exacto va
    // delante.
    assert_eq!(ids(&pool, text("MDNGHT000123")).await, vec![7, 6]);
}

#[tokio::test]
async fn isrc_con_forma_de_isrc_no_se_busca_por_prefijo() {
    let pool = seed().await;
    // T8 tiene un ISRC cuyo token COMPARTIRÍA prefijo con la consulta
    // truncada; con texto de forma ISRC (12 caracteres) solo vale la
    // coincidencia exacta: T8 fuera.
    let result = ids(&pool, text("MDNGHT000123")).await;
    assert!(!result.contains(&8), "sin prefijo: T8 no puede salir");
}

#[tokio::test]
async fn prefijo_del_ultimo_token_para_escritura_incremental() {
    let pool = seed().await;
    // Escritura incremental: texto parcial (no de forma ISRC) encuentra por
    // prefijo el ISRC que acaba empezando igual.
    assert!(ids(&pool, text("MDNGHT0001")).await.contains(&7));
}

// ==========================================================================
// Escape de caracteres especiales de FTS
// ==========================================================================

#[tokio::test]
async fn caracteres_especiales_de_fts_se_escapan_sin_error() {
    let pool = seed().await;
    // Comillas, paréntesis, palabras clave de FTS y asteriscos del usuario:
    // nada debe romper la consulta ni reinterpretarse como sintaxis.
    for text_input in [
        "Rain \"quoted\" (live)",
        "Sun\"rise",
        "Rain OR NOT NEAR",
        "Rain * : ^ -",
        "((",
        "\")\"",
    ] {
        let result = query_library(&pool, text(text_input)).await;
        assert!(result.is_ok(), "«{text_input}» no debe romper MATCH");
    }
    // «Rain» a secas sigue encontrando sus pistas tras los escapes.
    assert_eq!(ids(&pool, text("Rain")).await, vec![1, 9]);
}

// ==========================================================================
// Texto vacío: se usa el orden elegido
// ==========================================================================

#[tokio::test]
async fn texto_vacio_usa_el_orden_elegido() {
    let pool = seed().await;
    let mut query = q();
    query.sort = LibrarySortKey::Title;
    query.direction = SortDirection::Desc;
    let expected = ids(&pool, query.clone()).await; // texto None: orden elegido

    // Texto vacío y solo de espacios: mismo orden que sin texto.
    let mut vacio = query.clone();
    vacio.text = Some(String::new());
    assert_eq!(ids(&pool, vacio).await, expected);
    let mut espacios = query.clone();
    espacios.text = Some("   ".into());
    assert_eq!(ids(&pool, espacios.clone()).await, expected);
    // No es el orden por defecto (DateAdded): el elegido (título desc) manda.
    let mut sin_orden_elegido = q();
    sin_orden_elegido.text = Some(String::new());
    assert_ne!(
        ids(&pool, espacios.clone()).await,
        ids(&pool, sin_orden_elegido).await
    );

    // Con texto el orden manda la relevancia, no el elegido.
    let mut con_texto = query;
    con_texto.text = Some("Midnight".into());
    assert_eq!(ids(&pool, con_texto).await, vec![1, 2, 3, 4, 5]);
}

// ==========================================================================
// Búsqueda combinada con los filtros (no los sustituye)
// ==========================================================================

#[tokio::test]
async fn busqueda_se_combina_con_los_filtros() {
    let pool = seed().await;
    // Texto + favoritos: solo la favorita que coincide.
    let mut query = text("Midnight");
    query.favorites_only = true;
    assert_eq!(ids(&pool, query).await, vec![1]);
    // Texto + género exacto: acota entre las que coinciden por texto.
    let mut query = text("Midnight");
    query.genre = vec!["Blues Extra".into()];
    assert_eq!(ids(&pool, query).await, vec![2]);
    // Texto + filtro que excluye todo lo coincidente: vacío (AND, no OR).
    let mut query = text("Midnight");
    query.genre = vec!["Inexistente".into()];
    assert!(ids(&pool, query).await.is_empty());
    // Los contadores de la cabecera respetan la búsqueda combinada.
    let mut query = text("Midnight");
    query.favorites_only = true;
    let result = query_library(&pool, query).await.unwrap();
    assert_eq!(result.total, 1);
    let counts = result.counts.expect("primera página trae contadores");
    assert_eq!(counts.total, 1);
}

// ==========================================================================
// Sin array de resultados paralelo
// ==========================================================================

#[tokio::test]
async fn la_busqueda_vive_en_el_mismo_array_tracks_sin_array_paralelo() {
    let pool = seed().await;
    let result = query_library(&pool, text("Midnight")).await.unwrap();
    // El conjunto de claves del resultado es exactamente el del contrato:
    // no hay un campo «search»/«results» paralelo al array de la lista.
    let json = serde_json::to_value(&result).unwrap();
    let Value::Object(map) = json else {
        panic!("el resultado debe serializar a objeto");
    };
    let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["counts", "hasMore", "limit", "offset", "total", "tracks"]
    );
    // Y las coincidencias están EN el array de la lista.
    let track_ids: Vec<i64> = result.tracks.iter().map(|t| t.id).collect();
    assert_eq!(track_ids, vec![1, 2, 3, 4, 5]);
}
