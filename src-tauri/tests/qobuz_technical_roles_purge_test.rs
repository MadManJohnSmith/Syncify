//! Integration and Regression Test Suite for TASK-68:
//! Depuración de Artistas Hipertrofiados con Roles Técnicos de Qobuz en la Tabla `artists`.
//!
//! Validates:
//! 1. Domain parser and sanitizer (`parse_credit_role_and_name`, `sanitize_artist_name`,
//!    `has_technical_role_prefix`, `is_technical_role`, `parse_credits_string`) correctly separate
//!    technical roles from artist names while preserving legitimate names ('Guitar Wolf', 'Jean-Luc Ponty').
//! 2. Migration 0082 purges technical role prefixes ('Guitar - ...', 'Choir - ...', 'Producer - ...'),
//!    remaps track_credits/track_artists/album_artists, unifies duplicates, preserves roles, and purges unlinked stubs.
//! 3. Recurrence prevention triggers (`trg_artists_reject_technical_roles_ins`, `trg_artists_reject_technical_roles_upd`)
//!    strictly reject inserting or updating artists with technical role prefixes.
//! 4. Qobuz import gate sanitizes and rejects technical role strings before DB persistence.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_core_domain::metadata::{
    has_technical_role_prefix, is_technical_role, parse_credit_role_and_name, parse_credits_string,
    sanitize_artist_name,
};

