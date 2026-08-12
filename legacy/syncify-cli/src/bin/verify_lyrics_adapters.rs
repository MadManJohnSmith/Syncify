// Controlled network validation of Lyrics Provider Adapters

use std::time::Instant;
use syncify_cli::download::lyrics::LyricsClient;

#[tokio::main]
async fn main() {
    println!("============================================================");
    println!("     CONTROLLED LYRICS PROVIDER ADAPTERS AUDIT REPORT       ");
    println!("============================================================");

    let client = LyricsClient::new();

    // 1. NetEase Cloud Music
    println!("\n--- [1/3] Testing NetEase Cloud Music Adapter ---");
    let start_netease = Instant::now();
    let netease_res = client.resolve_netease("Gloria Gaynor", "I Will Survive", 198.0).await;
    let netease_dur = start_netease.elapsed();

    println!("  Provider:              {}", netease_res.provider);
    println!("  Strategy:              {}", netease_res.strategy);
    println!("  Status:                {:?}", netease_res.status);
    println!("  Sync Type:             {:?}", netease_res.sync_type);
    println!("  Line Count:            {}", netease_res.lines.len());
    println!("  Fallback Applied:      {}", netease_res.fallback_applied);
    println!("  Latency:               {:.2?}", netease_dur);
    println!("  Exact Error:           {:?}", netease_res.error);
    println!("  Credentials:           None (Public API)");

    // 2. LRCLIB
    println!("\n--- [2/3] Testing LRCLIB Adapter ---");
    let start_lrclib = Instant::now();
    let lrclib_res = client.resolve_lrclib("David Bowie", "Heroes", 371.0).await;
    let lrclib_dur = start_lrclib.elapsed();

    println!("  Provider:              {}", lrclib_res.provider);
    println!("  Strategy:              {}", lrclib_res.strategy);
    println!("  Status:                {:?}", lrclib_res.status);
    println!("  Sync Type:             {:?}", lrclib_res.sync_type);
    println!("  Line Count:            {}", lrclib_res.lines.len());
    println!("  Fallback Applied:      {}", lrclib_res.fallback_applied);
    println!("  Latency:               {:.2?}", lrclib_dur);
    println!("  Exact Error:           {:?}", lrclib_res.error);
    println!("  Credentials:           None (Public API)");

    // 3. LyricsPlus
    println!("\n--- [3/3] Testing LyricsPlus Adapter ---");
    let start_lyricsplus = Instant::now();
    let lyricsplus_res = client.resolve_lyricsplus("Queen", "Bohemian Rhapsody", 354.0).await;
    let lyricsplus_dur = start_lyricsplus.elapsed();

    println!("  Provider:              {}", lyricsplus_res.provider);
    println!("  Strategy:              {}", lyricsplus_res.strategy);
    println!("  Status:                {:?}", lyricsplus_res.status);
    println!("  Sync Type:             {:?}", lyricsplus_res.sync_type);
    println!("  Line Count:            {}", lyricsplus_res.lines.len());
    println!("  Fallback Applied:      {}", lyricsplus_res.fallback_applied);
    println!("  Latency:               {:.2?}", lyricsplus_dur);
    println!("  Exact Error:           {:?}", lyricsplus_res.error);
    println!("  Credentials:           None (Public API)");

    println!("\n============================================================");
    println!("              ADAPTER AUDIT COMPLETE                        ");
    println!("============================================================");
}
