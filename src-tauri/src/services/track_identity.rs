//! Identidad de pista del sync (issue #73, ADR-0006 decisión 16).
//!
//! Una sola función decide a qué Track canónico pertenece un ítem del
//! proveedor antes de tocarlo:
//!
//!   * **ISRC primero.** Un ISRC idéntico enlaza automáticamente: el mismo
//!     código de grabación exportado por otro servicio es la misma grabación.
//!   * **Huella después.** Sin ISRC local, la evidencia es artista + título +
//!     duración dentro de [`DURATION_TOLERANCE_MS`]. Los tres tienen que
//!     coincidir: **el título por sí solo nunca enlaza**, y una duración que
//!     falta no es evidencia de que la grabación sea la misma.
//!   * **Lo ambiguo no se fusiona.** Si la huella encuentra más de un Track, se
//!     devuelve [`IdentityOutcome::Ambiguous`] con la evidencia de ambos lados
//!     para que el llamador la registre en `track_identity_reviews` (0115) y
//!     decida una persona. El sync importa el ítem como Track propio: no
//!     fusiona ni lo pierde.
//!
//! El ISRC del ítem manda sobre la huella: dos ISRC distintos son dos
//! grabaciones distintas y no se fusionan aunque el resto coincida. Si el
//! candidato local no tiene ISRC, la huella sí puede enlazarlo (el proveedor
//! omitió el código, no lo contradijo).
//!
//! El módulo no escribe fuera de la cola de revisión: la persistencia del Track
//! sigue siendo de `EnrichmentEngine::enrich_and_persist_sync_track`.

use std::collections::BTreeMap;

use sqlx::SqliteConnection;
use syncify_core_domain::metadata::{
    clean_mojibake, decode_html_entities, is_placeholder_artist, is_placeholder_title,
    strip_artist_role_annotations,
};

use crate::services::track_matcher::is_explicit_compatible;

/// Tolerancia de duración de la huella, en milisegundos. Misma que el matcher
/// de descargas (`services::track_matcher`): dos proveedores redondean distinto
/// el mismo máster, pero dos ediciones distintas se separan mucho más.
pub const DURATION_TOLERANCE_MS: i64 = 2_000;

/// Motivo del único caso que hoy llega a la cola de revisión: la huella
/// encontró más de un Track local. Es vocabulario del núcleo, no del
/// proveedor; la cola (0115) no tiene CHECK para que un motivo nuevo sea un
/// cambio de código y no otra migración.
pub const REASON_FINGERPRINT_MULTIPLE_CANDIDATES: &str = "fingerprint_multiple_candidates";

/// El lado del proveedor que llega al sync.
pub struct IdentityRequest<'a> {
    pub service_id: i64,
    pub service_track_id: &'a str,
    pub account_id: Option<i64>,
    pub title: &'a str,
    pub artist: &'a str,
    pub album: Option<&'a str>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<&'a str>,
    pub explicit: Option<bool>,
}

/// Qué prueba sostiene el enlace. Se registra para diagnóstico: un enlace por
/// huella no es el mismo hecho que uno por ISRC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityBasis {
    Isrc,
    Fingerprint,
}

/// Un Track local que compite por el ítem, con la evidencia congelada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityCandidate {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub duration_delta_ms: Option<i64>,
}

/// Decisión de identidad. `Ambiguous` nunca enlaza: el llamador importa el ítem
/// como Track propio y deja la revisión abierta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityOutcome {
    Linked { track_id: i64, basis: IdentityBasis },
    New,
    Ambiguous { candidates: Vec<IdentityCandidate> },
}

