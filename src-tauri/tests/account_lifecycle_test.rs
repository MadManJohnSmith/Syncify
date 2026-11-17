//! Issue #85 — transiciones de ciclo de vida y efectos del borrado de cuenta.
//!
//! Contrato: ADR-0007 decisión 3 y reglas derivadas. Bases `:memory:`
//! completamente migradas (la puerta de integridad de sqlx aplica). Cubre los
//! criterios del issue: los tres estados persisten, desactivar/archivar
//! detienen sync y descargas, reactivar restaura participación sin robarla a
//! una hermana activa, y eliminar conserva pistas canónicas, fuentes y
//! archivos salvo casilla de audio explícita.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::commands::accounts::{
    ensure_account_can_sync, get_service_fallback_account_ids, perform_delete_account,
    perform_set_account_state,
};
use syncify_tauri_lib::commands::{
    perform_sync_service, ACCOUNT_STATE_ARCHIVED, ACCOUNT_STATE_DISABLED,
};

async fn setup_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones completas");
    pool
}

async fn insert_account(pool: &Pool<Sqlite>, service: &str, is_active: i64) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por las migraciones");
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email, is_active) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(service_id)
    .bind(format!("Cuenta {}", is_active))
    .bind(format!("acct{}@example.invalid", is_active))
    .bind(is_active)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

