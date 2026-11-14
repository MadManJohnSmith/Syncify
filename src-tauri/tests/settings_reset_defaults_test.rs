//! SB00 (#196): contrato del reset completo de ajustes.
//!
//! El reset debe dejar TODAS las superficies tipadas y la KV de ajustes en el
//! estado exacto de una BD recién migrada, usando la única fuente de defaults
//! (`settings_defaults`). Los errores se propagan (nada de escrituras
//! silenciosas) y las filas fuera de catálogo sobreviven.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use syncify_tauri_lib::settings_defaults::reset_all_settings;

async fn migrated_pool() -> SqlitePool {
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

// ---------------------------------------------------------------------------
// Snapshots canónicos por superficie: structs explícitos (sin timestamps)
// que documentan qué cubre el reset. fresh == post_reset es el contrato.
// ---------------------------------------------------------------------------

macro_rules! snapshot_struct {
    ($name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[derive(Debug, PartialEq, sqlx::FromRow)]
        struct $name { $(pub $field: $ty),+ }
    };
}

snapshot_struct!(SyncSnap {
    auto_sync_enabled: bool,
    sync_interval_value: i64,
    sync_interval_unit: String,
    sync_on_startup: bool,
    background_download: bool,
    max_concurrent_downloads: i64,
    rate_limit_delay_ms: i64,
    pause_on_metered: bool,
    pause_on_low_battery: bool,
});
snapshot_struct!(ServiceSyncSnap {
    service_name: String, sync_favorites: bool, sync_playlists: bool, sync_albums: bool,
    incremental_sync: bool, sync_favorite_artists: bool, sync_purchases: bool,
    sync_library_history: bool, sync_include_appearances: bool, last_synced: Option<String>,
});
snapshot_struct!(QualitySnap {
    service_name: String,
    max_quality: String,
    preferred_format: String,
    fallback_quality: String,
    fallback_format: String,
});
snapshot_struct!(FolderSnap {
    base_folder: String, folder_template: String, file_template: String,
    artist_separator: String, replace_spaces_with: Option<String>, max_path_length: i64,
    fallback_action: String,
});
snapshot_struct!(DuplicateSnap {
    enable_detection: bool,
    prefer_higher_quality: bool,
    prefer_lossless: bool,
    replace_same_quality_different_source: bool,
    quality_threshold_kbps: i64,
    delete_duplicates_immediately: bool,
    move_to_trash: bool,
});
snapshot_struct!(AudioSnap {
    replay_gain_mode: String,
    target_loudness_lufs: f64,
    transcode_enabled: bool,
    transcode_format: String,
    transcode_bitrate: i64,
    keep_original_after_transcode: bool,
    embed_lyrics: bool,
    embed_artwork: bool,
    artwork_max_size: i64,
});
snapshot_struct!(LyricsConfigSnap {
    min_sync_level: String,
    preferred_language: String,
    storage_format: String,
    auto_fetch_on_import: bool,
    retry_failed: bool,
    retry_frequency: String,
});
snapshot_struct!(AdvancedSnap {
    log_level: String, log_to_file: bool, log_file_max_size_mb: i64,
    log_file_retention_days: i64, max_concurrent_downloads: i64, max_concurrent_imports: i64,
    worker_timeout_seconds: i64, cache_enabled: bool, cache_max_size_mb: i64,
    cache_ttl_hours: i64, fuzzy_match_threshold: f64, use_acoustic_fingerprinting: bool,
    prefer_exact_matches: bool, request_timeout_seconds: i64, max_retries: i64,
    retry_delay_seconds: i64, use_proxy: bool, debug_mode: bool, verbose_api_logging: bool,
    proxy_url: Option<String>,
});
snapshot_struct!(MetadataSnap {
    enable_musicbrainz: bool,
    enable_lastfm: bool,
    enable_acoustid: bool,
    overwrite_on_reimport: bool,
    preserve_custom_tags: bool,
    multi_value_separator: String,
    write_releasetype: bool,
    write_label: bool,
    write_work_composer: bool,
    write_musicbrainz_ids: bool,
    write_download_source: bool,
    write_download_date: bool,
    write_only_available_on: bool,
    write_not_available_streaming: bool,
    write_quality_score: bool,
    write_lyrics_tags: bool,
    weight_album: i64,
    weight_isrc: i64,
    weight_mb_id: i64,
    weight_cover: i64,
    weight_year: i64,
    weight_genre: i64,
});
snapshot_struct!(ServicePrefSnap {
    service_name: String,
    priority: i64,
    auto_import_enabled: bool,
});
snapshot_struct!(LyricsProviderSnap {
    provider_id: String,
    provider_name: String,
    enabled: bool,
    priority: i64,
    sync_level: String,
});

#[derive(Debug, PartialEq)]
struct SettingsSnapshot {
    sync: Vec<SyncSnap>,
    service_sync: Vec<ServiceSyncSnap>,
    quality: Vec<QualitySnap>,
    folder: Vec<FolderSnap>,
    duplicate: Vec<DuplicateSnap>,
    audio: Vec<AudioSnap>,
    lyrics_config: Vec<LyricsConfigSnap>,
    advanced: Vec<AdvancedSnap>,
    metadata: Vec<MetadataSnap>,
    service_prefs: Vec<ServicePrefSnap>,
    lyrics_providers: Vec<LyricsProviderSnap>,
    kv_keys: Vec<String>,
}

async fn snapshot(db: &SqlitePool) -> SettingsSnapshot {
    macro_rules! all {
        ($ty:ty, $sql:expr) => {
            sqlx::query_as::<_, $ty>($sql).fetch_all(db).await.unwrap()
        };
    }
    SettingsSnapshot {
        sync: all!(
            SyncSnap,
            "SELECT auto_sync_enabled, sync_interval_value, sync_interval_unit, sync_on_startup,
             background_download, max_concurrent_downloads, rate_limit_delay_ms,
             pause_on_metered, pause_on_low_battery FROM sync_settings ORDER BY id"
        ),
        service_sync: all!(
            ServiceSyncSnap,
            "SELECT service_name, sync_favorites, sync_playlists, sync_albums, incremental_sync,
             sync_favorite_artists, sync_purchases, sync_library_history,
             sync_include_appearances, last_synced FROM service_sync_settings
             ORDER BY service_name"
        ),
        quality: all!(
            QualitySnap,
            "SELECT service_name, max_quality, preferred_format, fallback_quality,
             fallback_format FROM quality_preferences ORDER BY service_name"
        ),
        folder: all!(
            FolderSnap,
            "SELECT base_folder, folder_template, file_template, artist_separator,
             replace_spaces_with, max_path_length, fallback_action FROM folder_settings
             ORDER BY id"
        ),
        duplicate: all!(
            DuplicateSnap,
            "SELECT enable_detection, prefer_higher_quality, prefer_lossless,
             replace_same_quality_different_source, quality_threshold_kbps,
             delete_duplicates_immediately, move_to_trash FROM duplicate_settings ORDER BY id"
        ),
        audio: all!(
            AudioSnap,
            "SELECT replay_gain_mode, target_loudness_lufs, transcode_enabled, transcode_format,
             transcode_bitrate, keep_original_after_transcode, embed_lyrics, embed_artwork,
             artwork_max_size FROM audio_processing_settings ORDER BY id"
        ),
        lyrics_config: all!(
            LyricsConfigSnap,
            "SELECT min_sync_level, preferred_language, storage_format, auto_fetch_on_import,
             retry_failed, retry_frequency FROM lyrics_config ORDER BY id"
        ),
        advanced: all!(
            AdvancedSnap,
            "SELECT log_level, log_to_file, log_file_max_size_mb, log_file_retention_days,
             max_concurrent_downloads, max_concurrent_imports, worker_timeout_seconds,
             cache_enabled, cache_max_size_mb, cache_ttl_hours, fuzzy_match_threshold,
             use_acoustic_fingerprinting, prefer_exact_matches, request_timeout_seconds,
             max_retries, retry_delay_seconds, use_proxy, debug_mode, verbose_api_logging,
             proxy_url FROM advanced_settings ORDER BY id"
        ),
        metadata: all!(
            MetadataSnap,
            "SELECT enable_musicbrainz, enable_lastfm, enable_acoustid, overwrite_on_reimport,
             preserve_custom_tags, multi_value_separator, write_releasetype, write_label,
             write_work_composer, write_musicbrainz_ids, write_download_source,
             write_download_date, write_only_available_on, write_not_available_streaming,
             write_quality_score, write_lyrics_tags, weight_album, weight_isrc, weight_mb_id,
             weight_cover, weight_year, weight_genre FROM metadata_preferences ORDER BY id"
        ),
        service_prefs: all!(
            ServicePrefSnap,
            "SELECT service_name, priority, auto_import_enabled FROM service_preferences
             ORDER BY service_name"
        ),
        lyrics_providers: all!(
            LyricsProviderSnap,
            "SELECT provider_id, provider_name, enabled, priority, sync_level
             FROM lyrics_provider_settings ORDER BY provider_id"
        ),
        kv_keys: sqlx::query(
            "SELECT key FROM settings
             WHERE key NOT LIKE 'bpm_postprocess_queue_%'
               AND key != 'credential_migration_v1_complete'
             ORDER BY key",
        )
        .fetch_all(db)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get::<String, _>("key"))
        .collect(),
    }
}

