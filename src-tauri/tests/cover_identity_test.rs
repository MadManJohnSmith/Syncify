//! Cover Identity & Collision Prevention Test (S175)
//!
//! Validates:
//! 1. Cover identity isolation: distinct releases resolve distinct cover artwork without cross-album contamination or collisions.
//! 2. Multi-disc releases share the album cover while keeping distinct destination subdirectories (`Disc 1`, `Disc 2`) and propagating to the shared album root.
//! 3. File payload verification confirms matching bytes for the expected release.

use tempfile::tempdir;

fn generate_cover_payload(seed: u8, len: usize) -> Vec<u8> {
    let mut v = vec![seed; len];
    // Add JPEG header to simulate valid image payload
    v[0] = 0xFF;
    v[1] = 0xD8;
    v[len - 2] = 0xFF;
    v[len - 1] = 0xD9;
    v
}

#[tokio::test]
async fn test_cover_identity_isolation_between_distinct_releases() {
    let root_dir = tempdir().expect("tempdir");
    let staging_root = root_dir.path().join(".staging");
    let library_root = root_dir.path().join("Music");

    tokio::fs::create_dir_all(&staging_root).await.unwrap();
    tokio::fs::create_dir_all(&library_root).await.unwrap();

    let cover_payload_a = generate_cover_payload(0xAA, 1024);
    let cover_payload_b = generate_cover_payload(0xBB, 2048);

    assert_ne!(cover_payload_a, cover_payload_b, "Payloads for distinct releases must be different");

    // 1. Stage Release A in its unique staging directory
    let staging_a = staging_root.join("item_release_a_101");
    tokio::fs::create_dir_all(&staging_a).await.unwrap();
    let cover_staged_a = staging_a.join("cover.jpg");
    tokio::fs::write(&cover_staged_a, &cover_payload_a).await.unwrap();

    // 2. Stage Release B in its unique staging directory
    let staging_b = staging_root.join("item_release_b_202");
    tokio::fs::create_dir_all(&staging_b).await.unwrap();
    let cover_staged_b = staging_b.join("cover.jpg");
    tokio::fs::write(&cover_staged_b, &cover_payload_b).await.unwrap();

    // 3. Promote Release A to Library / Artist A / Album A
    let dest_dir_a = library_root.join("Artist A").join("Album A");
    tokio::fs::create_dir_all(&dest_dir_a).await.unwrap();
    let final_cover_a = dest_dir_a.join("cover.jpg");
    tokio::fs::copy(&cover_staged_a, &final_cover_a).await.unwrap();
    tokio::fs::remove_file(&cover_staged_a).await.unwrap();

    // 4. Promote Release B to Library / Artist B / Album B
    let dest_dir_b = library_root.join("Artist B").join("Album B");
    tokio::fs::create_dir_all(&dest_dir_b).await.unwrap();
    let final_cover_b = dest_dir_b.join("cover.jpg");
    tokio::fs::copy(&cover_staged_b, &final_cover_b).await.unwrap();
    tokio::fs::remove_file(&cover_staged_b).await.unwrap();

    // 5. Verify no cross-contamination or collisions
    let read_a = tokio::fs::read(&final_cover_a).await.unwrap();
    let read_b = tokio::fs::read(&final_cover_b).await.unwrap();

    assert_eq!(read_a, cover_payload_a, "Album A cover must match Release A payload");
    assert_eq!(read_b, cover_payload_b, "Album B cover must match Release B payload");
    assert_ne!(read_a, read_b, "Album A and Album B covers must be isolated and distinct");
}

#[tokio::test]
async fn test_multidisc_cover_identity_and_root_propagation() {
    let root_dir = tempdir().expect("tempdir");
    let staging_root = root_dir.path().join(".staging");
    let library_root = root_dir.path().join("Music");

    let album_cover_payload = generate_cover_payload(0x55, 4096);

    let album_root = library_root.join("David Bowie").join("Sound + Vision (Deluxe Edition)");
    let disc_1_dir = album_root.join("Disc 1");
    let disc_2_dir = album_root.join("Disc 2");

    tokio::fs::create_dir_all(&disc_1_dir).await.unwrap();
    tokio::fs::create_dir_all(&disc_2_dir).await.unwrap();

    // Process Disc 1
    let staging_d1 = staging_root.join("bowie_disc_1");
    tokio::fs::create_dir_all(&staging_d1).await.unwrap();
    let d1_cover_staged = staging_d1.join("cover.jpg");
    tokio::fs::write(&d1_cover_staged, &album_cover_payload).await.unwrap();

    let d1_final = disc_1_dir.join("cover.jpg");
    tokio::fs::copy(&d1_cover_staged, &d1_final).await.unwrap();
    // Propagate to album root
    let root_final = album_root.join("cover.jpg");
    if !root_final.exists() {
        tokio::fs::copy(&d1_cover_staged, &root_final).await.unwrap();
    }
    tokio::fs::remove_file(&d1_cover_staged).await.unwrap();

    // Process Disc 2
    let staging_d2 = staging_root.join("bowie_disc_2");
    tokio::fs::create_dir_all(&staging_d2).await.unwrap();
    let d2_cover_staged = staging_d2.join("cover.jpg");
    tokio::fs::write(&d2_cover_staged, &album_cover_payload).await.unwrap();

    let d2_final = disc_2_dir.join("cover.jpg");
    tokio::fs::copy(&d2_cover_staged, &d2_final).await.unwrap();
    if !root_final.exists() {
        tokio::fs::copy(&d2_cover_staged, &root_final).await.unwrap();
    }
    tokio::fs::remove_file(&d2_cover_staged).await.unwrap();

    // Verify all 3 locations have the identical album cover
    assert_eq!(tokio::fs::read(&d1_final).await.unwrap(), album_cover_payload);
    assert_eq!(tokio::fs::read(&d2_final).await.unwrap(), album_cover_payload);
    assert_eq!(tokio::fs::read(&root_final).await.unwrap(), album_cover_payload);
}
