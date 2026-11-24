//! Canonicalización de playlists por contenido (issue #74, ADR-0006 decisión 34).
//!
//! Las playlists de origen conservan su identidad remota y su contenido. La
//! canónica es una agrupación persistente de esas copias, no un borrado: así una
//! fuente que diverge después puede separarse sin perder su propio snapshot.

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Pool, Sqlite};

    use crate::services::playlist_canonical::{
        canonicalize_playlist, canonicalize_playlists, get_playlist_canonical_membership,
    };

    async fn migrated_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool :memory:");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("todas las migraciones reales aplican");
        pool
    }

    async fn seed_account(pool: &Pool<Sqlite>, service: &str, label: &str) -> i64 {
        sqlx::query_scalar(
            "INSERT INTO accounts (service_id, display_name, email)
             SELECT id, ?, ? FROM services WHERE name = ? RETURNING id",
        )
        .bind(label)
        .bind(format!("{label}@example.test"))
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("cuenta sembrada")
    }

    async fn seed_tracks(pool: &Pool<Sqlite>, count: i64) -> Vec<i64> {
        let mut ids = Vec::new();
        for index in 0..count {
            let id =
                sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
                    .bind(format!("Track {index}"))
                    .bind(format!("CANON-{}-{index}", uuid::Uuid::new_v4()))
                    .fetch_one(pool)
                    .await
                    .expect("track sembrada");
            ids.push(id);
        }
        ids
    }

    /// Fuente remota con cuenta, servicio y contenido. `declared_count` es la
    /// declaración del proveedor: diferirlo de las filas simula expansión
    /// incompleta incluso cuando el contador local se reconcilia después.
    async fn seed_remote_playlist(
        pool: &Pool<Sqlite>,
        account_id: i64,
        remote_id: &str,
        name: &str,
        declared_count: i64,
        track_ids: &[i64],
        created_at: &str,
    ) -> i64 {
        let playlist_id = sqlx::query_scalar(
            "INSERT INTO playlists
                (account_id, service_playlist_id, name, track_count, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(account_id)
        .bind(remote_id)
        .bind(name)
        .bind(declared_count)
        .bind(created_at)
        .bind(created_at)
        .fetch_one(pool)
        .await
        .expect("playlist remota sembrada");

        sqlx::query(
            "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
             SELECT ?, ?, service_id, ? FROM accounts WHERE id = ?",
        )
        .bind(playlist_id)
        .bind(account_id)
        .bind(remote_id)
        .bind(account_id)
        .execute(pool)
        .await
        .expect("fuente remota sembrada");

        for (position, track_id) in track_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                 VALUES (?, ?, ?)",
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(position as i64 + 1)
            .execute(pool)
            .await
            .expect("pista enlazada");
        }

        playlist_id
    }

    #[tokio::test]
    async fn exact_copies_cross_accounts_and_services_share_the_oldest_source() {
        let pool = migrated_pool().await;
        let spotify_a = seed_account(&pool, "spotify", "Spotify A").await;
        let spotify_b = seed_account(&pool, "spotify", "Spotify B").await;
        let qobuz = seed_account(&pool, "qobuz", "Qobuz").await;
        let tracks = seed_tracks(&pool, 2).await;

        let oldest = seed_remote_playlist(
            &pool,
            spotify_a,
            "sp-road",
            "  Road Trip  ",
            2,
            &[tracks[0], tracks[1]],
            "2024-01-01 00:00:00",
        )
        .await;
        let same_account_copy = seed_remote_playlist(
            &pool,
            spotify_a,
            "sp-road-copy",
            "road trip",
            2,
            &[tracks[1], tracks[0]],
            "2024-02-01 00:00:00",
        )
        .await;
        let other_service = seed_remote_playlist(
            &pool,
            qobuz,
            "qb-road",
            "ROAD TRIP",
            3,
            &[tracks[0], tracks[1], tracks[0]],
            "2024-03-01 00:00:00",
        )
        .await;
        let _other_account = seed_remote_playlist(
            &pool,
            spotify_b,
            "sp-road-b",
            "Road Trip",
            2,
            &[tracks[1], tracks[0]],
            "2024-04-01 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("canonicalización de catálogo");

        assert_eq!(summary.canonicals, 1);
        assert_eq!(summary.merged_canonicals, 1);
        assert_eq!(summary.playlists_linked, 4);

        for playlist_id in [oldest, same_account_copy, other_service] {
            let membership = get_playlist_canonical_membership(&pool, playlist_id)
                .await
                .expect("ficha canónica")
                .expect("playlist participa");
            assert_eq!(membership.sources.len(), 4);
            assert_eq!(membership.is_primary, playlist_id == oldest);
            assert_eq!(
                membership
                    .sources
                    .iter()
                    .filter(|source| source.is_primary)
                    .count(),
                1
            );
            assert_eq!(
                membership
                    .sources
                    .iter()
                    .find(|source| source.is_primary)
                    .unwrap()
                    .playlist_id,
                oldest
            );
            assert!(membership
                .sources
                .iter()
                .any(|source| source.account_id == spotify_a));
            assert!(membership
                .sources
                .iter()
                .any(|source| source.account_id == spotify_b));
            assert!(membership
                .sources
                .iter()
                .any(|source| source.service_name == "qobuz"));
        }
    }

    #[tokio::test]
    async fn same_name_with_different_content_stays_separate() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Spotify").await;
        let tracks = seed_tracks(&pool, 3).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "sp-first",
            "Mix",
            2,
            &tracks[..2],
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "sp-second",
            " MIX ",
            2,
            &[tracks[0], tracks[2]],
            "2024-01-02 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("canonicalización");

        assert_eq!(summary.canonicals, 2);
        assert_eq!(summary.merged_canonicals, 0);
        let first_membership = get_playlist_canonical_membership(&pool, first)
            .await
            .expect("primera ficha")
            .expect("primera participa");
        let second_membership = get_playlist_canonical_membership(&pool, second)
            .await
            .expect("segunda ficha")
            .expect("segunda participa");
        assert_ne!(
            first_membership.canonical_id,
            second_membership.canonical_id
        );
        assert_eq!(first_membership.sources.len(), 1);
        assert_eq!(second_membership.sources.len(), 1);
    }

    #[tokio::test]
    async fn empty_and_incomplete_expansions_do_not_merge() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "tidal", "Tidal").await;
        let tracks = seed_tracks(&pool, 2).await;
        let empty = seed_remote_playlist(
            &pool,
            account,
            "td-empty",
            "Empty",
            0,
            &[],
            "2024-01-01 00:00:00",
        )
        .await;
        let incomplete = seed_remote_playlist(
            &pool,
            account,
            "td-incomplete",
            "Incomplete",
            3,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;
        let complete_a = seed_remote_playlist(
            &pool,
            account,
            "td-complete-a",
            "Complete",
            2,
            &tracks,
            "2024-01-03 00:00:00",
        )
        .await;
        let complete_b = seed_remote_playlist(
            &pool,
            account,
            "td-complete-b",
            "complete",
            2,
            &[tracks[1], tracks[0]],
            "2024-01-04 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("canonicalización");

        assert_eq!(summary.empty_playlists, 1);
        assert_eq!(summary.incomplete_playlists, 1);
        assert_eq!(summary.canonicals, 1);
        assert_eq!(summary.playlists_linked, 2);
        assert!(get_playlist_canonical_membership(&pool, empty)
            .await
            .expect("vacía leída")
            .is_none());
        assert!(get_playlist_canonical_membership(&pool, incomplete)
            .await
            .expect("incompleta leída")
            .is_none());
        let membership = get_playlist_canonical_membership(&pool, complete_a)
            .await
            .expect("completa leída")
            .expect("completa participa");
        assert_eq!(membership.sources.len(), 2);
        assert!(membership.is_primary);
        assert_eq!(
            get_playlist_canonical_membership(&pool, complete_b)
                .await
                .expect("copia leída")
                .unwrap()
                .canonical_id,
            membership.canonical_id
        );
    }

    #[tokio::test]
    async fn measured_64_name_groups_merge_only_the_16_exact_sets() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Measured").await;
        let tracks = seed_tracks(&pool, 3).await;

        for group in 0..64 {
            let first_tracks = if group % 2 == 0 {
                vec![tracks[0], tracks[1]]
            } else {
                vec![tracks[0], tracks[2]]
            };
            let second_tracks = if group < 16 {
                vec![first_tracks[1], first_tracks[0]]
            } else {
                vec![tracks[1], tracks[2]]
            };
            seed_remote_playlist(
                &pool,
                account,
                &format!("measured-{group}-a"),
                &format!(" Measured Group {group:02} "),
                2,
                &first_tracks,
                "2024-01-01 00:00:00",
            )
            .await;
            seed_remote_playlist(
                &pool,
                account,
                &format!("measured-{group}-b"),
                &format!("measured group {group:02}"),
                2,
                &second_tracks,
                "2024-01-02 00:00:00",
            )
            .await;
        }

        let summary = canonicalize_playlists(&pool).await.expect("64 grupos");

        assert_eq!(summary.playlists_examined, 128);
        assert_eq!(summary.playlists_linked, 128);
        assert_eq!(summary.merged_canonicals, 16);
        assert_eq!(summary.canonicals, 16 + 48 * 2);
        assert_eq!(summary.incomplete_playlists, 0);
        assert_eq!(summary.empty_playlists, 0);
    }

    #[tokio::test]
    async fn changed_content_splits_from_its_copy_and_reruns_are_idempotent() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "qobuz", "Qobuz").await;
        let tracks = seed_tracks(&pool, 3).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "qb-first",
            "Split",
            2,
            &[tracks[0], tracks[1]],
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "qb-second",
            "split",
            2,
            &[tracks[1], tracks[0]],
            "2024-01-02 00:00:00",
        )
        .await;

        canonicalize_playlists(&pool).await.expect("fusión inicial");
        let before = get_playlist_canonical_membership(&pool, second)
            .await
            .expect("antes del cambio")
            .unwrap();
        assert_eq!(before.sources.len(), 2);

        sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
            .bind(second)
            .execute(&pool)
            .await
            .expect("contenido anterior retirado");
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position)
             VALUES (?, ?, 1), (?, ?, 2)",
        )
        .bind(second)
        .bind(tracks[0])
        .bind(second)
        .bind(tracks[2])
        .execute(&pool)
        .await
        .expect("nuevo contenido");
        sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
            .bind(second)
            .execute(&pool)
            .await
            .expect("declaración actualizada");
        sqlx::query(
            "UPDATE playlist_sources SET synced_at = CURRENT_TIMESTAMP WHERE playlist_id = ?",
        )
        .bind(second)
        .execute(&pool)
        .await
        .expect("fuente reexpandida");

        canonicalize_playlist(&pool, second)
            .await
            .expect("separación incremental");

        let first_after = get_playlist_canonical_membership(&pool, first)
            .await
            .expect("primera tras el cambio")
            .unwrap();
        let second_after = get_playlist_canonical_membership(&pool, second)
            .await
            .expect("segunda tras el cambio")
            .unwrap();
        assert_ne!(first_after.canonical_id, second_after.canonical_id);
        assert_eq!(first_after.sources.len(), 1);
        assert_eq!(second_after.sources.len(), 1);

        let once = canonicalize_playlists(&pool)
            .await
            .expect("primera repetición");
        let twice = canonicalize_playlists(&pool)
            .await
            .expect("segunda repetición");
        assert_eq!(once.canonicals, twice.canonicals);
        assert_eq!(once.playlists_linked, twice.playlists_linked);
        assert_eq!(once.merged_canonicals, twice.merged_canonicals);
    }
}