/// Mutación agresiva: todo valor de ajuste sale de fábrica en todas las
/// superficies, incluida la KV.
async fn scramble_everything(db: &SqlitePool) {
    sqlx::query(
        "UPDATE sync_settings SET auto_sync_enabled = TRUE, sync_interval_value = 5,
         sync_interval_unit = 'days', sync_on_startup = TRUE, background_download = FALSE,
         max_concurrent_downloads = 9, rate_limit_delay_ms = 42, pause_on_metered = FALSE,
         pause_on_low_battery = FALSE",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE service_sync_settings SET sync_favorites = FALSE, sync_playlists = FALSE,
         sync_albums = FALSE, incremental_sync = FALSE, sync_favorite_artists = FALSE,
         sync_purchases = TRUE, sync_library_history = TRUE, sync_include_appearances = TRUE,
         last_synced = '2026-01-01T00:00:00Z'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE quality_preferences SET max_quality = 'medium', preferred_format = 'mp3',
         fallback_quality = 'low', fallback_format = 'ogg'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE folder_settings SET base_folder = '/tmp/x', folder_template = 'X/{Y}',
         file_template = 'Z', artist_separator = ' | ', replace_spaces_with = '_',
         max_path_length = 99, fallback_action = 'always_ask'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE duplicate_settings SET enable_detection = 0, prefer_higher_quality = 0,
         prefer_lossless = 0, replace_same_quality_different_source = 1,
         quality_threshold_kbps = 320, delete_duplicates_immediately = 1, move_to_trash = 0",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE audio_processing_settings SET replay_gain_mode = 'album',
         target_loudness_lufs = -18.0, transcode_enabled = TRUE, transcode_format = 'ogg',
         transcode_bitrate = 96, keep_original_after_transcode = FALSE, embed_lyrics = FALSE,
         embed_artwork = FALSE, artwork_max_size = 500",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE lyrics_config SET min_sync_level = 'none', preferred_language = 'es',
         storage_format = 'txt', auto_fetch_on_import = FALSE, retry_failed = FALSE,
         retry_frequency = 'daily'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE advanced_settings SET log_level = 'trace', log_to_file = FALSE,
         log_file_max_size_mb = 1, log_file_retention_days = 1, max_concurrent_downloads = 7,
         max_concurrent_imports = 9, worker_timeout_seconds = 1, cache_enabled = FALSE,
         cache_max_size_mb = 1, cache_ttl_hours = 1, fuzzy_match_threshold = 0.1,
         use_acoustic_fingerprinting = FALSE, prefer_exact_matches = FALSE,
         request_timeout_seconds = 1, max_retries = 0, retry_delay_seconds = 0,
         use_proxy = TRUE, proxy_url = 'http://proxy:1', debug_mode = TRUE,
         verbose_api_logging = TRUE",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE metadata_preferences SET overwrite_on_reimport = TRUE,
         preserve_custom_tags = FALSE, multi_value_separator = ',', write_releasetype = FALSE,
         write_label = FALSE, write_work_composer = TRUE, write_musicbrainz_ids = FALSE,
         write_download_source = TRUE, write_download_date = TRUE,
         write_only_available_on = TRUE, write_not_available_streaming = TRUE,
         write_quality_score = TRUE, write_lyrics_tags = TRUE, weight_album = 9,
         weight_isrc = 9, weight_mb_id = 9, weight_cover = 9, weight_year = 9,
         weight_genre = 9, enable_musicbrainz = FALSE, enable_lastfm = TRUE,
         enable_acoustid = TRUE",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query("UPDATE service_preferences SET priority = 99, auto_import_enabled = TRUE")
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE lyrics_provider_settings SET enabled = FALSE, priority = 999,
         sync_level = 'none', provider_name = 'renamed'",
    )
    .execute(db)
    .await
    .unwrap();

    for key in [
        "dl_concurrent_downloads",
        "dl_download_path",
        "download_dir",
        "download_path",
        "dl_temp_dir",
        "temp_dir",
        "preferred_quality",
        "global_max_quality",
        "lastfm_api_key",
        "close_to_tray",
        "start_on_boot",
    ] {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, 'scrambled')")
            .bind(key)
            .execute(db)
            .await
            .unwrap();
    }
    // Clave de infraestructura: el reset NO debe tocarla.
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('credential_migration_v1_complete', '1')",
    )
    .execute(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn reset_restores_fresh_seed_state_in_every_surface() {
    let pool = migrated_pool().await;
    let fresh = snapshot(&pool).await;

    scramble_everything(&pool).await;

    // Sanity: la mutación tiene que haber cambiado algo, si no el test miente.
    let scrambled = snapshot(&pool).await;
    assert_ne!(fresh, scrambled, "scramble_everything must change state");

    reset_all_settings(&pool)
        .await
        .expect("reset_all_settings must succeed");

    let post_reset = snapshot(&pool).await;
    assert_eq!(
        fresh, post_reset,
        "reset must restore the fresh-migrated state"
    );
}

#[tokio::test]
async fn reset_recreates_missing_singleton_rows() {
    let pool = migrated_pool().await;
    let fresh = snapshot(&pool).await;

    sqlx::query("DELETE FROM sync_settings")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM metadata_preferences")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM folder_settings")
        .execute(&pool)
        .await
        .unwrap();

    reset_all_settings(&pool).await.expect("reset must succeed");

    let post_reset = snapshot(&pool).await;
    assert_eq!(
        fresh, post_reset,
        "reset must recreate and repopulate missing singleton rows"
    );
}

