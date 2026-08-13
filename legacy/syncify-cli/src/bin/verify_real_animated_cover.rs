//! Real end-to-end animated cover production downloader verification

use anyhow::Result;
use std::path::Path;
use syncify_cli::download::resolve_and_download_animated_cover;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("=======================================================");
    println!(" REAL PRODUCTION PIPELINE ANIMATED COVER VERIFICATION ");
    println!("=======================================================");

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let target_dir = Path::new("downloads_real_production_test/The Warning/[2024] Keep Me Fed");
    let _ = tokio::fs::create_dir_all(&target_dir).await;

    println!("[1/2] Invoking resolve_and_download_animated_cover for 'The Warning' - 'Keep Me Fed'...");
    let status = resolve_and_download_animated_cover(&client, "The Warning", "Keep Me Fed", target_dir).await;

    println!("Result status: {:?}", status);

    let webp_path = target_dir.join("cover.webp");
    if webp_path.exists() {
        let metadata = std::fs::metadata(&webp_path)?;
        let size = metadata.len();
        println!("\n[2/2] SUCCESS: cover.webp produced by production pipeline!");
        println!("  File Path: {}", webp_path.display());
        println!("  File Size: {} bytes ({} KB)", size, size / 1024);

        let data = std::fs::read(&webp_path)?;
        if data.len() >= 30 {
            println!("  RIFF Header: {:?}", &data[0..4]);
            println!("  WEBP Magic: {:?}", &data[8..12]);
            println!("  FourCC 1: {:?}", std::str::from_utf8(&data[12..16]).unwrap_or(""));
            let flags = data[20];
            println!("  VP8X Flags byte: 0x{:02x} (Animation bit: {})", flags, (flags & 0x02) != 0);
        }
    } else {
        println!("\n[2/2] FAILED: cover.webp was not created: {:?}", status);
    }

    println!("\n=======================================================");
    Ok(())
}