/// Texto comparable de identidad: entidades HTML, mojibake, mayúsculas y
/// espacios colapsados. No usa `clean_title`: quitar «(Live)» o «(Remaster)»
/// convertiría dos ediciones distintas en la misma grabación.
fn normalize_identity_text(raw: &str) -> String {
    let decoded = decode_html_entities(raw);
    clean_mojibake(&decoded)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalize_artist(raw: &str) -> String {
    normalize_identity_text(&strip_artist_role_annotations(raw))
}

fn normalize_title(raw: &str) -> String {
    normalize_identity_text(raw)
}

/// Forma canónica de un ISRC: sin guiones ni espacios y en mayúsculas.
///
/// Es la misma identidad que la 0064 aplica a `tracks.isrc`
/// (`UPPER(REPLACE(TRIM(isrc), '-', ''))`), ampliada con los espacios
/// internos que algunos proveedores intercalan. No valida la forma: eso lo
/// decide `is_valid_isrc`. La comparte el matcher de descargas para que no
/// existan dos definiciones de «mismo ISRC».
pub fn normalize_isrc(raw: &str) -> String {
    raw.chars()
        .filter(|character| *character != '-' && !character.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}

fn isrc_evidence(item_isrc: Option<&str>) -> Option<String> {
    item_isrc
        .map(normalize_isrc)
        .filter(|value| !value.is_empty())
}

/// Resuelve la identidad del ítem contra los Tracks canónicos.
///
/// No escribe: la revisión la registra [`record_review`] con lo que devuelve
/// esta función.
pub async fn resolve_identity(
    conn: &mut SqliteConnection,
    request: &IdentityRequest<'_>,
) -> Result<IdentityOutcome, String> {
    if let Some(isrc) = isrc_evidence(request.isrc) {
        // Mismo NOCASE que el índice único de 0064:186; comparar en BINARY
        // trataba `USCGH…` y `uscgh…` como grabaciones distintas.
        let mut matches: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM tracks WHERE isrc = ? COLLATE NOCASE ORDER BY id")
                .bind(&isrc)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| format!("identity ISRC lookup failed: {e}"))?;
        if matches.is_empty() {
            // La 0064 compactó lo que existía al aplicarse, pero varias rutas
            // de proveedor siguen insertando el ISRC tal cual (con guiones o
            // espacios) durante la sesión. Sin este fallback el ítem compacto
            // no enlazaría con esa fila.
            matches = sqlx::query_as(
                "SELECT id FROM tracks
                 WHERE isrc IS NOT NULL
                   AND UPPER(REPLACE(REPLACE(TRIM(isrc), '-', ''), ' ', '')) = ?
                 ORDER BY id",
            )
            .bind(&isrc)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| format!("identity ISRC fallback lookup failed: {e}"))?;
        }
        match matches.as_slice() {
            [] => {}
            [(track_id,)] => {
                return Ok(IdentityOutcome::Linked {
                    track_id: *track_id,
                    basis: IdentityBasis::Isrc,
                })
            }
            // El índice único NOCASE debería impedirlo; si una BD migrada
            // arrastra dos filas, no se elige una al azar.
            _ => return Err("needs_review: ISRC maps to multiple canonical tracks".to_string()),
        }
    }

    let candidates = fingerprint_candidates(conn, request).await?;
    Ok(match candidates.len() {
        0 => IdentityOutcome::New,
        1 => IdentityOutcome::Linked {
            track_id: candidates[0].track_id,
            basis: IdentityBasis::Fingerprint,
        },
        _ => IdentityOutcome::Ambiguous { candidates },
    })
}

/// Candidatos locales para la huella del ítem, ordenados por `track_id`.
async fn fingerprint_candidates(
    conn: &mut SqliteConnection,
    request: &IdentityRequest<'_>,
) -> Result<Vec<IdentityCandidate>, String> {
    // Una duración ausente, 0 o negativa no es evidencia: el repo ya trata
    // `> 0` como conocido (favorites, download/identity) y hay rutas que
    // pasan `Some(0)` sin filtrar.
    let Some(duration_ms) = request.duration_ms.filter(|duration| *duration > 0) else {
        return Ok(Vec::new());
    };
    // Sin artista y título utilizables no hay huella: el título solo no enlaza.
    if is_placeholder_artist(request.artist) || is_placeholder_title(request.title) {
        return Ok(Vec::new());
    }

    let rows: Vec<(
        i64,
        String,
        i64,
        Option<String>,
        Option<i32>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        r#"
            SELECT t.id, t.title, t.duration_ms, t.isrc, t.explicit, al.title, a.name
            FROM tracks t
            LEFT JOIN albums al ON al.id = t.album_id
            JOIN track_artists ta ON ta.track_id = t.id
                AND (ta.role IN ('primary', 'main') OR ta.role IS NULL)
            JOIN artists a ON a.id = ta.artist_id
            WHERE t.duration_ms BETWEEN ? AND ? AND t.duration_ms > 0
            ORDER BY t.id, a.id
            "#,
    )
    .bind(duration_ms - DURATION_TOLERANCE_MS)
    .bind(duration_ms + DURATION_TOLERANCE_MS)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| format!("identity fingerprint lookup failed: {e}"))?;

    // Una fila por (Track, artista primario): varias colaboraciones del mismo
    // Track no son varios candidatos.
    let mut by_track: BTreeMap<
        i64,
        (
            String,
            i64,
            Option<String>,
            Option<i32>,
            Option<String>,
            Vec<String>,
        ),
    > = BTreeMap::new();
    for (track_id, title, track_duration, isrc, explicit, album, artist) in rows {
        by_track
            .entry(track_id)
            .or_insert((title, track_duration, isrc, explicit, album, Vec::new()))
            .5
            .push(artist);
    }

    let artist = normalize_artist(request.artist);
    let title = normalize_title(request.title);
    let item_isrc = isrc_evidence(request.isrc);
    let mut candidates = Vec::new();
    for (track_id, (cand_title, cand_duration, cand_isrc, cand_explicit, album, artists)) in
        by_track
    {
        if !artists.iter().any(|name| normalize_artist(name) == artist) {
            continue;
        }
        if normalize_title(&cand_title) != title {
            continue;
        }
        if !is_explicit_compatible(request.explicit, cand_explicit) {
            continue;
        }
        // Dos ISRC distintos son dos grabaciones: la huella no puede pisarlos.
        // Se comparan en forma canónica para que los guiones o las mayúsculas
        // del proveedor no conviertan el mismo código en una contradicción.
        if let (Some(item_isrc), Some(cand_isrc)) = (item_isrc.as_deref(), cand_isrc.as_deref()) {
            let cand_isrc = normalize_isrc(cand_isrc);
            if !cand_isrc.is_empty() && cand_isrc != item_isrc {
                continue;
            }
        }
        candidates.push(IdentityCandidate {
            track_id,
            title: Some(cand_title),
            artist: artists.first().cloned(),
            album,
            duration_ms: Some(cand_duration),
            isrc: cand_isrc,
            duration_delta_ms: Some((cand_duration - duration_ms).abs()),
        });
    }
    Ok(candidates)
}