#[test]
fn test_domain_technical_role_credit_separation_and_sanitization() {
    // 1. Hyphen role prefix separation: "Role - Name"
    let (name_gt, role_gt) = parse_credit_role_and_name("Guitar - Juan Perez", "performer");
    assert_eq!(name_gt, "Juan Perez");
    assert_eq!(role_gt, "Guitar");

    let (name_ch, role_ch) = parse_credit_role_and_name("Choir - Coro de Praga", "performer");
    assert_eq!(name_ch, "Coro de Praga");
    assert_eq!(role_ch, "Choir");

    let (name_comp, role_comp) = parse_credit_role_and_name("Composer - Beethoven", "composer");
    assert_eq!(name_comp, "Beethoven");
    assert_eq!(role_comp, "Composer");

    let (name_prod, role_prod) = parse_credit_role_and_name("Producer - Quincy Jones", "producer");
    assert_eq!(name_prod, "Quincy Jones");
    assert_eq!(role_prod, "Producer");

    let (name_voc, role_voc) = parse_credit_role_and_name("Vocals - John Doe", "performer");
    assert_eq!(name_voc, "John Doe");
    assert_eq!(role_voc, "Vocals");

    let (name_bass, role_bass) = parse_credit_role_and_name("Bass - Flea", "performer");
    assert_eq!(name_bass, "Flea");
    assert_eq!(role_bass, "Bass");

    let (name_drums, role_drums) = parse_credit_role_and_name("Drums - Dave Grohl", "performer");
    assert_eq!(name_drums, "Dave Grohl");
    assert_eq!(role_drums, "Drums");

    let (name_mixer, role_mixer) = parse_credit_role_and_name("Mixer - Bob Clearmountain", "mixer");
    assert_eq!(name_mixer, "Bob Clearmountain");
    assert_eq!(role_mixer, "Mixer");

    let (name_eng, role_eng) = parse_credit_role_and_name("Engineer - Al Schmitt", "engineer");
    assert_eq!(name_eng, "Al Schmitt");
    assert_eq!(role_eng, "Engineer");

    // 2. Comma separation: "Role, Name" and "Name, Role"
    let (name_c1, role_c1) = parse_credit_role_and_name("Guitar, Juan Perez", "performer");
    assert_eq!(name_c1, "Juan Perez");
    assert_eq!(role_c1, "Guitar");

    let (name_c2, role_c2) = parse_credit_role_and_name("Juan Perez, Guitar", "performer");
    assert_eq!(name_c2, "Juan Perez");
    assert_eq!(role_c2, "Guitar");

    // 3. Name - Role format
    let (name_rev, role_rev) = parse_credit_role_and_name("Freddie Mercury - Vocals, Piano", "performer");
    assert_eq!(name_rev, "Freddie Mercury");
    assert_eq!(role_rev, "Vocals, Piano");

    // 4. sanitize_artist_name strips technical role prefixes
    assert_eq!(sanitize_artist_name("Guitar - Juan Perez"), "Juan Perez");
    assert_eq!(sanitize_artist_name("Choir - Coro de Praga"), "Coro de Praga");
    assert_eq!(sanitize_artist_name("Composer - Beethoven"), "Beethoven");
    assert_eq!(sanitize_artist_name("Producer - Quincy Jones"), "Quincy Jones");
    assert_eq!(sanitize_artist_name("Vocals - John Doe"), "John Doe");

    // 5. Legitimate artists with hyphens or musical words preserved intact
    assert_eq!(sanitize_artist_name("Guitar Wolf"), "Guitar Wolf");
    assert_eq!(sanitize_artist_name("Jean-Luc Ponty"), "Jean-Luc Ponty");
    assert_eq!(sanitize_artist_name("AC/DC"), "AC/DC");
    assert_eq!(sanitize_artist_name("Pink Floyd"), "Pink Floyd");

    // 6. has_technical_role_prefix validation
    assert!(has_technical_role_prefix("Guitar - Juan Perez"));
    assert!(has_technical_role_prefix("Choir - Coro de Praga"));
    assert!(has_technical_role_prefix("Composer - Beethoven"));
    assert!(has_technical_role_prefix("Producer - Quincy Jones"));
    assert!(has_technical_role_prefix("Vocals - John Doe"));
    assert!(!has_technical_role_prefix("Guitar Wolf"));
    assert!(!has_technical_role_prefix("Jean-Luc Ponty"));
    assert!(!has_technical_role_prefix("Pink Floyd"));
    assert!(!has_technical_role_prefix("Juan Perez"));

    // 7. is_technical_role validation
    assert!(is_technical_role("Guitar"));
    assert!(is_technical_role("electric guitar"));
    assert!(is_technical_role("Choir"));
    assert!(is_technical_role("Producer"));
    assert!(is_technical_role("Recording Engineer"));
    assert!(is_technical_role("Vocals, Piano"));
    assert!(!is_technical_role("Pink Floyd"));
    assert!(!is_technical_role("Juan Perez"));

    // 8. Multi-credit list parsing
    let credits = parse_credits_string("Guitar - Juan Perez, Choir - Coro de Praga", "performer");
    assert_eq!(
        credits,
        vec![
            ("Juan Perez".to_string(), "Guitar".to_string()),
            ("Coro de Praga".to_string(), "Choir".to_string()),
        ]
    );

    // 9. Serialized JSON performer object parsing
    let json_credits = parse_credits_string(
        r#"{"guitar": "Brian May", "main": "Freddie Mercury - Vocals, Piano"}"#,
        "performer",
    );
    assert!(json_credits.iter().any(|(n, r)| n == "Brian May" && r == "guitar"));
    assert!(json_credits.iter().any(|(n, r)| n == "Freddie Mercury" && r == "Vocals, Piano"));
}

