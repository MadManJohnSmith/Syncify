use sqlx::SqlitePool;
use std::fs::File;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = "sqlite:c:/Users/madma/OneDrive/Documents/Syncify/src-tauri/data/syncify.db";
    let pool = SqlitePool::connect(database_url).await?;
    let mut output = File::create(
        "c:/Users/madma/OneDrive/Documents/Syncify/src-tauri/db_verification_output.txt",
    )?;

    writeln!(output, "--- Tarea 1: Sync Settings Persistence ---")?;
    let sync_settings = sqlx::query!("SELECT service_name, sync_favorites FROM service_sync_settings WHERE service_name = 'qobuz'")
        .fetch_optional(&pool)
        .await?;

    match sync_settings {
        Some(s) => writeln!(output, "Qobuz Sync Favorites: {}", s.sync_favorites)?,
        None => writeln!(output, "Qobuz sync settings not found in DB")?,
    }

    writeln!(output, "\n--- Tarea 2: Album Release Dates ---")?;
    let albums = sqlx::query!("SELECT title, release_date FROM albums WHERE release_date IS NOT NULL ORDER BY id DESC LIMIT 5")
        .fetch_all(&pool)
        .await?;

    if albums.is_empty() {
        writeln!(output, "No albums found with release_date in DB.")?;
    } else {
        for album in albums {
            writeln!(
                output,
                "Album: {:<30} | Release Date: {:?}",
                album.title, album.release_date
            )?;
        }
    }

    Ok(())
}
