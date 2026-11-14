//! Única fuente de valores de fábrica de ajustes (SB00, #196; ADR-0019
//! decisiones 3 y 5).
//!
//! Este módulo es la referencia viva de los defaults: los alimenta el reset
//! completo del backend, los fallbacks de lectura en Rust y cualquier seed
//! futuro. Las migraciones ya aplicadas conservan sus seeds históricos
//! (no se editan), pero el árbol final que producen es exactamente el que
//! este módulo declara — así lo exige el test de coherencia con el estado
//! recién migrado (`tests/settings_reset_defaults_test.rs`).
//!
//! Contrato: ningún ajuste nuevo entra sin fila previa en
//! `docs/auditoria/matriz-ajustes.md` y sin su default aquí.

use crate::db::DbPool;
use crate::models::{
    AdvancedSettings, AudioProcessingSettings, FolderSettings, LyricsConfig, MetadataPreferences,
    SyncSettings,
};
use sqlx::Sqlite;

// ---------------------------------------------------------------------------
// Catálogos con defaults por fila (tablas multi-fila)
// ---------------------------------------------------------------------------

/// Fila de fábrica de `service_preferences` (0007).
pub struct ServicePreferenceSeed {
    pub service_name: &'static str,
    pub priority: i64,
}

pub const SERVICE_PREFERENCE_SEEDS: &[ServicePreferenceSeed] = &[
    ServicePreferenceSeed {
        service_name: "spotify",
        priority: 1,
    },
    ServicePreferenceSeed {
        service_name: "qobuz",
        priority: 2,
    },
    ServicePreferenceSeed {
        service_name: "tidal",
        priority: 3,
    },
    ServicePreferenceSeed {
        service_name: "deezer",
        priority: 4,
    },
    ServicePreferenceSeed {
        service_name: "soundcloud",
        priority: 5,
    },
];

/// Fila de fábrica de `service_sync_settings` (0008 + reconstrucción 0092).
/// Todos los servicios comparten valores; `incremental_sync` es la columna
/// retirada (#197) que se normaliza a su DEFAULT 1 por coherencia con una BD
/// recién migrada.
pub const SERVICE_SYNC_SEEDS: &[&str] = &["spotify", "qobuz", "tidal", "deezer", "soundcloud"];

/// Fila de fábrica de `quality_preferences` (0009 + ajustes posteriores; el
/// valor vigente de soundcloud es el del árbol completo, no el del seed
/// original de 0009).
pub struct QualityPreferenceSeed {
    pub service_name: &'static str,
    pub max_quality: &'static str,
    pub preferred_format: &'static str,
    pub fallback_quality: &'static str,
    pub fallback_format: &'static str,
}

pub const QUALITY_PREFERENCE_SEEDS: &[QualityPreferenceSeed] = &[
    QualityPreferenceSeed {
        service_name: "spotify",
        max_quality: "high",
        preferred_format: "ogg",
        fallback_quality: "medium",
        fallback_format: "ogg",
    },
    QualityPreferenceSeed {
        service_name: "qobuz",
        max_quality: "hires",
        preferred_format: "flac",
        fallback_quality: "lossless",
        fallback_format: "flac",
    },
    QualityPreferenceSeed {
        service_name: "tidal",
        max_quality: "master",
        preferred_format: "flac",
        fallback_quality: "hifi",
        fallback_format: "flac",
    },
    QualityPreferenceSeed {
        service_name: "deezer",
        max_quality: "lossless",
        preferred_format: "flac",
        fallback_quality: "high",
        fallback_format: "mp3",
    },
    QualityPreferenceSeed {
        service_name: "soundcloud",
        max_quality: "lossy",
        preferred_format: "mp3",
        fallback_quality: "medium",
        fallback_format: "mp3",
    },
];

/// Fila de fábrica de `lyrics_provider_settings` (0013 + catálogo estable
/// 0100). Todas activas; el orden es el de fábrica del catálogo vigente.
pub struct LyricsProviderSeed {
    pub provider_id: &'static str,
    pub provider_name: &'static str,
    pub priority: i64,
    pub sync_level: &'static str,
}

