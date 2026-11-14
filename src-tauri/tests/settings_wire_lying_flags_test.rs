//! SB03 (#198): cada ajuste que miente pasa a condicionar comportamiento.
//!
//! Por cada flag cableado hay un test contra BD `:memory:` con valores de
//! fábrica y modificados. La paridad de fábrica es parte del contrato: con
//! los defaults el resultado observable es el histórico (migración 0105
//! alinea la semilla con lo que el escritor ya emitía).

use sqlx::sqlite::SqlitePoolOptions;
use syncify_flac_writer::FlacMetadata;
use syncify_tauri_lib::models::MetadataPreferences;
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, SyncTrackInput};

async fn setup_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");
    pool
}

/// FLAC mínimo válido para etiquetar y para el escáner de la biblioteca.
fn create_test_flac(path: &std::path::Path, genre: Option<&str>) {
    // STREAMINFO real (48 kHz, 2 canales, 24 bits, 48000 muestras): el
    // escáner lee bit_depth del fichero y la BD tiene un CHECK sobre él.
    let packed: u128 = (48000u128 << 44)
        | (1u128 << 41) // channels - 1
        | (23u128 << 36) // bits-per-sample - 1
        | 48000u128; // total samples
    let streaminfo_body: Vec<u8> = {
        let mut v = Vec::new();
        v.extend_from_slice(&4096u16.to_be_bytes()); // min blocksize
        v.extend_from_slice(&4096u16.to_be_bytes()); // max blocksize
        v.extend_from_slice(&[0u8; 6]); // min/max framesize
        v.extend_from_slice(&packed.to_be_bytes()[8..16]);
        v.extend_from_slice(&[0u8; 16]); // md5
        v
    };

    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC");
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]); // STREAMINFO header
    data.extend_from_slice(&streaminfo_body);
    data.extend_from_slice(&[0x81, 0x00, 0x00, 0x00]); // PADDING header, last-block
    data.extend(vec![0xCC; 2048]);
    std::fs::write(path, &data).expect("write flac skeleton");

    let meta = FlacMetadata {
        title: "Cancion Original".to_string(),
        artist: "Artista Original".to_string(),
        album: "Album Original".to_string(),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        bit_depth: Some(24),
        sample_rate: Some(48000.0),
        genre: genre.map(|s| s.to_string()),
        ..Default::default()
    };
    syncify_flac_writer::apply_and_verify_flac_tags(path, &meta).expect("tag fixture flac");
}

fn vorbis_values(path: &std::path::Path, key: &str) -> Vec<String> {
    let tag = metaflac::Tag::read_from_path(path).expect("read flac tags");
    tag.vorbis_comments()
        .and_then(|c| c.get(key).cloned())
        .unwrap_or_default()
}

fn prefs_with_flags(off: &[&str]) -> MetadataPreferences {
    let mut p = MetadataPreferences::default();
    for name in off {
        match *name {
            "write_releasetype" => p.write_releasetype = false,
            "write_label" => p.write_label = false,
            "write_work_composer" => p.write_work_composer = false,
            "write_musicbrainz_ids" => p.write_musicbrainz_ids = false,
            "write_lyrics_tags" => p.write_lyrics_tags = false,
            "write_download_source" => p.write_download_source = false,
            "write_download_date" => p.write_download_date = false,
            "write_quality_score" => p.write_quality_score = false,
            other => panic!("unknown flag in test: {other}"),
        }
    }
    p
}