async fn insert_owned_rows(pool: &Pool<Sqlite>, account_id: i64) -> (i64, i64, i64) {
    // Pista canónica + fuente de servicio: NUNCA se borran con la cuenta.
    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Canon') RETURNING id")
            .fetch_one(pool)
            .await
            .expect("track sembrado");
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("servicio qobuz");
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, 'src-1')",
    )
    .bind(track_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("track_source sembrado");

    // Filas propiedad de la cuenta.
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (?, ?)")
        .bind(account_id)
        .bind(track_id)
        .execute(pool)
        .await
        .expect("library_entry sembrado");
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, name) VALUES (?, 'Mix') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("playlist sembrada");
    sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
         VALUES (?, ?, ?, 'pl-1')",
    )
    .bind(playlist_id)
    .bind(account_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("playlist_source sembrado");
    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title)
         VALUES (?, ?, 'track', 'fav-1', 'Canon')",
    )
    .bind(account_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("favorite sembrado");
    sqlx::query(
        "INSERT INTO account_import_counts (account_id, collection, imported_total, completeness)
         VALUES (?, 'favorites', 1, 'complete')",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .expect("import counts sembrado");
    sqlx::query(
        "INSERT INTO sync_runs (account_id, service_id, mode, scope, status)
         VALUES (?, ?, 'import_only', 'favorites', 'completed')",
    )
    .bind(account_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("sync_run sembrado");

    (track_id, service_id, playlist_id)
}

#[tokio::test]
async fn desactivar_y_reactivar_mapean_el_ciclo_de_vida_a_participacion() {
    let pool = setup_db().await;
    let account = insert_account(&pool, "qobuz", 1).await;

    // Desactivar: estado persistido y participación apagada (el toggle del
    // ADR-0006 y `disabled` son el mismo estado).
    let change = perform_set_account_state(&pool, account, ACCOUNT_STATE_DISABLED)
        .await
        .expect("desactivar");
    assert_eq!(change.old_state, "enabled");
    assert_eq!(change.new_state, "disabled");
    assert!(change.changed);
    let (state, is_active): (String, i64) =
        sqlx::query_as("SELECT state, is_active FROM accounts WHERE id = ?")
            .bind(account)
            .fetch_one(&pool)
            .await
            .expect("cuenta leída");
    assert_eq!(state, "disabled");
    assert_eq!(is_active, 0, "desactivar detiene la participación legacy");

    // Y un sync por cuenta concreta falla con motivo legible, no con
    // RequiresAuth ambiguo.
    let error = perform_sync_service(&pool, "qobuz", Some(account), None)
        .await
        .expect_err("una cuenta desactivada no sincroniza");
    assert!(
        error.starts_with("AccountDisabled:"),
        "el error nombra el gate de ciclo de vida: {}",
        error
    );
    let gate = ensure_account_can_sync(&pool, account)
        .await
        .expect_err("el gate también rechaza");
    assert!(gate.starts_with("AccountDisabled:"));

    // Reactivar: el servicio se quedó sin activa, así que recupera el rol sin
    // transición extra.
    let back = perform_set_account_state(&pool, account, "enabled")
        .await
        .expect("reactivar");
    assert_eq!(back.old_state, "disabled");
    assert!(back.is_active, "reactivar restaura la participación");
}

#[tokio::test]
async fn reactivar_no_roba_la_participacion_a_una_hermana_activa() {
    let pool = setup_db().await;
    let sister = insert_account(&pool, "qobuz", 1).await;
    let account = insert_account(&pool, "qobuz", 0).await;
    perform_set_account_state(&pool, account, ACCOUNT_STATE_DISABLED)
        .await
        .expect("desactivar la segunda");

    perform_set_account_state(&pool, account, "enabled")
        .await
        .expect("reactivar la segunda");

    let sister_active: i64 = sqlx::query_scalar("SELECT is_active FROM accounts WHERE id = ?")
        .bind(sister)
        .fetch_one(&pool)
        .await
        .expect("hermana leída");
    assert_eq!(
        sister_active, 1,
        "la hermana que seguía activa conserva el rol"
    );
}

#[tokio::test]
async fn archivar_oculta_y_desactiva_sin_perder_filas() {
    let pool = setup_db().await;
    let account = insert_account(&pool, "qobuz", 1).await;
    let _ = insert_owned_rows(&pool, account).await;

    let change = perform_set_account_state(&pool, account, ACCOUNT_STATE_ARCHIVED)
        .await
        .expect("archivar");
    assert_eq!(change.new_state, "archived");
    assert!(!change.is_active);

    let (state, is_active): (String, i64) =
        sqlx::query_as("SELECT state, is_active FROM accounts WHERE id = ?")
            .bind(account)
            .fetch_one(&pool)
            .await
            .expect("cuenta leída");
    assert_eq!(state, "archived");
    assert_eq!(is_active, 0, "archivar implica deshabilitar");

    // Archivar conserva TODO: no es un borrado.
    let entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account)
            .fetch_one(&pool)
            .await
            .expect("entradas leídas");
    assert_eq!(entries, 1, "archivar conserva la biblioteca importada");
}

#[tokio::test]
async fn un_estado_fuera_del_contrato_se_rechaza() {
    let pool = setup_db().await;
    let account = insert_account(&pool, "qobuz", 1).await;
    let error = perform_set_account_state(&pool, account, "eliminada")
        .await
        .expect_err("eliminar no es un estado");
    assert!(error.contains("InvalidAccountState"), "{}", error);
    // Transición nula: el mismo estado no reporta cambio.
    let null_change = perform_set_account_state(&pool, account, "enabled")
        .await
        .expect("transición nula");
    assert!(!null_change.changed);
}

#[tokio::test]
async fn una_cuenta_desactivada_deja_de_aparecer_en_el_fallback_de_descargas() {
    let pool = setup_db().await;
    let first = insert_account(&pool, "tidal", 1).await;
    let second = insert_account(&pool, "tidal", 0).await;

    let before = get_service_fallback_account_ids(&pool, "tidal", None)
        .await
        .expect("fallback inicial");
    assert_eq!(before.len(), 2, "dos cuentas habilitadas compiten");

    perform_set_account_state(&pool, second, ACCOUNT_STATE_DISABLED)
        .await
        .expect("desactivar la segunda");

    let after = get_service_fallback_account_ids(&pool, "tidal", None)
        .await
        .expect("fallback tras desactivar");
    assert_eq!(
        after,
        vec![first],
        "una cuenta desactivada deja de recibir descargas, también como fallback"
    );

    // Y con cuenta desactivada el gate del contrato también cierra la corrida.
    ensure_account_can_sync(&pool, second)
        .await
        .expect_err("el contrato de corridas tampoco arranca");
}

#[tokio::test]
async fn eliminar_conserva_pistas_fuentes_y_archivos_sin_casilla() {
    let pool = setup_db().await;
    let account = insert_account(&pool, "qobuz", 1).await;
    let (track_id, _, playlist_id) = insert_owned_rows(&pool, account).await;

    let dir = tempfile::tempdir().expect("directorio temporal");
    let audio_path = dir.path().join("canon.flac");
    std::fs::write(&audio_path, b"audio").expect("fichero de audio");

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("servicio qobuz");
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, account_id) VALUES (?, ?, ?, ?)",
    )
    .bind(track_id)
    .bind(service_id)
    .bind(audio_path.to_string_lossy().to_string())
    .bind(account)
    .execute(&pool)
    .await
    .expect("descarga sembrada");

    let summary = perform_delete_account(&pool, account, false)
        .await
        .expect("borrar cuenta sin casilla");
    assert_eq!(summary.service, "qobuz");
    assert!(!summary.delete_audio);
    assert_eq!(
        summary.audio_files_deleted, 0,
        "sin casilla no se borra audio"
    );

    // La fila desaparece (eliminar no es un estado).
    let account_gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE id = ?")
        .bind(account)
        .fetch_one(&pool)
        .await
        .expect("cuenta consultada");
    assert_eq!(account_gone, 0);

    // Lo compartido se conserva: pista canónica, fuente y archivo en disco.
    let track_alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .expect("track consultado");
    assert_eq!(track_alive, 1, "las pistas canónicas nunca se borran");
    let source_alive: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
            .bind(track_id)
            .fetch_one(&pool)
            .await
            .expect("fuente consultada");
    assert_eq!(source_alive, 1, "las fuentes de pista se conservan");
    assert!(audio_path.exists(), "sin casilla el audio permanece");
    let downloads_alive: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
            .bind(track_id)
            .fetch_one(&pool)
            .await
            .expect("ledger de descargas consultado");
    assert_eq!(downloads_alive, 1, "el ledger de descargas se conserva");

    // Lo propiedad de la cuenta desaparece.
    for (table, what) in [
        ("library_entries", "entradas de biblioteca"),
        ("playlists", "playlists"),
        ("playlist_sources", "fuentes de playlist"),
        ("favorites", "favoritos"),
        ("account_import_counts", "contadores"),
        ("sync_runs", "corridas"),
        ("downloads", "no espera descargas"),
    ] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {} WHERE 1=1", table))
            .fetch_one(&pool)
            .await
            .expect("tabla consultada");
        let expected: i64 = if table == "downloads" { 1 } else { 0 };
        assert_eq!(
            rows,
            expected,
            "tras borrar la cuenta, {} queda {}",
            what,
            if expected == 0 { "vacío" } else { "intacto" }
        );
    }
    let _ = playlist_id;
}

