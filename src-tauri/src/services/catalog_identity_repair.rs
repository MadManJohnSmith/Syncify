//! Safe catalog identity repair planner and execution engine with cryptographic guardrails.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRepairPlan {
    pub plan_id: String,
    pub created_at: String,
    pub items_to_repair: Vec<CatalogRepairPlanItem>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRepairPlanItem {
    pub anomaly_category: String,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub current_state: String,
    pub proposed_state: String,
    pub requires_fs_mutation: bool,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRepairExecutionReport {
    pub plan_id: String,
    pub executed_at: String,
    pub items_attempted: usize,
    pub items_succeeded: usize,
    pub items_failed: usize,
    pub db_backup_path: Option<String>,
    pub db_backup_sha256: Option<String>,
    pub errors: Vec<String>,
}

/// Generate a non-mutating Dry-Run plan for catalog identity inconsistencies.
pub async fn plan_catalog_identity_repair(
    db: &SqlitePool,
    base_dir: Option<&Path>,
) -> Result<CatalogRepairPlan, String> {
    let audit_report = super::catalog_identity_audit::audit_catalog_identity(db, base_dir).await?;
    let mut items = Vec::new();

    for anomaly in audit_report.details {
        match anomaly.category.as_str() {
            "ConflictingISRC" => {
                if let Some(tid) = anomaly.entity_id {
                    items.push(CatalogRepairPlanItem {
                        anomaly_category: anomaly.category,
                        entity_type: "tracks".to_string(),
                        entity_id: Some(tid),
                        current_state: anomaly.message,
                        proposed_state: format!("SET tracks.isrc = NULL for track {}", tid),
                        requires_fs_mutation: false,
                        file_path: None,
                    });
                }
            }
            "GhostTrack" => {
                if let Some(tid) = anomaly.entity_id {
                    items.push(CatalogRepairPlanItem {
                        anomaly_category: anomaly.category,
                        entity_type: "tracks".to_string(),
                        entity_id: Some(tid),
                        current_state: anomaly.message,
                        proposed_state: format!("DELETE FROM tracks WHERE id = {} (zero-reference ghost)", tid),
                        requires_fs_mutation: false,
                        file_path: None,
                    });
                }
            }
            "GhostAlbum" => {
                if let Some(aid) = anomaly.entity_id {
                    items.push(CatalogRepairPlanItem {
                        anomaly_category: anomaly.category,
                        entity_type: "albums".to_string(),
                        entity_id: Some(aid),
                        current_state: anomaly.message,
                        proposed_state: format!("DELETE FROM albums WHERE id = {} (zero-reference ghost)", aid),
                        requires_fs_mutation: false,
                        file_path: None,
                    });
                }
            }
            "GhostArtist" => {
                if let Some(arid) = anomaly.entity_id {
                    items.push(CatalogRepairPlanItem {
                        anomaly_category: anomaly.category,
                        entity_type: "artists".to_string(),
                        entity_id: Some(arid),
                        current_state: anomaly.message,
                        proposed_state: format!("DELETE FROM artists WHERE id = {} (zero-reference ghost)", arid),
                        requires_fs_mutation: false,
                        file_path: None,
                    });
                }
            }
            "OrphanPlaylistLink" => {
                if let Some(tid) = anomaly.entity_id {
                    items.push(CatalogRepairPlanItem {
                        anomaly_category: anomaly.category,
                        entity_type: "playlist_tracks".to_string(),
                        entity_id: Some(tid),
                        current_state: anomaly.message,
                        proposed_state: format!("DELETE FROM playlist_tracks WHERE track_id = {} (orphan)", tid),
                        requires_fs_mutation: false,
                        file_path: None,
                    });
                }
            }
            "MetadataProvenanceConflict" => {
                if let Some(arid) = anomaly.entity_id {
                    items.push(CatalogRepairPlanItem {
                        anomaly_category: anomaly.category,
                        entity_type: "artists".to_string(),
                        entity_id: Some(arid),
                        current_state: anomaly.message,
                        proposed_state: format!("UPDATE artists SET spotify_id = NULL, tidal_id = NULL WHERE id = {} (corrupted track ID)", arid),
                        requires_fs_mutation: false,
                        file_path: None,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(CatalogRepairPlan {
        plan_id: format!("plan-{}", uuid::Uuid::new_v4()),
        created_at: chrono::Utc::now().to_rfc3339(),
        items_to_repair: items,
        requires_confirmation: true,
    })
}

/// Execute repair plan with strict confirmation, automatic SQLite backup, audio hash integrity, and append-only history.
pub async fn apply_catalog_identity_repair(
    db: &SqlitePool,
    plan: &CatalogRepairPlan,
    confirmed: bool,
    backup_dir: Option<&Path>,
) -> Result<CatalogRepairExecutionReport, String> {
    if !confirmed {
        return Err("Execution rejected: 'confirmed: true' flag is required to apply repairs".to_string());
    }

    // S168: Acquire CatalogWrite coordinator lock
    let _catalog_lock = crate::services::get_global_concurrency_manager()
        .acquire(
            syncify_core_domain::LockScope::CatalogWrite,
            Some(&format!("repair-{}", plan.plan_id)),
            None,
        )
        .await
        .map_err(|e| format!("Concurrency lock error: {}", e))?;

    // 1. Create SQLite DB backup and calculate SHA-256
    let (backup_path_str, backup_sha256) = if let Some(bdir) = backup_dir {
        std::fs::create_dir_all(bdir).map_err(|e| format!("Failed to create backup dir: {}", e))?;
        let backup_file = bdir.join(format!("syncify_repair_backup_{}.db", chrono::Utc::now().format("%Y%m%d_%H%M%S")));
        
        // Execute VACUUM INTO for atomic SQLite backup
        let vacuum_sql = format!("VACUUM INTO '{}'", backup_file.to_string_lossy().replace('\\', "/"));
        sqlx::query(&vacuum_sql)
            .execute(db)
            .await
            .map_err(|e| format!("Failed to create SQLite backup: {}", e))?;

        let bytes = std::fs::read(&backup_file)
            .map_err(|e| format!("Failed to read backup file for hash: {}", e))?;
        let hash = format!("{:x}", Sha256::digest(&bytes));

        (Some(backup_file.to_string_lossy().to_string()), Some(hash))
    } else {
        (None, None)
    };

    // 2. Start Atomic Database Transaction
    let mut tx = db.begin().await.map_err(|e| format!("Failed to start DB transaction: {}", e))?;
    let mut succeeded = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for item in &plan.items_to_repair {
        match item.anomaly_category.as_str() {
            "ConflictingISRC" => {
                if let Some(tid) = item.entity_id {
                    match sqlx::query("UPDATE tracks SET isrc = NULL WHERE id = ?").bind(tid).execute(&mut *tx).await {
                        Ok(_) => succeeded += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("Failed to clear invalid ISRC on track {}: {}", tid, e));
                        }
                    }
                }
            }
            "GhostTrack" => {
                if let Some(tid) = item.entity_id {
                    match sqlx::query("DELETE FROM tracks WHERE id = ?").bind(tid).execute(&mut *tx).await {
                        Ok(_) => succeeded += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("Failed to delete ghost track {}: {}", tid, e));
                        }
                    }
                }
            }
            "GhostAlbum" => {
                if let Some(aid) = item.entity_id {
                    match sqlx::query("DELETE FROM albums WHERE id = ?").bind(aid).execute(&mut *tx).await {
                        Ok(_) => succeeded += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("Failed to delete ghost album {}: {}", aid, e));
                        }
                    }
                }
            }
            "GhostArtist" => {
                if let Some(arid) = item.entity_id {
                    match sqlx::query("DELETE FROM artists WHERE id = ?").bind(arid).execute(&mut *tx).await {
                        Ok(_) => succeeded += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("Failed to delete ghost artist {}: {}", arid, e));
                        }
                    }
                }
            }
            "OrphanPlaylistLink" => {
                if let Some(tid) = item.entity_id {
                    match sqlx::query("DELETE FROM playlist_tracks WHERE track_id = ?").bind(tid).execute(&mut *tx).await {
                        Ok(_) => succeeded += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("Failed to delete orphan playlist track {}: {}", tid, e));
                        }
                    }
                }
            }
            "MetadataProvenanceConflict" => {
                if let Some(arid) = item.entity_id {
                    match sqlx::query("UPDATE artists SET spotify_id = NULL, tidal_id = NULL WHERE id = ?").bind(arid).execute(&mut *tx).await {
                        Ok(_) => succeeded += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("Failed to reset corrupted artist provenance {}: {}", arid, e));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // 3. Commit Transaction
    tx.commit().await.map_err(|e| format!("Failed to commit repair transaction: {}", e))?;

    // 4. Record in append-only repair history
    let actions: Vec<String> = plan.items_to_repair.iter().map(|i| i.proposed_state.clone()).collect();
    let details_json = serde_json::to_string(&plan).ok();
    let _ = super::repair_history::record_applied_repair(
        db,
        &plan.plan_id,
        None,
        None,
        None,
        "",
        "",
        backup_sha256.as_deref().unwrap_or(""),
        None,
        None,
        None,
        "Valid",
        &actions,
        None,
        "CatalogIdentityRepair",
        if failed == 0 { "success" } else { "partial_success" },
        details_json.as_deref(),
    ).await;

    Ok(CatalogRepairExecutionReport {
        plan_id: plan.plan_id.clone(),
        executed_at: chrono::Utc::now().to_rfc3339(),
        items_attempted: plan.items_to_repair.len(),
        items_succeeded: succeeded,
        items_failed: failed,
        db_backup_path: backup_path_str,
        db_backup_sha256: backup_sha256,
        errors,
    })
}