#[tokio::test]
async fn reset_preserves_rows_outside_the_catalog() {
    let pool = migrated_pool().await;

    sqlx::query(
        "INSERT INTO lyrics_provider_settings (provider_id, provider_name, enabled, priority, sync_level)
         VALUES ('genius_legacy', 'Genius (legacy)', FALSE, 99, 'none')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO service_preferences (service_name, priority, auto_import_enabled)
         VALUES ('applemusic_test', 77, TRUE)",
    )
    .execute(&pool)
    .await
    .unwrap();

    reset_all_settings(&pool).await.expect("reset must succeed");

    let legacy: (bool, i64, String) = sqlx::query_as(
        "SELECT enabled, priority, sync_level FROM lyrics_provider_settings
         WHERE provider_id = 'genius_legacy'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        legacy,
        (false, 99, "none".into()),
        "row outside the catalog must be untouched"
    );

    let extra: (i64, bool) = sqlx::query_as(
        "SELECT priority, auto_import_enabled FROM service_preferences
         WHERE service_name = 'applemusic_test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        extra,
        (77, true),
        "row outside the catalog must be untouched"
    );
}

#[tokio::test]
async fn reset_clears_settings_kv_but_not_infra_keys() {
    let pool = migrated_pool().await;
    scramble_everything(&pool).await;

    reset_all_settings(&pool).await.expect("reset must succeed");

    let remaining: Vec<String> = sqlx::query("SELECT key FROM settings ORDER BY key")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get::<String, _>("key"))
        .collect();
    assert!(
        remaining
            .iter()
            .all(|k| k.starts_with("bpm_postprocess_queue_")
                || k == "credential_migration_v1_complete"),
        "reset must leave only non-settings keys, got {remaining:?}"
    );
    for key in [
        "lastfm_api_key",
        "dl_download_path",
        "global_max_quality",
        "start_on_boot",
    ] {
        assert!(
            !remaining.contains(&key.to_string()),
            "settings key {key} must be cleared by reset"
        );
    }
}