pub const LYRICS_PROVIDER_SEEDS: &[LyricsProviderSeed] = &[
    LyricsProviderSeed {
        provider_id: "apple_music_ttml",
        provider_name: "Apple Music TTML",
        priority: 1,
        sync_level: "syllable",
    },
    LyricsProviderSeed {
        provider_id: "lrclib_synced",
        provider_name: "LRCLIB (synced)",
        priority: 2,
        sync_level: "line",
    },
    LyricsProviderSeed {
        provider_id: "netease_synced",
        provider_name: "NetEase (synced)",
        priority: 3,
        sync_level: "line",
    },
    LyricsProviderSeed {
        provider_id: "spotify_color",
        provider_name: "Spotify Color Lyrics",
        priority: 5,
        sync_level: "word",
    },
    LyricsProviderSeed {
        provider_id: "musixmatch_richsync",
        provider_name: "Musixmatch Richsync",
        priority: 6,
        sync_level: "word",
    },
    LyricsProviderSeed {
        provider_id: "ultrastar",
        provider_name: "UltraStar Karaoke",
        priority: 7,
        sync_level: "syllable",
    },
    LyricsProviderSeed {
        provider_id: "kugou",
        provider_name: "Kugou Karaoke",
        priority: 8,
        sync_level: "word",
    },
    LyricsProviderSeed {
        provider_id: "qqmusic",
        provider_name: "QQ Music",
        priority: 9,
        sync_level: "line",
    },
    LyricsProviderSeed {
        provider_id: "lyricsplus",
        provider_name: "LyricsPlus",
        priority: 10,
        sync_level: "line",
    },
    LyricsProviderSeed {
        provider_id: "musixmatch_plain",
        provider_name: "Musixmatch (plain)",
        priority: 11,
        sync_level: "none",
    },
    LyricsProviderSeed {
        provider_id: "lrclib_plain",
        provider_name: "LRCLIB (plain)",
        priority: 12,
        sync_level: "none",
    },
    LyricsProviderSeed {
        provider_id: "tekstowo_plain",
        provider_name: "Tekstowo (plain)",
        priority: 13,
        sync_level: "none",
    },
];

/// Claves de la KV `settings` que son ajustes de usuario y el reset devuelve
/// a fábrica. Fuera quedan: credenciales y banderas de infraestructura
/// (`credential_migration_v1_complete`, `spotify_*`), estado de tareas del
/// worker (`bpm_postprocess_queue_*`) y cualquier clave desconocida.
pub const SETTINGS_KV_KEYS: &[&str] = &[
    "dl_concurrent_downloads",
    "dl_download_path",
    "download_dir",
    "download_path",
    "dl_temp_dir",
    "temp_dir",
    "dl_retry_failed",
    "dl_create_artist_folder",
    "dl_create_album_folder",
    "dl_generate_lyrics_lrc",
    "dl_generate_cover_art",
    "dl_generate_animated_cover",
    "dl_generate_booklet",
    "dl_generate_artist_sidecars",
    "dl_allow_downgrade",
    "dl_analyze_bpm_on_download",
    "preferred_quality",
    "global_max_quality",
    "close_to_tray",
    "start_on_boot",
    "start_minimized",
    "notifications_enabled",
    "lastfm_api_key",
];

// ---------------------------------------------------------------------------
// Defaults de los singletons (impl Default = misma verdad que la BD nueva)
// ---------------------------------------------------------------------------

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            id: 1,
            auto_sync_enabled: false,
            sync_interval_value: 60,
            sync_interval_unit: "minutes".to_string(),
            sync_on_startup: false,
            background_download: true,
            max_concurrent_downloads: 3,
            rate_limit_delay_ms: 500,
            pause_on_metered: true,
            pause_on_low_battery: true,
        }
    }
}

impl Default for FolderSettings {
    fn default() -> Self {
        Self {
            id: 1,
            base_folder: String::new(),
            folder_template: "{AlbumArtist}/{Album}".to_string(),
            file_template: "{TrackNumber:pad2} - {Title}".to_string(),
            artist_separator: ", ".to_string(),
            replace_spaces_with: None,
            max_path_length: 255,
            fallback_action: "try_next".to_string(),
        }
    }
}