fn full_meta() -> FlacMetadata {
    FlacMetadata {
        title: "T".to_string(),
        artist: "A".to_string(),
        album: "Al".to_string(),
        composer: Some("Comp".to_string()),
        work: Some("Work".to_string()),
        release_type: Some("album".to_string()),
        label: Some("Label".to_string()),
        lyrics_lrc: Some("[00:01.00] line".to_string()),
        audio_source: Some("Qobuz".to_string()),
        lyrics_source: Some("lrclib".to_string()),
        cover_source: Some("qobuz".to_string()),
        musicbrainz_track_id: Some("mb-track".to_string()),
        musicbrainz_artist_id: Some("mb-artist".to_string()),
        musicbrainz_album_id: Some("mb-album".to_string()),
        download_date: Some("2026-11-14T00:00:00Z".to_string()),
        quality_score: Some("hires".to_string()),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Paridad de fábrica: la migración 0105 alinea la semilla con el historial.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_0105_aligns_seed_to_current_behavior() {
    let pool = setup_db().await;
    let row: (bool, bool, bool, bool) = sqlx::query_as(
        "SELECT write_work_composer, write_lyrics_tags, write_download_source, enable_lastfm
         FROM metadata_preferences WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        row.0,
        "write_work_composer: el escritor siempre emitió WORK/COMPOSER"
    );
    assert!(
        row.1,
        "write_lyrics_tags: el escritor siempre emitió LYRICS"
    );
    assert!(
        row.2,
        "write_download_source: el escritor siempre emitió SYNCIFY_*_SOURCE"
    );
    assert!(
        row.3,
        "enable_lastfm: el fallback de enrich_track ya lo intentaba"
    );

    let d = MetadataPreferences::default();
    assert!(
        d.write_work_composer && d.write_lyrics_tags && d.write_download_source && d.enable_lastfm,
        "los defaults de settings_defaults.rs y la semilla migrada deben coincidir"
    );
}

// ---------------------------------------------------------------------------
// write_*: el DTO se filtra antes de escribir; fábrica = historial.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tag_policy_factory_preserves_historical_keys() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("a.flac");
    create_test_flac(&path, None);

    let mut meta = full_meta();
    let prefs = MetadataPreferences::default();
    syncify_tauri_lib::services::tag_writer::apply_tag_policy(&mut meta, &prefs);
    let opts = syncify_tauri_lib::services::tag_writer::tag_write_options(&prefs);
    syncify_flac_writer::apply_flac_tags_with_options(&path, &meta, &opts).unwrap();

    assert_eq!(vorbis_values(&path, "WORK"), vec!["Work"]);
    assert_eq!(vorbis_values(&path, "COMPOSER"), vec!["Comp"]);
    assert_eq!(vorbis_values(&path, "RELEASETYPE"), vec!["album"]);
    assert_eq!(vorbis_values(&path, "LABEL"), vec!["Label"]);
    assert!(
        !vorbis_values(&path, "LYRICS").is_empty(),
        "LYRICS presente"
    );
    assert_eq!(vorbis_values(&path, "SYNCIFY_AUDIO_SOURCE"), vec!["Qobuz"]);
    assert_eq!(
        vorbis_values(&path, "MUSICBRAINZ_TRACKID"),
        vec!["mb-track"]
    );
    // Los flags de provenancia nuevos están apagados de fábrica: nada escrito.
    assert!(vorbis_values(&path, "SYNCIFY_DOWNLOAD_DATE").is_empty());
    assert!(vorbis_values(&path, "SYNCIFY_QUALITY_SCORE").is_empty());
}

#[tokio::test]
async fn tag_policy_disabled_flags_drop_their_keys() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("b.flac");
    create_test_flac(&path, None);

    let mut meta = full_meta();
    let prefs = prefs_with_flags(&[
        "write_releasetype",
        "write_label",
        "write_work_composer",
        "write_musicbrainz_ids",
        "write_lyrics_tags",
        "write_download_source",
    ]);
    syncify_tauri_lib::services::tag_writer::apply_tag_policy(&mut meta, &prefs);
    let opts = syncify_tauri_lib::services::tag_writer::tag_write_options(&prefs);
    syncify_flac_writer::apply_flac_tags_with_options(&path, &meta, &opts).unwrap();

    assert!(vorbis_values(&path, "WORK").is_empty());
    assert!(vorbis_values(&path, "COMPOSER").is_empty());
    assert!(vorbis_values(&path, "RELEASETYPE").is_empty());
    assert!(vorbis_values(&path, "LABEL").is_empty());
    assert!(vorbis_values(&path, "LYRICS").is_empty());
    assert!(vorbis_values(&path, "UNSYNCEDLYRICS").is_empty());
    assert!(vorbis_values(&path, "SYNCIFY_AUDIO_SOURCE").is_empty());
    assert!(vorbis_values(&path, "SYNCIFY_LYRICS_SOURCE").is_empty());
    assert!(vorbis_values(&path, "MUSICBRAINZ_TRACKID").is_empty());
    // TITLE no lo gobierna ningún flag: sigue presente.
    assert_eq!(vorbis_values(&path, "TITLE"), vec!["T"]);
}