#[tokio::test]
async fn la_casilla_de_audio_borra_solo_los_ficheros_de_la_cuenta() {
    let pool = setup_db().await;
    let account = insert_account(&pool, "qobuz", 1).await;
    let survivor = insert_account(&pool, "qobuz", 0).await;
    let (track_id, service_id, _) = insert_owned_rows(&pool, account).await;

    let dir = tempfile::tempdir().expect("directorio temporal");
    let own_audio = dir.path().join("propio.flac");
    std::fs::write(&own_audio, b"audio propio").expect("fichero propio");

    // Audio de OTRA cuenta, sobre SU propia pista: la casilla nunca debe
    // alcanzarla.
    let other_audio = dir.path().join("ajeno.flac");
    std::fs::write(&other_audio, b"audio ajeno").expect("fichero ajeno");
    let other_track: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Ajeno') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("segundo track sembrado");
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, account_id) VALUES (?, ?, ?, ?)",
    )
    .bind(other_track)
    .bind(service_id)
    .bind(other_audio.to_string_lossy().to_string())
    .bind(survivor)
    .execute(&pool)
    .await
    .expect("descarga ajena sembrada");
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, account_id) VALUES (?, ?, ?, ?)",
    )
    .bind(track_id)
    .bind(service_id)
    .bind(own_audio.to_string_lossy().to_string())
    .bind(account)
    .execute(&pool)
    .await
    .expect("descarga propia sembrada");

    let summary = perform_delete_account(&pool, account, true)
        .await
        .expect("borrar con casilla");
    assert!(summary.delete_audio);
    assert_eq!(summary.audio_files_deleted, 1, "solo el audio propio");

    assert!(
        !own_audio.exists(),
        "la casilla borra el audio de la cuenta"
    );
    assert!(
        other_audio.exists(),
        "el audio de otras cuentas queda intacto"
    );
    let other_downloads: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE account_id = ?")
            .bind(survivor)
            .fetch_one(&pool)
            .await
            .expect("ledger ajeno consultado");
    assert_eq!(
        other_downloads, 1,
        "el ledger de la otra cuenta se conserva"
    );
}