#[tokio::test]
async fn module_defaults_match_the_fresh_migrated_seed() {
    // El contrato «defaults en un solo sitio»: los Default del módulo y el
    // estado de una BD recién migrada tienen que coincidir por superficie.
    let pool = migrated_pool().await;

    let sync: SyncSnap = sqlx::query_as(
        "SELECT auto_sync_enabled, sync_interval_value, sync_interval_unit, sync_on_startup,
         background_download, max_concurrent_downloads, rate_limit_delay_ms,
         pause_on_metered, pause_on_low_battery FROM sync_settings WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let d = syncify_tauri_lib::models::SyncSettings::default();
    assert_eq!(
        sync,
        SyncSnap {
            auto_sync_enabled: d.auto_sync_enabled,
            sync_interval_value: d.sync_interval_value,
            sync_interval_unit: d.sync_interval_unit.clone(),
            sync_on_startup: d.sync_on_startup,
            background_download: d.background_download,
            max_concurrent_downloads: d.max_concurrent_downloads,
            rate_limit_delay_ms: d.rate_limit_delay_ms,
            pause_on_metered: d.pause_on_metered,
            pause_on_low_battery: d.pause_on_low_battery,
        }
    );

    let folder: FolderSnap = sqlx::query_as(
        "SELECT base_folder, folder_template, file_template, artist_separator,
         replace_spaces_with, max_path_length, fallback_action FROM folder_settings WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let d = syncify_tauri_lib::models::FolderSettings::default();
    assert_eq!(
        folder,
        FolderSnap {
            base_folder: d.base_folder.clone(),
            folder_template: d.folder_template.clone(),
            file_template: d.file_template.clone(),
            artist_separator: d.artist_separator.clone(),
            replace_spaces_with: d.replace_spaces_with.clone(),
            max_path_length: d.max_path_length,
            fallback_action: d.fallback_action.clone(),
        }
    );

    let row_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM lyrics_provider_settings WHERE provider_id IN
         ('apple_music_ttml','lrclib_synced','netease_synced','spotify_color',
          'musixmatch_richsync','ultrastar','kugou','qqmusic','lyricsplus',
          'musixmatch_plain','lrclib_plain','tekstowo_plain')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row_count.0 as usize,
        syncify_tauri_lib::settings_defaults::LYRICS_PROVIDER_SEEDS.len(),
        "the module catalog must mirror the migrated provider catalog"
    );
}