impl Default for AudioProcessingSettings {
    fn default() -> Self {
        Self {
            id: 1,
            replay_gain_mode: "off".to_string(),
            target_loudness_lufs: -14.0,
            transcode_enabled: false,
            transcode_format: "mp3".to_string(),
            transcode_bitrate: 320,
            keep_original_after_transcode: true,
            embed_lyrics: true,
            embed_artwork: true,
            artwork_max_size: 1200,
        }
    }
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            id: 1,
            min_sync_level: "line".to_string(),
            preferred_language: "en".to_string(),
            storage_format: "lrc".to_string(),
            auto_fetch_on_import: true,
            retry_failed: true,
            retry_frequency: "weekly".to_string(),
        }
    }
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            id: 1,
            log_level: "info".to_string(),
            log_to_file: true,
            log_file_max_size_mb: 50,
            log_file_retention_days: 30,
            max_concurrent_downloads: 3,
            max_concurrent_imports: 2,
            worker_timeout_seconds: 300,
            cache_enabled: true,
            cache_max_size_mb: 500,
            cache_ttl_hours: 168,
            fuzzy_match_threshold: 0.85,
            use_acoustic_fingerprinting: true,
            prefer_exact_matches: true,
            request_timeout_seconds: 30,
            max_retries: 3,
            retry_delay_seconds: 5,
            use_proxy: false,
            proxy_url: None,
            debug_mode: false,
            verbose_api_logging: false,
        }
    }
}

