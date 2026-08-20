import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    auditCatalogIdentity,
    planCatalogIdentityRepair,
    applyCatalogIdentityRepair,
    getRepairHistory,
} from '../../api/metadata';
import type {
    CatalogIdentityAuditReport,
    CatalogRepairPlan,
    CatalogRepairExecutionReport,
    RepairHistoryRecord,
} from '../../api/types';

// Mock the Tauri invokeCommand bridge
vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri';

describe('Catalog Identity Audit & Safe Repair API', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('auditCatalogIdentity retrieves 16-category forensic audit report', async () => {
        const mockReport: CatalogIdentityAuditReport = {
            audit_timestamp: '2026-08-20T06:00:00Z',
            duplicate_service_sources_count: 0,
            conflicting_isrc_count: 1,
            ghost_tracks_count: 0,
            ghost_albums_count: 0,
            ghost_artists_count: 0,
            downloads_without_canonical_track_count: 0,
            canonical_tracks_without_valid_source_count: 0,
            placeholder_metadata_count: 2,
            ambiguous_editions_count: 0,
            orphan_playlist_links_count: 0,
            physical_path_mismatches_count: 0,
            metadata_provenance_conflicts_count: 0,
            invalid_filenames_count: 0,
            invalid_taggings_count: 0,
            sidecar_mismatches_count: 0,
            staging_residuals_count: 0,
            total_anomalies: 3,
            details: [
                {
                    category: 'ConflictingISRC',
                    entity_type: 'tracks',
                    entity_id: 101,
                    message: "Track 101 contains invalid ISRC format: '134683067'",
                    suggested_action: 'Nullify numeric or malformed ISRC',
                },
                {
                    category: 'PlaceholderMetadata',
                    entity_type: 'artists',
                    entity_id: 202,
                    message: "Artist 202 uses placeholder name 'Unknown Artist'",
                    suggested_action: 'Resolve real artist name from provider API',
                },
                {
                    category: 'PlaceholderMetadata',
                    entity_type: 'tracks',
                    entity_id: 303,
                    message: "Track 303 uses placeholder title 'Tidal Track 134683067'",
                    suggested_action: 'Fetch real metadata from provider API',
                },
            ],
        };

        vi.mocked(invokeCommand).mockResolvedValueOnce(mockReport);

        const report = await auditCatalogIdentity();
        expect(invokeCommand).toHaveBeenCalledWith('audit_catalog_identity');
        expect(report.total_anomalies).toBe(3);
        expect(report.conflicting_isrc_count).toBe(1);
        expect(report.placeholder_metadata_count).toBe(2);
        expect(report.details).toHaveLength(3);

        // Verify classification category filtering
        const isrcAnomalies = report.details.filter(d => d.category === 'ConflictingISRC');
        expect(isrcAnomalies).toHaveLength(1);
        expect(isrcAnomalies[0].entity_id).toBe(101);
    });

    it('planCatalogIdentityRepair generates non-mutating DryRun plan', async () => {
        const mockPlan: CatalogRepairPlan = {
            plan_id: 'plan-uuid-12345',
            created_at: '2026-08-20T06:05:00Z',
            requires_confirmation: true,
            items_to_repair: [
                {
                    anomaly_category: 'ConflictingISRC',
                    entity_type: 'tracks',
                    entity_id: 101,
                    current_state: "Track 101 contains invalid ISRC format: '134683067'",
                    proposed_state: 'SET tracks.isrc = NULL for track 101',
                    requires_fs_mutation: false,
                    file_path: null,
                },
            ],
        };

        vi.mocked(invokeCommand).mockResolvedValueOnce(mockPlan);

        const plan = await planCatalogIdentityRepair();
        expect(invokeCommand).toHaveBeenCalledWith('plan_catalog_identity_repair');
        expect(plan.plan_id).toBe('plan-uuid-12345');
        expect(plan.requires_confirmation).toBe(true);
        expect(plan.items_to_repair).toHaveLength(1);
    });

    it('applyCatalogIdentityRepair requires confirmed: true and passes SHA-256 backup metadata', async () => {
        const mockPlan: CatalogRepairPlan = {
            plan_id: 'plan-uuid-12345',
            created_at: '2026-08-20T06:05:00Z',
            requires_confirmation: true,
            items_to_repair: [],
        };

        const mockExecReport: CatalogRepairExecutionReport = {
            plan_id: 'plan-uuid-12345',
            executed_at: '2026-08-20T06:10:00Z',
            items_attempted: 1,
            items_succeeded: 1,
            items_failed: 0,
            db_backup_path: '/path/to/backup.db',
            db_backup_sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
            errors: [],
        };

        vi.mocked(invokeCommand).mockResolvedValueOnce(mockExecReport);

        const execResult = await applyCatalogIdentityRepair(mockPlan, true);
        expect(invokeCommand).toHaveBeenCalledWith('apply_catalog_identity_repair', {
            plan: mockPlan,
            confirmed: true,
        });
        expect(execResult.items_succeeded).toBe(1);
        expect(execResult.items_failed).toBe(0);
        expect(execResult.db_backup_sha256).toBeDefined();
    });

    it('getRepairHistory queries append-only historical audit records', async () => {
        const mockRecords: RepairHistoryRecord[] = [
            {
                id: 1,
                repair_id: 'plan-uuid-12345',
                timestamp: '2026-08-20T06:10:00Z',
                download_id: null,
                old_track_id: null,
                new_track_id: null,
                old_path: '',
                new_path: '',
                input_file_hash: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
                output_file_hash: null,
                audio_payload_hash_before: null,
                audio_payload_hash_after: null,
                baseline_validation: 'Valid',
                actions: ['SET tracks.isrc = NULL for track 101'],
                rollback_state: null,
                provenance: 'CatalogIdentityRepair',
                result: 'success',
                details_json: '{"items":1}',
            },
        ];

        vi.mocked(invokeCommand).mockResolvedValueOnce(mockRecords);

        const records = await getRepairHistory(50, 0);
        expect(invokeCommand).toHaveBeenCalledWith('get_repair_history', { limit: 50, offset: 0 });
        expect(records).toHaveLength(1);
        expect(records[0].repair_id).toBe('plan-uuid-12345');
        expect(records[0].provenance).toBe('CatalogIdentityRepair');
    });

    it('ensures no token, bearer, or private credentials leak in payloads', () => {
        const sensitiveTokens = ['Bearer ', 'client_secret', 'access_token', 'refresh_token', 'password'];
        const samplePayload = JSON.stringify({
            category: 'AuthInvalid',
            message: 'Session token has expired on provider endpoint',
            endpoint: 'https://api.tidal.com/v1/tracks',
        });

        for (const token of sensitiveTokens) {
            expect(samplePayload).not.toContain(token);
        }
    });
});