// ---------------------------------------------------------------------------
// Provenancia nueva (SYNCIFY_DOWNLOAD_DATE / SYNCIFY_QUALITY_SCORE): escrita
// solo con el flag activo.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provenance_keys_write_only_with_flag_on() {
    let tmp = tempfile::TempDir::new().unwrap();

    // Fábrica (flags off): el flujo puede poblar los campos y aun así nada.
    let path = tmp.path().join("off.flac");
    create_test_flac(&path, None);
    let mut meta = full_meta();
    let prefs = MetadataPreferences::default();
    syncify_tauri_lib::services::tag_writer::apply_tag_policy(&mut meta, &prefs);
    let opts = syncify_tauri_lib::services::tag_writer::tag_write_options(&prefs);
    syncify_flac_writer::apply_flac_tags_with_options(&path, &meta, &opts).unwrap();
    assert!(vorbis_values(&path, "SYNCIFY_DOWNLOAD_DATE").is_empty());
    assert!(vorbis_values(&path, "SYNCIFY_QUALITY_SCORE").is_empty());

    // Flags on: aparecen.
    let path = tmp.path().join("on.flac");
    create_test_flac(&path, None);
    let mut meta = full_meta();
    let mut prefs = MetadataPreferences::default();
    prefs.write_download_date = true;
    prefs.write_quality_score = true;
    syncify_tauri_lib::services::tag_writer::apply_tag_policy(&mut meta, &prefs);
    let opts = syncify_tauri_lib::services::tag_writer::tag_write_options(&prefs);
    syncify_flac_writer::apply_flac_tags_with_options(&path, &meta, &opts).unwrap();
    assert_eq!(
        vorbis_values(&path, "SYNCIFY_DOWNLOAD_DATE"),
        vec!["2026-11-14T00:00:00Z"]
    );
    assert_eq!(vorbis_values(&path, "SYNCIFY_QUALITY_SCORE"), vec!["hires"]);
}

// ---------------------------------------------------------------------------
// preserve_custom_tags: fábrica true conserva comentarios ajenos; false los
// retira.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn preserve_custom_tags_governs_unmanaged_comments() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("c.flac");
    create_test_flac(&path, None);

    // Un comentario "custom" ajeno al escritor queda plantado en el fichero.
    let mut tag = metaflac::Tag::read_from_path(&path).unwrap();
    tag.vorbis_comments_mut()
        .set("MY_CUSTOM_TAG", vec!["keep-me".to_string()]);
    tag.write_to_path(&path).unwrap();

    let prefs = MetadataPreferences::default();
    let meta = full_meta();

    // Fábrica (preserve=true): sobrevive.
    let opts = syncify_tauri_lib::services::tag_writer::tag_write_options(&prefs);
    syncify_flac_writer::apply_flac_tags_with_options(&path, &meta, &opts).unwrap();
    assert_eq!(vorbis_values(&path, "MY_CUSTOM_TAG"), vec!["keep-me"]);

    // Flag off: se retira; las claves gestionadas siguen.
    let mut prefs = prefs.clone();
    prefs.preserve_custom_tags = false;
    let opts = syncify_tauri_lib::services::tag_writer::tag_write_options(&prefs);
    syncify_flac_writer::apply_flac_tags_with_options(&path, &meta, &opts).unwrap();
    assert!(vorbis_values(&path, "MY_CUSTOM_TAG").is_empty());
    assert_eq!(vorbis_values(&path, "TITLE"), vec!["T"]);
}

// ---------------------------------------------------------------------------
// include_appearances: el artista del track no primario del álbum se registra
// en album_artists solo con el flag activo.
// ---------------------------------------------------------------------------

async fn create_account(pool: &sqlx::SqlitePool, service: &str, name: &str) -> (i64, i64) {
    let service_id: i64 = match sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    {
        Some(id) => id,
        None => sqlx::query_scalar("INSERT INTO services (name) VALUES (?) RETURNING id")
            .bind(service)
            .fetch_one(pool)
            .await
            .unwrap(),
    };
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, ?, 1) RETURNING id",
    )
    .bind(service_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();
    (service_id, account_id)
}

