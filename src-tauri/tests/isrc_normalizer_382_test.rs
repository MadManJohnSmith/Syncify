//! #382: un único normalizador de ISRC para la identidad y el matcher.
//!
//! El mismo código de grabación debe enlazar al mismo Track aunque el
//! proveedor lo entregue con guiones, espacios o distinto caso. Dos ISRC
//! realmente distintos siguen siendo dos grabaciones y no se fusionan.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{SqliteConnection, SqlitePool};
use syncify_core_domain::metadata::ProviderTrackIdentity;
use syncify_tauri_lib::services::track_identity::{
    normalize_isrc, resolve_identity, IdentityBasis, IdentityOutcome, IdentityRequest,
};
use syncify_tauri_lib::services::track_matcher::find_or_create_track_with_identity;

async fn pool() -> SqlitePool {
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

/// Siembra un Track con su artista primario y devuelve su id.
async fn seed_track(
    conn: &mut SqliteConnection,
    artist: &str,
    title: &str,
    duration_ms: i64,
    isrc: &str,
) -> i64 {
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES (?) ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id = id RETURNING id",
    )
    .bind(artist)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(title)
    .bind(duration_ms)
    .bind(isrc)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&mut *conn)
        .await
        .unwrap();
    track_id
}

fn identity_request<'a>(
    artist: &'a str,
    title: &'a str,
    duration_ms: Option<i64>,
    isrc: Option<&'a str>,
) -> IdentityRequest<'a> {
    IdentityRequest {
        service_id: 1,
        service_track_id: "item-382",
        account_id: None,
        title,
        artist,
        album: Some("Album"),
        duration_ms,
        isrc,
        explicit: None,
    }
}

fn provider_identity(isrc: &str) -> ProviderTrackIdentity {
    ProviderTrackIdentity {
        service_id: 1,
        service_name: "fixture".to_string(),
        service_track_id: "item-382".to_string(),
        isrc: Some(isrc.to_string()),
        title: Some("Matcher Song".to_string()),
        artist: Some("Matcher Artist".to_string()),
        duration_ms: Some(200_000),
        ..Default::default()
    }
}

#[test]
fn el_normalizador_es_canonico_y_no_iguala_isrc_distintos() {
    assert_eq!(normalize_isrc(" us-s1z-24-00001 "), "USS1Z2400001");
    assert_eq!(normalize_isrc("US S1Z 24 00001"), "USS1Z2400001");
    assert_eq!(normalize_isrc("us\ts1z\t24\t00001"), "USS1Z2400001");
    assert_eq!(normalize_isrc("USS1Z2400001"), "USS1Z2400001");

    assert_eq!(normalize_isrc("---"), "");
    assert_ne!(
        normalize_isrc("USS1Z2400001"),
        normalize_isrc("USS1Z2400002")
    );
}

#[tokio::test]
async fn un_isrc_con_guiones_espacios_y_minusculas_enlaza_al_track_compacto() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let existing = seed_track(
        &mut conn,
        "Normalizer Artist",
        "Canonical Song",
        200_000,
        "USS1Z2400001",
    )
    .await;

    let request = identity_request(
        "Normalizer Artist",
        "Otro título",
        None,
        Some(" us-s1z-24-00001 "),
    );
    assert_eq!(
        resolve_identity(&mut conn, &request).await.unwrap(),
        IdentityOutcome::Linked {
            track_id: existing,
            basis: IdentityBasis::Isrc
        }
    );
}

#[tokio::test]
async fn un_isrc_guardado_con_guiones_o_espacios_enlaza_al_item_compacto() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let hyphenated = seed_track(
        &mut conn,
        "Normalizer Artist",
        "Hyphen Song",
        200_000,
        "US-S1Z-24-00001",
    )
    .await;
    let spaced = seed_track(
        &mut conn,
        "Normalizer Artist",
        "Spaced Song",
        200_000,
        " us s1z 24 00002 ",
    )
    .await;

    let hyphenated_request = identity_request(
        "Normalizer Artist",
        "Hyphen Song",
        Some(200_000),
        Some("USS1Z2400001"),
    );
    assert_eq!(
        resolve_identity(&mut conn, &hyphenated_request)
            .await
            .unwrap(),
        IdentityOutcome::Linked {
            track_id: hyphenated,
            basis: IdentityBasis::Isrc
        }
    );

    let spaced_request = identity_request(
        "Normalizer Artist",
        "Spaced Song",
        Some(200_000),
        Some("USS1Z2400002"),
    );
    assert_eq!(
        resolve_identity(&mut conn, &spaced_request).await.unwrap(),
        IdentityOutcome::Linked {
            track_id: spaced,
            basis: IdentityBasis::Isrc
        }
    );
}

#[tokio::test]
async fn dos_isrc_distintos_no_se_fusionan_aunque_cambie_el_formato() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_track(
        &mut conn,
        "Normalizer Artist",
        "Shared Song",
        200_000,
        "US-S1Z-24-00001",
    )
    .await;

    // Mismo artista, título y duración; solo cambia el último dígito del ISRC.
    let request = identity_request(
        "Normalizer Artist",
        "Shared Song",
        Some(200_000),
        Some("us-s1z-24-00002"),
    );
    assert_eq!(
        resolve_identity(&mut conn, &request).await.unwrap(),
        IdentityOutcome::New,
        "dos ISRC distintos son dos grabaciones"
    );
}

#[tokio::test]
async fn el_matcher_enlaza_un_isrc_con_guiones_al_track_guardado_compacto() {
    let pool = pool().await;
    let existing: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Matcher Song', 'USS1Z2400001') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let matched =
        find_or_create_track_with_identity(&pool, &provider_identity("us-s1z-24-00001"), None)
            .await
            .unwrap();

    assert!(
        !matched.is_new,
        "el ISRC canónico enlaza el Track existente"
    );
    assert_eq!(matched.track_id, existing);
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 1, "no se duplica el Track");
}

#[tokio::test]
async fn el_matcher_enlaza_un_isrc_compacto_al_track_guardado_con_guiones() {
    let pool = pool().await;
    let existing: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Matcher Song', 'US-S1Z-24-00001') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let matched =
        find_or_create_track_with_identity(&pool, &provider_identity("USS1Z2400001"), None)
            .await
            .unwrap();

    assert!(!matched.is_new, "la forma cruda guardada también enlaza");
    assert_eq!(matched.track_id, existing);
}
