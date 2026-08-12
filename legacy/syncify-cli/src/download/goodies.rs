//! Qobuz Digital Booklet (Goodies) PDF Downloader (CLI Standalone)

use anyhow::{anyhow, Result};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tracing::info;

pub async fn download_goodies_booklet(
    client: &Client,
    booklet_url: &str,
    target_album_dir: &Path,
) -> Result<Option<PathBuf>> {
    if booklet_url.trim().is_empty() {
        return Ok(None);
    }
    tokio::fs::create_dir_all(target_album_dir).await?;
    let target_pdf = target_album_dir.join("booklet.pdf");
    if target_pdf.exists() {
        info!("[Goodies] Booklet already exists at {}", target_pdf.display());
        return Ok(Some(target_pdf));
    }
    let res = client.get(booklet_url).send().await?;
    if res.status().is_success() {
        let bytes = res.bytes().await?;
        tokio::fs::write(&target_pdf, &bytes).await?;
        info!("[Goodies] Downloaded digital booklet to {}", target_pdf.display());
        return Ok(Some(target_pdf));
    }
    Err(anyhow!("Failed to download booklet: HTTP {}", res.status()))
}