impl Default for MetadataPreferences {
    fn default() -> Self {
        Self {
            id: 1,
            enable_musicbrainz: true,
            enable_lastfm: false,
            enable_acoustid: false,
            overwrite_on_reimport: false,
            preserve_custom_tags: true,
            multi_value_separator: ";".to_string(),
            write_releasetype: true,
            write_label: true,
            write_work_composer: false,
            write_musicbrainz_ids: true,
            write_download_source: false,
            write_download_date: false,
            write_only_available_on: false,
            write_not_available_streaming: false,
            write_quality_score: false,
            write_lyrics_tags: false,
            weight_album: 1,
            weight_isrc: 1,
            weight_mb_id: 1,
            weight_cover: 1,
            weight_year: 1,
            weight_genre: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Reset completo (ADR-0019 decisión 5): una transacción, errores propagados
// ---------------------------------------------------------------------------

/// Resetea TODAS las superficies tipadas de ajustes y las claves de ajustes de
/// la KV `settings` a sus valores de fábrica. En una transacción: o resetea
/// todo o nada.
///
/// Para los singletons borra y reinserta con los valores explícitos de este
/// módulo (no confía en los DEFAULT de columna, congelados en migraciones). En
/// las tablas de catálogo restaura valores y pertenencia con upsert: las filas
/// que el catálogo vigente ya no conoce no se tocan (el histórico es de la
/// migración, no del reset). No borra columnas ni toca migraciones aplicadas.
pub async fn reset_all_settings(db: &DbPool) -> Result<(), String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Reset transaction error: {}", e))?;

    // -- Singletons: DELETE + INSERT con valores explícitos del módulo.
    let s = SyncSettings::default();
    sqlx::query("DELETE FROM sync_settings WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("sync_settings"))?;
    sqlx::query(
        "INSERT INTO sync_settings (id, auto_sync_enabled, sync_interval_value, sync_interval_unit,
         sync_on_startup, background_download, max_concurrent_downloads, rate_limit_delay_ms,
         pause_on_metered, pause_on_low_battery, updated_at)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(s.auto_sync_enabled)
    .bind(s.sync_interval_value)
    .bind(&s.sync_interval_unit)
    .bind(s.sync_on_startup)
    .bind(s.background_download)
    .bind(s.max_concurrent_downloads)
    .bind(s.rate_limit_delay_ms)
    .bind(s.pause_on_metered)
    .bind(s.pause_on_low_battery)
    .execute(&mut *tx)
    .await
    .map_err(err("sync_settings"))?;

    let f = FolderSettings::default();
    sqlx::query("DELETE FROM folder_settings WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("folder_settings"))?;
    sqlx::query(
        "INSERT INTO folder_settings (id, base_folder, folder_template, file_template,
         artist_separator, replace_spaces_with, max_path_length, fallback_action, updated_at)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&f.base_folder)
    .bind(&f.folder_template)
    .bind(&f.file_template)
    .bind(&f.artist_separator)
    .bind(&f.replace_spaces_with)
    .bind(f.max_path_length)
    .bind(&f.fallback_action)
    .execute(&mut *tx)
    .await
    .map_err(err("folder_settings"))?;

    let a = AudioProcessingSettings::default();
    sqlx::query("DELETE FROM audio_processing_settings WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("audio_processing_settings"))?;
    sqlx::query(
        "INSERT INTO audio_processing_settings (id, replay_gain_mode, target_loudness_lufs,
         transcode_enabled, transcode_format, transcode_bitrate, keep_original_after_transcode,
         embed_lyrics, embed_artwork, artwork_max_size, updated_at)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&a.replay_gain_mode)
    .bind(a.target_loudness_lufs)
    .bind(a.transcode_enabled)
    .bind(&a.transcode_format)
    .bind(a.transcode_bitrate)
    .bind(a.keep_original_after_transcode)
    .bind(a.embed_lyrics)
    .bind(a.embed_artwork)
    .bind(a.artwork_max_size)
    .execute(&mut *tx)
    .await
    .map_err(err("audio_processing_settings"))?;

    let l = LyricsConfig::default();
    sqlx::query("DELETE FROM lyrics_config WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("lyrics_config"))?;
    sqlx::query(
        "INSERT INTO lyrics_config (id, min_sync_level, preferred_language, storage_format,
         auto_fetch_on_import, retry_failed, retry_frequency, updated_at)
         VALUES (1, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&l.min_sync_level)
    .bind(&l.preferred_language)
    .bind(&l.storage_format)
    .bind(l.auto_fetch_on_import)
    .bind(l.retry_failed)
    .bind(&l.retry_frequency)
    .execute(&mut *tx)
    .await
    .map_err(err("lyrics_config"))?;

    let adv = AdvancedSettings::default();
    sqlx::query("DELETE FROM advanced_settings WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("advanced_settings"))?;
    insert_advanced_defaults(&mut tx, &adv)
        .await
        .map_err(err("advanced_settings"))?;

    let m = MetadataPreferences::default();
    sqlx::query("DELETE FROM metadata_preferences WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("metadata_preferences"))?;
    sqlx::query(
        "INSERT INTO metadata_preferences (id, enable_musicbrainz, enable_lastfm, enable_acoustid,
         overwrite_on_reimport, preserve_custom_tags, multi_value_separator, write_releasetype,
         write_label, write_work_composer, write_musicbrainz_ids, write_download_source,
         write_download_date, write_only_available_on, write_not_available_streaming,
         write_quality_score, write_lyrics_tags, weight_album, weight_isrc, weight_mb_id,
         weight_cover, weight_year, weight_genre)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(m.enable_musicbrainz)
    .bind(m.enable_lastfm)
    .bind(m.enable_acoustid)
    .bind(m.overwrite_on_reimport)
    .bind(m.preserve_custom_tags)
    .bind(&m.multi_value_separator)
    .bind(m.write_releasetype)
    .bind(m.write_label)
    .bind(m.write_work_composer)
    .bind(m.write_musicbrainz_ids)
    .bind(m.write_download_source)
    .bind(m.write_download_date)
    .bind(m.write_only_available_on)
    .bind(m.write_not_available_streaming)
    .bind(m.write_quality_score)
    .bind(m.write_lyrics_tags)
    .bind(m.weight_album)
    .bind(m.weight_isrc)
    .bind(m.weight_mb_id)
    .bind(m.weight_cover)
    .bind(m.weight_year)
    .bind(m.weight_genre)
    .execute(&mut *tx)
    .await
    .map_err(err("metadata_preferences"))?;

    // -- duplicate_settings: superficie oculta (#197) pero tipada; el reset la
    //    restaura a fábrica aunque hoy nadie la lea.
    sqlx::query("DELETE FROM duplicate_settings WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("duplicate_settings"))?;
    sqlx::query("INSERT INTO duplicate_settings (id) VALUES (1)")
        .execute(&mut *tx)
        .await
        .map_err(err("duplicate_settings"))?;

    // -- Catálogos: upsert de pertenencia + valores de fábrica, sin borrar
    //    filas que el catálogo ya no conoce.
    for seed in SERVICE_PREFERENCE_SEEDS {
        sqlx::query(
            "INSERT INTO service_preferences (service_name, priority, auto_import_enabled)
             VALUES (?, ?, FALSE)
             ON CONFLICT(service_name) DO UPDATE SET
               priority = excluded.priority,
               auto_import_enabled = excluded.auto_import_enabled,
               updated_at = datetime('now')",
        )
        .bind(seed.service_name)
        .bind(seed.priority)
        .execute(&mut *tx)
        .await
        .map_err(err("service_preferences"))?;
    }

    for service in SERVICE_SYNC_SEEDS {
        sqlx::query(
            "INSERT INTO service_sync_settings (service_name, sync_favorites, sync_playlists,
             sync_albums, incremental_sync, sync_favorite_artists, sync_purchases,
             sync_library_history, sync_include_appearances, last_synced)
             VALUES (?, TRUE, TRUE, TRUE, TRUE, TRUE, FALSE, FALSE, FALSE, NULL)
             ON CONFLICT(service_name) DO UPDATE SET
               sync_favorites = TRUE, sync_playlists = TRUE, sync_albums = TRUE,
               incremental_sync = TRUE, sync_favorite_artists = TRUE, sync_purchases = FALSE,
               sync_library_history = FALSE, sync_include_appearances = FALSE,
               last_synced = NULL, updated_at = datetime('now')",
        )
        .bind(service)
        .execute(&mut *tx)
        .await
        .map_err(err("service_sync_settings"))?;
    }

    for seed in QUALITY_PREFERENCE_SEEDS {
        sqlx::query(
            "INSERT INTO quality_preferences (service_name, max_quality, preferred_format,
             fallback_quality, fallback_format)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(service_name) DO UPDATE SET
               max_quality = excluded.max_quality,
               preferred_format = excluded.preferred_format,
               fallback_quality = excluded.fallback_quality,
               fallback_format = excluded.fallback_format,
               updated_at = datetime('now')",
        )
        .bind(seed.service_name)
        .bind(seed.max_quality)
        .bind(seed.preferred_format)
        .bind(seed.fallback_quality)
        .bind(seed.fallback_format)
        .execute(&mut *tx)
        .await
        .map_err(err("quality_preferences"))?;
    }

    for seed in LYRICS_PROVIDER_SEEDS {
        sqlx::query(
            "INSERT INTO lyrics_provider_settings (provider_id, provider_name, enabled, priority, sync_level)
             VALUES (?, ?, TRUE, ?, ?)
             ON CONFLICT(provider_id) DO UPDATE SET
               provider_name = excluded.provider_name,
               enabled = excluded.enabled,
               priority = excluded.priority,
               sync_level = excluded.sync_level,
               updated_at = datetime('now')",
        )
        .bind(seed.provider_id)
        .bind(seed.provider_name)
        .bind(seed.priority)
        .bind(seed.sync_level)
        .execute(&mut *tx).await.map_err(err("lyrics_provider_settings"))?;
    }

    // -- KV de ajustes: solo las claves enumeradas como ajustes de usuario.
    for key in SETTINGS_KV_KEYS {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&mut *tx)
            .await
            .map_err(err("settings (KV)"))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Reset commit error: {}", e))
}

/// Resetea solo la superficie `advanced_settings` a fábrica (brazo
/// `"advanced"` del comando de reset).
pub async fn reset_advanced_settings(db: &DbPool) -> Result<(), String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Reset transaction error: {}", e))?;
    sqlx::query("DELETE FROM advanced_settings WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(err("advanced_settings"))?;
    insert_advanced_defaults(&mut tx, &AdvancedSettings::default())
        .await
        .map_err(err("advanced_settings"))?;
    tx.commit()
        .await
        .map_err(|e| format!("Reset commit error: {}", e))
}

async fn insert_advanced_defaults(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    adv: &AdvancedSettings,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO advanced_settings (id, log_level, log_to_file, log_file_max_size_mb,
         log_file_retention_days, max_concurrent_downloads, max_concurrent_imports,
         worker_timeout_seconds, cache_enabled, cache_max_size_mb, cache_ttl_hours,
         fuzzy_match_threshold, use_acoustic_fingerprinting, prefer_exact_matches,
         request_timeout_seconds, max_retries, retry_delay_seconds, use_proxy, proxy_url,
         debug_mode, verbose_api_logging, updated_at)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&adv.log_level)
    .bind(adv.log_to_file)
    .bind(adv.log_file_max_size_mb)
    .bind(adv.log_file_retention_days)
    .bind(adv.max_concurrent_downloads)
    .bind(adv.max_concurrent_imports)
    .bind(adv.worker_timeout_seconds)
    .bind(adv.cache_enabled)
    .bind(adv.cache_max_size_mb)
    .bind(adv.cache_ttl_hours)
    .bind(adv.fuzzy_match_threshold)
    .bind(adv.use_acoustic_fingerprinting)
    .bind(adv.prefer_exact_matches)
    .bind(adv.request_timeout_seconds)
    .bind(adv.max_retries)
    .bind(adv.retry_delay_seconds)
    .bind(adv.use_proxy)
    .bind(&adv.proxy_url)
    .bind(adv.debug_mode)
    .bind(adv.verbose_api_logging)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

fn err(table: &'static str) -> impl Fn(sqlx::Error) -> String {
    move |e| format!("Reset error in {}: {}", table, e)
}