async fn album_artists_for(pool: &sqlx::SqlitePool, track_id: i64) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT ar.name, aa.is_primary FROM album_artists aa
         JOIN albums al ON al.id = aa.album_id
         JOIN artists ar ON ar.id = aa.artist_id
         WHERE al.id = (SELECT album_id FROM tracks WHERE id = ?)
         ORDER BY aa.is_primary DESC, ar.name",
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn appearance_input(service_id: i64, account_id: i64, include_appearances: bool) -> SyncTrackInput {
    SyncTrackInput {
        origin_meta: syncify_tauri_lib::services::enrichment::OriginTrackMetadata {
            title: Some("Cancion".to_string()),
            artist: Some("Invitado".to_string()),
            album: Some("Album Del Titular".to_string()),
            album_artist: Some("Titular".to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: format!("tidal_{}", include_appearances as i32),
        service_name: "tidal".to_string(),
        service_id,
        account_id,
        include_appearances,
        ..Default::default()
    }
}

#[tokio::test]
async fn include_appearances_false_keeps_historical_primary_only() {
    let pool = setup_db().await;
    let (service_id, account_id) = create_account(&pool, "tidal", "Acc").await;
    let engine = EnrichmentEngine::new();

    let res = engine
        .enrich_and_persist_sync_track(&pool, appearance_input(service_id, account_id, false))
        .await
        .expect("persist");
    let rows = album_artists_for(&pool, res.track_id).await;
    assert_eq!(
        rows,
        vec![("Titular".to_string(), 1)],
        "fábrica: solo el artista primario, como hasta ahora"
    );
}

#[tokio::test]
async fn include_appearances_true_registers_the_appearance() {
    let pool = setup_db().await;
    let (service_id, account_id) = create_account(&pool, "tidal", "Acc").await;
    let engine = EnrichmentEngine::new();

    let res = engine
        .enrich_and_persist_sync_track(&pool, appearance_input(service_id, account_id, true))
        .await
        .expect("persist");
    let rows = album_artists_for(&pool, res.track_id).await;
    assert!(
        rows.contains(&("Titular".to_string(), 1)),
        "el primario sigue registrado: {:?}",
        rows
    );
    assert!(
        rows.contains(&("Invitado".to_string(), 0)),
        "la aparición queda registrada como no primaria: {:?}",
        rows
    );
}

// ---------------------------------------------------------------------------
// overwrite_on_reimport + multi_value_separator: reimportado sobre fichero ya
// registrado.
// ---------------------------------------------------------------------------

async fn track_row(pool: &sqlx::SqlitePool, file_path: &str) -> (String, Option<String>) {
    sqlx::query_as(
        "SELECT t.title, t.genre FROM tracks t
         JOIN downloads d ON d.track_id = t.id WHERE d.file_path = ?",
    )
    .bind(file_path)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn reimport_factory_does_not_touch_registered_tracks() {
    let pool = setup_db().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let file = tmp.path().join("cancion.flac");
    create_test_flac(&file, Some("Rock|Jazz"));

    let report = syncify_tauri_lib::commands::library::sync_local_library_inner(
        &pool,
        tmp.path().to_str().unwrap(),
        false,
        false,
    )
    .await
    .unwrap();
    assert_eq!(report.imported, 1, "report1: {:?}", report);
    let (title, _genre) = track_row(&pool, file.to_str().unwrap()).await;
    assert_eq!(title, "Cancion Original");

    // Segunda corrida con fábrica: el fichero ya registrado no se toca y el
    // género con separador ajeno ('|') queda como llegó (histórico; el trigger
    // de TASK-142 solo limpia ';' y '/').
    let report2 = syncify_tauri_lib::commands::library::sync_local_library_inner(
        &pool,
        tmp.path().to_str().unwrap(),
        false,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        report2.duplicates_matched + report2.duplicates_skipped,
        1,
        "report2: {:?}",
        report2
    );
    let (title2, genre2) = track_row(&pool, file.to_str().unwrap()).await;
    assert_eq!(title2, "Cancion Original");
    assert_eq!(genre2.as_deref(), Some("Rock|Jazz"));
}

#[tokio::test]
async fn overwrite_and_separator_change_the_visible_result() {
    let pool = setup_db().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let file = tmp.path().join("cancion2.flac");
    create_test_flac(&file, Some("Rock|Jazz"));

    let report = syncify_tauri_lib::commands::library::sync_local_library_inner(
        &pool,
        tmp.path().to_str().unwrap(),
        false,
        false,
    )
    .await
    .unwrap();
    assert_eq!(report.imported, 1, "report1: {:?}", report);

    // El usuario activa la sobrescritura y su separador propio ('|'), y
    // reetiqueta el fichero fuera de la app.
    sqlx::query("UPDATE metadata_preferences SET overwrite_on_reimport = 1, multi_value_separator = '|' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    create_test_flac(&file, Some("Ambient|Downtempo"));
    let path = file.clone();
    let mut tag = metaflac::Tag::read_from_path(&path).unwrap();
    tag.vorbis_comments_mut()
        .set("TITLE", vec!["Cancion Reetiquetada".to_string()]);
    tag.write_to_path(&path).unwrap();

    let report2 = syncify_tauri_lib::commands::library::sync_local_library_inner(
        &pool,
        tmp.path().to_str().unwrap(),
        false,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        report2.duplicates_matched + report2.duplicates_skipped,
        1,
        "report2: {:?}",
        report2
    );

    let (title, genre) = track_row(&pool, file.to_str().unwrap()).await;
    assert_eq!(title, "Cancion Reetiquetada", "la sobrescritura refresca");
    assert_eq!(
        genre.as_deref(),
        Some("Ambient"),
        "el separador configurado extrae el primario"
    );
}