#[tokio::test]
async fn test_migration_0082_purges_technical_role_artists_and_preserves_legitimate() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    // 1. Run migrations 0001 through 0081
    let migrator = sqlx::migrate!("./migrations");
    for m in migrator.iter() {
        if m.version < 82 {
            // Apply migrations up to 0081
            let sql = &m.sql;
            sqlx::raw_sql(sql)
                .execute(&pool)
                .await
                .unwrap_or_else(|e| panic!("Failed to apply migration {:?}: {}", m.version, e));
        }
    }

    // 2. Seed test data:
    // Existing canonical artist
    let juan_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Juan Perez') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Contaminated artist matching existing canonical
    let guitar_juan_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, spotify_id) VALUES ('Guitar - Juan Perez', 'sp_juan') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Contaminated artist without existing clean (will be renamed winner)
    let choir_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, qobuz_id) VALUES ('Choir - Coro de Praga', 'q_choir') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Contaminated producer (will be renamed winner)
    let quincy_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Producer - Quincy Jones') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Unlinked residual contaminated artist (no tracks, no albums) -> should be purged
    let ghost_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Drums - Ghost Drummer') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Legitimate artists
    let guitar_wolf_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Guitar Wolf') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let pink_floyd_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let jean_luc_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Jean-Luc Ponty') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Seed Albums & Tracks
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let track1_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Track 1', ?) RETURNING id")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let track2_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Track 2', ?) RETURNING id")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    // Link contaminated artists
    // Link Guitar - Juan Perez to track1 (as primary) and track_credits
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track1_id)
        .bind(guitar_juan_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_credits (track_id, artist_id, role) VALUES (?, ?, 'performer')")
        .bind(track1_id)
        .bind(guitar_juan_id)
        .execute(&pool)
        .await
        .unwrap();

    // Link Choir - Coro de Praga to track2
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track2_id)
        .bind(choir_id)
        .execute(&pool)
        .await
        .unwrap();

    // Link Producer - Quincy Jones to track2 credits
    sqlx::query("INSERT INTO track_credits (track_id, artist_id, role) VALUES (?, ?, 'producer')")
        .bind(track2_id)
        .bind(quincy_id)
        .execute(&pool)
        .await
        .unwrap();

    // Link Legitimate artists
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id)
        .bind(pink_floyd_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track2_id)
        .bind(guitar_wolf_id)
        .execute(&pool)
        .await
        .unwrap();

    // 3. Apply Migration 0082
    let migration_0082_sql = include_str!("../migrations/0082_purge_technical_role_artists.sql");
    sqlx::raw_sql(migration_0082_sql)
        .execute(&pool)
        .await
        .expect("Migration 0082 must execute cleanly");

    // 4. Assert contaminated artists are gone
    let (contaminated_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE name LIKE 'Guitar - %' OR name LIKE 'Choir - %' OR name LIKE 'Producer - %' OR name LIKE 'Drums - %'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(contaminated_count, 0, "No contaminated technical role artists must remain");

    // 5. Assert canonical Juan Perez retained and merged
    let juan_row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, spotify_id FROM artists WHERE id = ?"
    )
    .bind(juan_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(juan_row.is_some());
    let (j_name, j_sp) = juan_row.unwrap();
    assert_eq!(j_name, "Juan Perez");
    assert_eq!(j_sp.as_deref(), Some("sp_juan"), "Metadata from source must be consolidated onto canonical");

    // Track 1 artist remapped to juan_id
    let (t1_artist,): (i64,) = sqlx::query_as(
        "SELECT artist_id FROM track_artists WHERE track_id = ? AND role = 'primary'"
    )
    .bind(track1_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(t1_artist, juan_id);

    // Track 1 credit updated to extracted role 'Guitar'
    let (t1_credit_role, t1_credit_art): (String, i64) = sqlx::query_as(
        "SELECT role, artist_id FROM track_credits WHERE track_id = ?"
    )
    .bind(track1_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(t1_credit_art, juan_id);
    assert_eq!(t1_credit_role, "Guitar", "Role must be updated to the extracted technical role");

    // 6. Assert winner artists renamed to clean names
    let choir_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(choir_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(choir_name, "Coro de Praga");

    let quincy_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(quincy_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(quincy_name, "Quincy Jones");

    // 7. Assert ghost unlinked artist purged
    let ghost_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM artists WHERE id = ?")
        .bind(ghost_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(ghost_exists.is_none(), "Unlinked residual artist must be purged");

    // 8. Assert legitimate artists untouched
    let gw_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(guitar_wolf_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gw_name, "Guitar Wolf");

    let pf_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(pink_floyd_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pf_name, "Pink Floyd");

    let jl_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(jean_luc_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jl_name, "Jean-Luc Ponty");

    // 9. Foreign key check & integrity check
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(fk_violations.is_empty(), "0 foreign key violations expected after migration");

    let (integrity,): (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(integrity, "ok", "Database integrity check must pass");
}

#[tokio::test]
async fn test_recurrence_prevention_triggers_reject_technical_role_artists() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    // Run all migrations including 0082
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // 1. Reject insertion of artists with technical role prefixes
    let ins_guitar = sqlx::query("INSERT INTO artists (name) VALUES ('Guitar - Paco de Lucia')")
        .execute(&pool)
        .await;
    assert!(ins_guitar.is_err(), "Trigger must reject inserting 'Guitar - Paco de Lucia'");

    let ins_producer = sqlx::query("INSERT INTO artists (name) VALUES ('Producer - George Martin')")
        .execute(&pool)
        .await;
    assert!(ins_producer.is_err(), "Trigger must reject inserting 'Producer - George Martin'");

    let ins_drums = sqlx::query("INSERT INTO artists (name) VALUES ('Drums - John Bonham')")
        .execute(&pool)
        .await;
    assert!(ins_drums.is_err(), "Trigger must reject inserting 'Drums - John Bonham'");

    let ins_vocals = sqlx::query("INSERT INTO artists (name) VALUES ('Vocals - Freddie Mercury')")
        .execute(&pool)
        .await;
    assert!(ins_vocals.is_err(), "Trigger must reject inserting 'Vocals - Freddie Mercury'");

    let ins_bass = sqlx::query("INSERT INTO artists (name) VALUES ('Bass - Jaco Pastorius')")
        .execute(&pool)
        .await;
    assert!(ins_bass.is_err(), "Trigger must reject inserting 'Bass - Jaco Pastorius'");

    // 2. Reject updating artist name to technical role prefix
    let clean_ins = sqlx::query("INSERT INTO artists (name) VALUES ('Paul McCartney')")
        .execute(&pool)
        .await;
    assert!(clean_ins.is_ok(), "Clean artist insert must succeed");

    let update_bad = sqlx::query("UPDATE artists SET name = 'Bass - Paul McCartney' WHERE name = 'Paul McCartney'")
        .execute(&pool)
        .await;
    assert!(update_bad.is_err(), "Trigger must reject updating name to technical role prefix");

    // 3. Allow clean and legitimate artist inserts
    let ins_clean1 = sqlx::query("INSERT INTO artists (name) VALUES ('Paco de Lucia')")
        .execute(&pool)
        .await;
    assert!(ins_clean1.is_ok(), "Clean artist 'Paco de Lucia' must be allowed");

    let ins_gw = sqlx::query("INSERT INTO artists (name) VALUES ('Guitar Wolf')")
        .execute(&pool)
        .await;
    assert!(ins_gw.is_ok(), "Legitimate artist 'Guitar Wolf' must be allowed");

    let ins_jl = sqlx::query("INSERT INTO artists (name) VALUES ('Jean-Luc Ponty')")
        .execute(&pool)
        .await;
    assert!(ins_jl.is_ok(), "Legitimate artist 'Jean-Luc Ponty' must be allowed");
}

#[tokio::test]
async fn test_qobuz_service_get_or_create_artist_gate() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    let qobuz_client = syncify_tauri_lib::services::QobuzClient::new("test".to_string(), "test".to_string());

    // Calling get_or_create_artist with "Guitar - Juan Perez" should sanitize to "Juan Perez"
    let artist_id = qobuz_client
        .get_or_create_artist(&pool, "Guitar - Juan Perez")
        .await
        .expect("get_or_create_artist must succeed by sanitizing technical role prefix");

    assert!(artist_id > 0);

    let saved_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(artist_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(saved_name, "Juan Perez", "Persisted artist name must be the clean name, never the technical prefix");
}