/// Registra (o refresca) la revisión `pending` del ítem y congela la evidencia
/// de los candidatos. Idempotente: reintentar el mismo ítem actualiza la fila
/// abierta en vez de acumular duplicados.
pub async fn record_review(
    conn: &mut SqliteConnection,
    request: &IdentityRequest<'_>,
    candidates: &[IdentityCandidate],
) -> Result<i64, String> {
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM track_identity_reviews
         WHERE service_id = ? AND service_track_id = ? AND reason = ? AND status = 'pending'",
    )
    .bind(request.service_id)
    .bind(request.service_track_id)
    .bind(REASON_FINGERPRINT_MULTIPLE_CANDIDATES)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("identity review lookup failed: {e}"))?;

    let review_id = match existing {
        Some(review_id) => {
            sqlx::query(
                "UPDATE track_identity_reviews SET
                    account_id = ?,
                    incoming_title = ?,
                    incoming_artist = ?,
                    incoming_album = ?,
                    incoming_duration_ms = ?,
                    incoming_isrc = ?,
                    updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?",
            )
            .bind(request.account_id)
            .bind(request.title)
            .bind(request.artist)
            .bind(request.album)
            .bind(request.duration_ms)
            .bind(request.isrc)
            .bind(review_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("identity review refresh failed: {e}"))?;
            review_id
        }
        None => sqlx::query_scalar(
            "INSERT INTO track_identity_reviews (
                service_id, service_track_id, account_id, reason,
                incoming_title, incoming_artist, incoming_album,
                incoming_duration_ms, incoming_isrc
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(request.service_id)
        .bind(request.service_track_id)
        .bind(request.account_id)
        .bind(REASON_FINGERPRINT_MULTIPLE_CANDIDATES)
        .bind(request.title)
        .bind(request.artist)
        .bind(request.album)
        .bind(request.duration_ms)
        .bind(request.isrc)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| format!("identity review insert failed: {e}"))?,
    };

    sqlx::query("DELETE FROM track_identity_review_candidates WHERE review_id = ?")
        .bind(review_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("identity review evidence reset failed: {e}"))?;
    for candidate in candidates {
        sqlx::query(
            "INSERT INTO track_identity_review_candidates (
                review_id, track_id, title, artist, album, duration_ms, isrc, duration_delta_ms
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(review_id)
        .bind(candidate.track_id)
        .bind(&candidate.title)
        .bind(&candidate.artist)
        .bind(&candidate.album)
        .bind(candidate.duration_ms)
        .bind(&candidate.isrc)
        .bind(candidate.duration_delta_ms)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("identity review evidence failed: {e}"))?;
    }
    Ok(review_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn pool() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    /// Siembra un Track con su artista primario y devuelve su id.
    async fn track(
        conn: &mut SqliteConnection,
        artist: &str,
        title: &str,
        duration_ms: i64,
        isrc: Option<&str>,
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
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(&mut *conn)
        .await
        .unwrap();
        track_id
    }

    fn request<'a>(
        artist: &'a str,
        title: &'a str,
        duration_ms: Option<i64>,
    ) -> IdentityRequest<'a> {
        IdentityRequest {
            service_id: 1,
            service_track_id: "item-1",
            account_id: None,
            title,
            artist,
            album: Some("Album"),
            duration_ms,
            isrc: None,
            explicit: None,
        }
    }

    #[test]
    fn la_normalizacion_conserva_la_edicion_y_pliega_la_forma() {
        assert_eq!(
            normalize_title("Song  (Remastered 2011)"),
            normalize_title("Song (Remastered 2011)")
        );
        assert_eq!(
            normalize_title("Rock &amp; Roll"),
            normalize_title("rock & roll")
        );
        assert_ne!(
            normalize_title("Song"),
            normalize_title("Song (Live)"),
            "el sufijo de edición es evidencia, no ruido"
        );
        assert_eq!(
            normalize_artist("Nena (CV: Akari)"),
            normalize_artist("nena")
        );
    }

    #[tokio::test]
    async fn el_sin_isrc_enlaza_por_huella_dentro_de_la_tolerancia() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        let existing = track(
            &mut conn,
            "Fingerprint Artist",
            "Fingerprint Song",
            240_000,
            None,
        )
        .await;

        let decision = resolve_identity(
            &mut conn,
            &request("Fingerprint Artist", "Fingerprint Song", Some(241_500)),
        )
        .await
        .unwrap();

        assert_eq!(
            decision,
            IdentityOutcome::Linked {
                track_id: existing,
                basis: IdentityBasis::Fingerprint
            }
        );
    }

    #[tokio::test]
    async fn fuera_de_la_tolerancia_o_sin_duracion_no_enlaza() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        track(
            &mut conn,
            "Fingerprint Artist",
            "Fingerprint Song",
            240_000,
            None,
        )
        .await;

        let tarde = resolve_identity(
            &mut conn,
            &request("Fingerprint Artist", "Fingerprint Song", Some(242_001)),
        )
        .await
        .unwrap();
        assert_eq!(tarde, IdentityOutcome::New);

        let sin_duracion = resolve_identity(
            &mut conn,
            &request("Fingerprint Artist", "Fingerprint Song", None),
        )
        .await
        .unwrap();
        assert_eq!(
            sin_duracion,
            IdentityOutcome::New,
            "sin duración no hay huella: el título solo no enlaza"
        );
    }

    #[tokio::test]
    async fn el_artista_forma_parte_de_la_huella() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        track(&mut conn, "Artist One", "Shared Title", 180_000, None).await;

        let otro = resolve_identity(
            &mut conn,
            &request("Artist Two", "Shared Title", Some(180_000)),
        )
        .await
        .unwrap();
        assert_eq!(otro, IdentityOutcome::New);
    }

    #[tokio::test]
    async fn dos_candidatos_son_ambiguos_y_la_revision_guarda_ambos_lados() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        let a = track(
            &mut conn,
            "Twice",
            "Same Song",
            200_000,
            Some("ED73DUPL0001"),
        )
        .await;
        let b = track(
            &mut conn,
            "Twice",
            "Same Song",
            200_900,
            Some("ED73DUPL0002"),
        )
        .await;

        let decision = resolve_identity(&mut conn, &request("Twice", "Same Song", Some(200_400)))
            .await
            .unwrap();
        let IdentityOutcome::Ambiguous { candidates } = decision else {
            panic!("dos candidatos tienen que ser ambiguos, no un enlace");
        };
        assert_eq!(
            candidates.iter().map(|c| c.track_id).collect::<Vec<_>>(),
            vec![a, b]
        );

        let review_id = record_review(
            &mut conn,
            &request("Twice", "Same Song", Some(200_400)),
            &candidates,
        )
        .await
        .unwrap();
        let evidence: Vec<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT track_id, duration_delta_ms FROM track_identity_review_candidates
             WHERE review_id = ? ORDER BY track_id",
        )
        .bind(review_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap();
        assert_eq!(evidence, vec![(a, Some(400)), (b, Some(500))]);
        let incoming: (String, String, Option<i64>) = sqlx::query_as(
            "SELECT incoming_artist, incoming_title, incoming_duration_ms
             FROM track_identity_reviews WHERE id = ?",
        )
        .bind(review_id)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        assert_eq!(
            incoming,
            ("Twice".to_string(), "Same Song".to_string(), Some(200_400))
        );
    }

    #[tokio::test]
    async fn registrar_la_misma_revision_dos_veces_no_duplica_la_cola() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        let a = track(&mut conn, "Twice", "Same Song", 200_000, None).await;
        let b = track(&mut conn, "Twice", "Same Song", 200_900, None).await;
        let request = request("Twice", "Same Song", Some(200_400));
        let IdentityOutcome::Ambiguous { candidates } =
            resolve_identity(&mut conn, &request).await.unwrap()
        else {
            panic!("dos candidatos tienen que ser ambiguos");
        };

        let first = record_review(&mut conn, &request, &candidates)
            .await
            .unwrap();
        let second = record_review(&mut conn, &request, &candidates)
            .await
            .unwrap();

        assert_eq!(first, second, "la revisión abierta se refresca");
        let open: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_identity_reviews WHERE status = 'pending'",
        )
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        assert_eq!(open, 1);
        let evidence: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_identity_review_candidates WHERE review_id = ?",
        )
        .bind(first)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        assert_eq!(evidence, 2, "la evidencia no se acumula al reintentar");
        assert_eq!(
            resolve_identity(&mut conn, &request).await.unwrap(),
            IdentityOutcome::Ambiguous {
                candidates: vec![
                    IdentityCandidate {
                        track_id: a,
                        title: Some("Same Song".into()),
                        artist: Some("Twice".into()),
                        album: None,
                        duration_ms: Some(200_000),
                        isrc: None,
                        duration_delta_ms: Some(400),
                    },
                    IdentityCandidate {
                        track_id: b,
                        title: Some("Same Song".into()),
                        artist: Some("Twice".into()),
                        album: None,
                        duration_ms: Some(200_900),
                        isrc: None,
                        duration_delta_ms: Some(500),
                    },
                ]
            }
        );
    }

    #[tokio::test]
    async fn un_isrc_distinto_no_se_fusiona_por_huella() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        track(&mut conn, "Artist", "Song", 200_000, Some("ED73OTHER001")).await;

        let mut req = request("Artist", "Song", Some(200_000));
        req.isrc = Some("ED73MINE0001");
        assert_eq!(
            resolve_identity(&mut conn, &req).await.unwrap(),
            IdentityOutcome::New,
            "dos ISRC distintos son dos grabaciones"
        );
    }

    #[tokio::test]
    async fn el_isrc_del_candidato_sin_codigo_se_puede_rellenar_por_huella() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        let existing = track(&mut conn, "Artist", "Song", 200_000, None).await;

        let mut req = request("Artist", "Song", Some(200_000));
        req.isrc = Some("ED73MINE0002");
        assert_eq!(
            resolve_identity(&mut conn, &req).await.unwrap(),
            IdentityOutcome::Linked {
                track_id: existing,
                basis: IdentityBasis::Fingerprint
            }
        );
    }

    #[tokio::test]
    async fn el_isrc_identico_enlaza_aunque_la_huella_no_coincida() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        let existing = track(&mut conn, "Artist", "Song", 200_000, Some("ED73SAME0001")).await;

        let mut req = request("Artist", "Otro título", None);
        req.isrc = Some("ed73same0001");
        assert_eq!(
            resolve_identity(&mut conn, &req).await.unwrap(),
            IdentityOutcome::Linked {
                track_id: existing,
                basis: IdentityBasis::Isrc
            }
        );
    }

    #[tokio::test]
    async fn un_artista_o_titulo_placeholder_no_usan_huella() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        track(&mut conn, "Real Artist", "Unknown Track", 200_000, None).await;
        track(&mut conn, "Real Artist", "Shared Title", 200_000, None).await;

        let titulo_placeholder = resolve_identity(
            &mut conn,
            &request("Real Artist", "Unknown Track", Some(200_000)),
        )
        .await
        .unwrap();
        assert_eq!(titulo_placeholder, IdentityOutcome::New);

        // El trigger de 0079 ni siquiera deja guardar «Unknown Artist» como
        // artista; el ítem con ese crédito tampoco puede usar la huella.
        let artista_placeholder = resolve_identity(
            &mut conn,
            &request("Unknown Artist", "Shared Title", Some(200_000)),
        )
        .await
        .unwrap();
        assert_eq!(artista_placeholder, IdentityOutcome::New);
    }

    #[tokio::test]
    async fn el_explicito_incompatible_no_es_candidato() {
        let pool = pool().await;
        let mut conn = pool.acquire().await.unwrap();
        let existing = track(&mut conn, "Artist", "Song", 200_000, None).await;
        sqlx::query("UPDATE tracks SET explicit = 1 WHERE id = ?")
            .bind(existing)
            .execute(&mut *conn)
            .await
            .unwrap();

        let mut req = request("Artist", "Song", Some(200_000));
        req.explicit = Some(false);
        assert_eq!(
            resolve_identity(&mut conn, &req).await.unwrap(),
            IdentityOutcome::New,
            "una versión explícita y una limpia no son la misma pista"
        );
    }
}
