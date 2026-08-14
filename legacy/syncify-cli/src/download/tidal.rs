//! Tidal downloader — re-exported from `syncify-tidal-downloader`.

pub use syncify_tidal_downloader::*;

pub trait TidalGuiSessionExt {
    fn resolve_gui_session(&self) -> impl std::future::Future<Output = (Option<String>, TidalAuthResolution)> + Send;
}

impl TidalGuiSessionExt for TidalDownloader {
    async fn resolve_gui_session(&self) -> (Option<String>, TidalAuthResolution) {
        let client = reqwest::Client::new();
        if let Ok(db_path) = crate::crypto::resolve_syncify_db_path() {
            let conn_str = format!("sqlite:{}?mode=ro", db_path.to_string_lossy());
            if let Ok(pool) = sqlx::sqlite::SqlitePoolOptions::new().connect(&conn_str).await {
                return crate::services::tidal::resolve_gui_credentials_from_pool(&pool, &client).await;
            }
        }
        (None, TidalAuthResolution::RequiresAuth)
    }
}
