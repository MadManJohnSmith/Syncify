//! S195-fix: pruebas del lock de escritor por servicio+cuenta.
//!
//! Contrato (requisito del propietario 2026-08-25):
//! - Dos syncs del MISMO servicio+cuenta NO pueden coexistir (multi-proceso vía flock).
//! - Servicios DISTINTOS SÍ sincronizan en paralelo.

use syncify_tauri_lib::db::SyncWriterLock;

async fn temp_pool(tag: &str) -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join(format!("{}.db", tag));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .expect("pool");
    sqlx::query("CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    (pool, dir)
}

#[tokio::test]
async fn mismo_servicio_y_cuenta_queda_bloqueado() {
    let (pool, _dir) = temp_pool("lock_same").await;
    let _g1 = SyncWriterLock::acquire(&pool, "qobuz", 9).await.expect("primer lock");
    let err = match SyncWriterLock::acquire(&pool, "qobuz", 9).await {
        Ok(_) => panic!("el segundo lock del mismo servicio+cuenta debe fallar"),
        Err(m) => m,
    };
    let msg = err;
    assert!(msg.contains("Ya existe una sincronización"), "mensaje accionable, obtuve: {}", msg);
}

#[tokio::test]
async fn servicios_distintos_en_paralelo_permitidos() {
    let (pool, _dir) = temp_pool("lock_diff").await;
    let _q = SyncWriterLock::acquire(&pool, "qobuz", 9).await.expect("qobuz");
    let _t = SyncWriterLock::acquire(&pool, "tidal", 4).await.expect("tidal");
    let _s = SyncWriterLock::acquire(&pool, "spotify", 8).await.expect("spotify");
    // Los tres locks viven simultáneamente: concurrencia entre servicios intacta.
}

#[tokio::test]
async fn misma_cuenta_distinto_servicio_permitido() {
    let (pool, _dir) = temp_pool("lock_account").await;
    // Cuenta compartida no colisiona si el servicio difiere (clave = servicio+cuenta).
    let _a = SyncWriterLock::acquire(&pool, "qobuz", 0).await.expect("qobuz default");
    let _b = SyncWriterLock::acquire(&pool, "tidal", 0).await.expect("tidal default");
}

#[tokio::test]
async fn lock_se_libera_al_soltar_el_guard() {
    let (pool, _dir) = temp_pool("lock_release").await;
    {
        let _g = SyncWriterLock::acquire(&pool, "deezer", 12).await.expect("primer lock");
        // drop aquí
    }
    let g2 = SyncWriterLock::acquire(&pool, "deezer", 12)
        .await
        .expect("tras soltar el guard debe poder readquirirse");
    drop(g2);
}
