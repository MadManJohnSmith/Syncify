//! Account identity: per-account email/username for the accounts surface.
//!
//! The accounts screen tells twin accounts apart by email first and display
//! name second. Services that reveal neither (Apple Music) or that were
//! connected before the identity capture existed leave several rows sharing
//! the generic service name ("Qobuz User"), and two accounts of the same
//! person become indistinguishable. These tests pin the update policy that
//! fixes that without ever overwriting the user's own decision:
//!   1. A custom name is never replaced (neither by a login nor by the refresh).
//!   2. A real provider name is never replaced by another provider string.
//!   3. Emails are stored only when plausible, and only fill an empty slot.
//!   4. The refresh is best effort: an unresolvable account changes nothing,
//!      and a service without a profile API (Apple Music) makes no call.
//!
//! No test here touches the network: the refresh probes Apple Music rows,
//! whose resolver arm answers instantly with "no identity".

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    apply_provider_identity, identity_needs_refresh, perform_refresh_account_identities,
    perform_set_account_display_name, plausible_identity_email, upsert_service_account,
    ProviderIdentity,
};
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn service_id(pool: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("service {name} must exist: {e}"))
}

/// Insert an account row with the identity fields under test and no credentials.
async fn insert_identity_account(
    pool: &sqlx::SqlitePool,
    service: &str,
    display_name: &str,
    email: Option<&str>,
    custom: bool,
) -> i64 {
    let svc = service_id(pool, service).await;
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, display_name_custom, is_active, credentials_invalid)
           VALUES (?, ?, ?, ?, 1, 0) RETURNING id"#,
    )
    .bind(svc)
    .bind(display_name)
    .bind(email)
    .bind(if custom { 1 } else { 0 })
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn identity_of(pool: &sqlx::SqlitePool, account_id: i64) -> (String, Option<String>, i64) {
    sqlx::query_as("SELECT display_name, email, display_name_custom FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[test]
fn identity_policy_helpers() {
    // Generic or missing name without email: exactly what the refresh exists for.
    assert!(identity_needs_refresh(
        "qobuz",
        Some("Qobuz User"),
        None,
        false
    ));
    assert!(identity_needs_refresh(
        "qobuz",
        Some("qobuz user"),
        Some("  "),
        false
    ));
    assert!(identity_needs_refresh("tidal", None, None, false));
    // A real name, an email or a custom label already distinguish the row.
    assert!(!identity_needs_refresh(
        "soundcloud",
        Some("ana_sc"),
        None,
        false
    ));
    assert!(!identity_needs_refresh(
        "qobuz",
        Some("Qobuz User"),
        Some("a@b.com"),
        false
    ));
    assert!(!identity_needs_refresh(
        "qobuz",
        Some("Qobuz User"),
        None,
        true
    ));

    // Plausibility is deliberately conservative: it rejects artifacts, not
    // exotic-but-valid addresses. Output is lowercased (UNIQUE is case-sensitive).
    assert_eq!(
        plausible_identity_email("  Ana@Example.com  ").as_deref(),
        Some("ana@example.com")
    );
    assert_eq!(plausible_identity_email("no-at-sign"), None);
    assert_eq!(plausible_identity_email("a@b"), None);
    assert_eq!(plausible_identity_email("a b@c.com"), None);
    assert_eq!(plausible_identity_email("a@b..com"), None);
    assert_eq!(plausible_identity_email("a@.com"), None);
    assert_eq!(plausible_identity_email("  "), None);
}

#[tokio::test]
async fn apply_provider_identity_respects_custom_and_real_names() {
    let pool = setup_test_db().await;

    // 1) Generic name + empty email: both fields are filled.
    let generic = insert_identity_account(&pool, "qobuz", "Qobuz User", None, false).await;
    let changed = apply_provider_identity(
        &pool,
        generic,
        &ProviderIdentity {
            display_name: Some("Ana Sounds".into()),
            email: Some("Ana@Example.com".into()),
        },
    )
    .await
    .unwrap();
    assert!(changed, "a generic row must change");
    let (name, email, custom) = identity_of(&pool, generic).await;
    assert_eq!(name, "Ana Sounds");
    assert_eq!(email.as_deref(), Some("ana@example.com"));
    assert_eq!(custom, 0);

    // 2) Custom name: kept, because it is the user's decision; the email still
    //    fills its empty slot (identity is data, the label is the user's).
    let custom_row = insert_identity_account(&pool, "qobuz", "Studio account", None, true).await;
    let changed = apply_provider_identity(
        &pool,
        custom_row,
        &ProviderIdentity {
            display_name: Some("Other Name".into()),
            email: Some("studio@example.com".into()),
        },
    )
    .await
    .unwrap();
    assert!(changed);
    let (name, email, custom) = identity_of(&pool, custom_row).await;
    assert_eq!(name, "Studio account");
    assert_eq!(email.as_deref(), Some("studio@example.com"));
    assert_eq!(custom, 1);

    // 3) A real provider name is never replaced by another provider string, and
    //    with an email already on file there is nothing to write.
    let real =
        insert_identity_account(&pool, "qobuz", "ana_qbz", Some("real@example.com"), false).await;
    let changed = apply_provider_identity(
        &pool,
        real,
        &ProviderIdentity {
            display_name: Some("Distinta".into()),
            email: Some("otra@example.com".into()),
        },
    )
    .await
    .unwrap();
    assert!(!changed, "a real identity must not churn");
    let (name, email, _) = identity_of(&pool, real).await;
    assert_eq!(name, "ana_qbz");
    assert_eq!(email.as_deref(), Some("real@example.com"));

    // 4) Opaque ids and implausible emails are not identities: the row stays
    //    generic instead of showing a provider number.
    let opaque = insert_identity_account(&pool, "qobuz", "Qobuz User", None, false).await;
    let changed = apply_provider_identity(
        &pool,
        opaque,
        &ProviderIdentity {
            display_name: Some("123456789".into()),
            email: Some("not-an-email".into()),
        },
    )
    .await
    .unwrap();
    assert!(!changed, "an opaque id is not a name");
    let (name, email, custom) = identity_of(&pool, opaque).await;
    assert_eq!(name, "Qobuz User");
    assert_eq!(email, None);
    assert_eq!(custom, 0);

    // 5) A generic display_name coming back from the resolver must not be
    //    written as if it were a discovery.
    let still_generic = insert_identity_account(&pool, "tidal", "Tidal User", None, false).await;
    let changed = apply_provider_identity(
        &pool,
        still_generic,
        &ProviderIdentity {
            display_name: Some("Tidal User".into()),
            email: None,
        },
    )
    .await
    .unwrap();
    assert!(!changed);
}

#[tokio::test]
async fn refresh_counts_only_probed_accounts_and_never_fails_the_pass() {
    let pool = setup_test_db().await;

    // Generic row WITH credentials: probed, but Apple Music has no profile API,
    // so it counts as checked + unresolved and the row must stay untouched.
    let apple =
        insert_identity_account(&pool, "apple_music", "apple_music User", None, false).await;
    let creds =
        crypto::encrypt(&serde_json::json!({"music_user_token": "token"}).to_string()).unwrap();
    sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
        .bind(&creds)
        .bind(apple)
        .execute(&pool)
        .await
        .unwrap();

    // A custom label is never probed.
    let custom_row = insert_identity_account(&pool, "apple_music", "Casa", None, true).await;
    sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
        .bind(&creds)
        .bind(custom_row)
        .execute(&pool)
        .await
        .unwrap();

    // A real name (or an email) is already distinguishable: not probed either.
    let named = insert_identity_account(&pool, "tidal", "ana_tdl", None, false).await;

    let summary = perform_refresh_account_identities(&pool).await.unwrap();
    assert_eq!(
        summary.checked, 1,
        "only the generic Apple Music row is probed"
    );
    assert_eq!(summary.updated, 0);
    assert_eq!(summary.unresolved, 1);

    // Nothing was modified: no invented identity, no lost custom name.
    let (name, email, _) = identity_of(&pool, apple).await;
    assert_eq!(name, "apple_music User");
    assert_eq!(email, None);
    let (name, _, custom) = identity_of(&pool, custom_row).await;
    assert_eq!(name, "Casa");
    assert_eq!(custom, 1);
    let (name, _, _) = identity_of(&pool, named).await;
    assert_eq!(name, "ana_tdl");

    // A second pass is harmless (idempotent).
    let again = perform_refresh_account_identities(&pool).await.unwrap();
    assert_eq!(again.checked, 1);
    assert_eq!(again.updated, 0);
}

#[tokio::test]
async fn login_upsert_preserves_a_custom_name() {
    let pool = setup_test_db().await;
    let qobuz = service_id(&pool, "qobuz").await;

    let custom_row =
        insert_identity_account(&pool, "qobuz", "Estudio", Some("estudio@example.com"), true).await;

    // The same login happens again: the bridge can only offer the generic name.
    let creds =
        crypto::encrypt(&serde_json::json!({"user_auth_token": "tok"}).to_string()).unwrap();
    let upserted = upsert_service_account(
        &pool,
        qobuz,
        "Qobuz User",
        Some("Estudio@Example.com"),
        None,
        &creds,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        upserted, custom_row,
        "the login must land on the same account"
    );
    let (name, email, custom) = identity_of(&pool, custom_row).await;
    assert_eq!(
        name, "Estudio",
        "a re-login cannot overwrite the user's name"
    );
    assert_eq!(email.as_deref(), Some("estudio@example.com"));
    assert_eq!(custom, 1);
}

#[tokio::test]
async fn set_account_display_name_marks_custom_and_rejects_empty() {
    let pool = setup_test_db().await;
    let account = insert_identity_account(&pool, "tidal", "Tidal User", None, false).await;

    perform_set_account_display_name(&pool, account, "  Casa  ")
        .await
        .unwrap();
    let (name, _, custom) = identity_of(&pool, account).await;
    assert_eq!(name, "Casa", "the stored name is trimmed");
    assert_eq!(custom, 1, "a user-chosen name must be protected");

    // Empty/whitespace names and unknown accounts are rejected, and rejection
    // changes nothing.
    assert!(perform_set_account_display_name(&pool, account, "   ")
        .await
        .is_err());
    assert!(
        perform_set_account_display_name(&pool, account, &"x".repeat(129))
            .await
            .is_err()
    );
    assert!(perform_set_account_display_name(&pool, 999_999, "x")
        .await
        .is_err());
    let (name, _, custom) = identity_of(&pool, account).await;
    assert_eq!(name, "Casa");
    assert_eq!(custom, 1);
}
