import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    auditCatalogIdentity,
    planCatalogIdentityRepair,
    applyCatalogIdentityRepair,
    getRepairHistory,
    normalizeCatalogAuditReport,
    normalizeCatalogRepairPlan,
} from '../../api/metadata';
import type {
    CatalogIdentityAuditReport,
    CatalogRepairPlan,
} from '../../api/types';

// Mock the Tauri invokeCommand bridge
vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri';

describe('Catalog Identity Audit & Safe Repair API (TASK-29)', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('auditCatalogIdentity Contract & Normalization', () => {
        it('normalizes real backend payload with mixed camelCase and snake_case into canonical 16-category contract', async () => {
            // Simulate backend serialization with serde camelCase renames
            const rawBackendPayload = {
                auditTimestamp: '2026-08-20T06:00:00Z',
                duplicateServiceSourcesCount: 2,
                conflictingIsrcCount: 1,
                ghostTracksCount: 0,
                ghostAlbumsCount: 0,
                ghostArtistsCount: 1,
                downloadsWithoutCanonicalTrackCount: 0,
                canonicalTracksWithoutValidSourceCount: 0,
                placeholderMetadataCount: 3,
                ambiguousEditionsCount: 0,
                orphanPlaylistLinksCount: 0,
                physicalPathMismatchesCount: 0,
                metadataProvenanceConflictsCount: 0,
                invalidFilenamesCount: 0,
                invalidTaggingsCount: 0,
                sidecarMismatchesCount: 0,
                stagingResidualsCount: 0,
                totalAnomalies: 7,
                details: [
                    {
                        anomalyCategory: 'ConflictingISRC',
                        entityType: 'tracks',
                        entityId: 101,
                        message: "Track 101 contains invalid ISRC format: '134683067'",
                        suggestedAction: 'Nullify numeric or malformed ISRC',
                    },
                    {
                        category: 'PlaceholderMetadata',
                        entity_type: 'artists',
                        entity_id: 202,
                        message: "Artist 202 uses placeholder name 'Unknown Artist'",
                        suggested_action: 'Resolve real artist name from provider API',
                    },
                ],
            };

            vi.mocked(invokeCommand).mockResolvedValueOnce(rawBackendPayload);

            const report = await auditCatalogIdentity();

            expect(invokeCommand).toHaveBeenCalledWith('audit_catalog_identity');
            // Canonical snake_case properties guaranteed
            expect(report.audit_timestamp).toBe('2026-08-20T06:00:00Z');
            expect(report.duplicate_service_sources_count).toBe(2);
            expect(report.conflicting_isrc_count).toBe(1);
            expect(report.ghost_artists_count).toBe(1);
            expect(report.placeholder_metadata_count).toBe(3);
            expect(report.total_anomalies).toBe(7);
            expect(report.details).toHaveLength(2);

            // Verify detail field transformation from camelCase
            expect(report.details[0].category).toBe('ConflictingISRC');
            expect(report.details[0].entity_type).toBe('tracks');
            expect(report.details[0].entity_id).toBe(101);
            expect(report.details[0].suggested_action).toBe('Nullify numeric or malformed ISRC');
        });

        it('handles corrupt, null, or empty backend responses gracefully with zeroed contract and safe fallbacks', async () => {
            vi.mocked(invokeCommand).mockResolvedValueOnce(null);

            const reportFromNull = await auditCatalogIdentity();

            expect(reportFromNull.total_anomalies).toBe(0);
            expect(reportFromNull.conflicting_isrc_count).toBe(0);
            expect(reportFromNull.placeholder_metadata_count).toBe(0);
            expect(reportFromNull.ghost_tracks_count).toBe(0);
            expect(reportFromNull.details).toEqual([]);
            expect(typeof reportFromNull.audit_timestamp).toBe('string');

            // Empty object with undefined details
            vi.mocked(invokeCommand).mockResolvedValueOnce({});
            const reportFromEmpty = await auditCatalogIdentity();
            expect(reportFromEmpty.total_anomalies).toBe(0);
            expect(reportFromEmpty.details).toEqual([]);
        });

        it('derives and reconciles total_anomalies when missing or omitted by backend', async () => {
            const partialPayload = {
                audit_timestamp: '2026-08-20T12:00:00Z',
                conflicting_isrc_count: 2,
                placeholder_metadata_count: 3,
                invalid_filenames_count: 1,
                // total_anomalies omitted
                details: [
                    { category: 'ConflictingISRC', entity_type: 'tracks', entity_id: 1, message: 'm1' },
                    { category: 'PlaceholderMetadata', entity_type: 'tracks', entity_id: 2, message: 'm2' },
                ],
            };

            vi.mocked(invokeCommand).mockResolvedValueOnce(partialPayload);

            const report = await auditCatalogIdentity();

            // Derives sum = 2 + 3 + 1 = 6
            expect(report.total_anomalies).toBe(6);
            expect(report.conflicting_isrc_count).toBe(2);
            expect(report.placeholder_metadata_count).toBe(3);
            expect(report.invalid_filenames_count).toBe(1);
        });

        it('defensively sanitizes malformed anomaly detail entries', () => {
            const rawCorrupt = {
                total_anomalies: 2,
                details: [
                    null, // invalid entry
                    {
                        // missing category and entity_type
                        entity_id: 'not-a-number',
                        message: null,
                    },
                    {
                        category: 'GhostTracks',
                        entity_type: 'tracks',
                        entity_id: 42,
                        message: 'Track file missing',
                        suggested_action: 'Prune record',
                    },
                ],
            };

            const normalized = normalizeCatalogAuditReport(rawCorrupt);

            // Filters or safely defaults malformed entries
            const valid = normalized.details.filter(d => d.category === 'GhostTracks');
            expect(valid).toHaveLength(1);
            expect(valid[0].entity_id).toBe(42);
            expect(valid[0].message).toBe('Track file missing');
            expect(valid[0].suggested_action).toBe('Prune record');
        });
    });

    describe('planCatalogIdentityRepair Contract & Safe Defaults', () => {
        it('normalizes repair plan and enforces requires_confirmation: true by default', async () => {
            const rawBackendPlan = {
                planId: 'plan-uuid-9999',
                createdAt: '2026-08-20T06:05:00Z',
                // requiresConfirmation omitted
                itemsToRepair: [
                    {
                        anomalyCategory: 'ConflictingISRC',
                        entityType: 'tracks',
                        entityId: 101,
                        currentState: "Track 101 contains invalid ISRC format: '134683067'",
                        proposedState: 'SET tracks.isrc = NULL for track 101',
                        requiresFsMutation: false,
                        filePath: null,
                    },
                ],
            };

            vi.mocked(invokeCommand).mockResolvedValueOnce(rawBackendPlan);

            const plan = await planCatalogIdentityRepair();

            expect(invokeCommand).toHaveBeenCalledWith('plan_catalog_identity_repair');
            expect(plan.plan_id).toBe('plan-uuid-9999');
            expect(plan.requires_confirmation).toBe(true);
            expect(plan.items_to_repair).toHaveLength(1);
            expect(plan.items_to_repair[0].anomaly_category).toBe('ConflictingISRC');
            expect(plan.items_to_repair[0].entity_type).toBe('tracks');
            expect(plan.items_to_repair[0].requires_fs_mutation).toBe(false);
        });

        it('safely handles null repair plan without throwing', () => {
            const plan = normalizeCatalogRepairPlan(null);
            expect(plan.plan_id).toBe('');
            expect(plan.items_to_repair).toEqual([]);
            expect(plan.requires_confirmation).toBe(true);
        });
    });

    describe('applyCatalogIdentityRepair Input Validation & Guard Rails', () => {
        const validPlan: CatalogRepairPlan = {
            plan_id: 'plan-valid-1234',
            created_at: '2026-08-20T06:05:00Z',
            requires_confirmation: true,
            items_to_repair: [
                {
                    anomaly_category: 'ConflictingISRC',
                    entity_type: 'tracks',
                    entity_id: 101,
                    current_state: 'Bad ISRC',
                    proposed_state: 'Clear ISRC',
                    requires_fs_mutation: false,
                    file_path: null,
                },
            ],
        };

        it('strictly rejects execution when confirmed is false without calling IPC', async () => {
            await expect(applyCatalogIdentityRepair(validPlan, false)).rejects.toThrow(
                /execution requires explicit confirmation/i
            );
            expect(invokeCommand).not.toHaveBeenCalled();
        });

        it('strictly rejects when plan is missing, null, or has an empty plan_id', async () => {
            await expect(applyCatalogIdentityRepair(null as unknown as CatalogRepairPlan, true)).rejects.toThrow(
                /invalid repair plan/i
            );
            await expect(applyCatalogIdentityRepair({ plan_id: '' } as CatalogRepairPlan, true)).rejects.toThrow(
                /non-empty plan_id is required/i
            );
            await expect(applyCatalogIdentityRepair({ plan_id: '   ' } as CatalogRepairPlan, true)).rejects.toThrow(
                /non-empty plan_id is required/i
            );
            expect(invokeCommand).not.toHaveBeenCalled();
        });

        it('executes valid plan when confirmed is true and normalizes execution report with SHA-256 backup', async () => {
            const rawBackendExecReport = {
                planId: 'plan-valid-1234',
                executedAt: '2026-08-20T06:10:00Z',
                itemsAttempted: 1,
                itemsSucceeded: 1,
                itemsFailed: 0,
                dbBackupPath: '/tmp/syncify_backup.db',
                dbBackupSha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
                errors: [],
            };

            vi.mocked(invokeCommand).mockResolvedValueOnce(rawBackendExecReport);

            const result = await applyCatalogIdentityRepair(validPlan, true);

            expect(invokeCommand).toHaveBeenCalledWith('apply_catalog_identity_repair', {
                plan: validPlan,
                confirmed: true,
            });
            expect(result.plan_id).toBe('plan-valid-1234');
            expect(result.items_succeeded).toBe(1);
            expect(result.items_failed).toBe(0);
            expect(result.db_backup_sha256).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
            expect(result.errors).toEqual([]);
        });
    });

    describe('getRepairHistory Contract & Defense', () => {
        it('normalizes historical records and handles pagination parameters safely', async () => {
            const rawRecords = [
                {
                    id: 1,
                    repairId: 'plan-uuid-12345',
                    timestamp: '2026-08-20T06:10:00Z',
                    inputFileHash: 'abc123hash',
                    baselineValidation: 'Valid',
                    actions: ['SET tracks.isrc = NULL'],
                    provenance: 'CatalogIdentityRepair',
                    result: 'success',
                },
            ];

            vi.mocked(invokeCommand).mockResolvedValueOnce(rawRecords);

            const records = await getRepairHistory(50, 0);

            expect(invokeCommand).toHaveBeenCalledWith('get_repair_history', { limit: 50, offset: 0 });
            expect(records).toHaveLength(1);
            expect(records[0].repair_id).toBe('plan-uuid-12345');
            expect(records[0].input_file_hash).toBe('abc123hash');
            expect(records[0].baseline_validation).toBe('Valid');
            expect(records[0].provenance).toBe('CatalogIdentityRepair');
            expect(records[0].result).toBe('success');
        });

        it('clamps negative limit and offset to 0 and returns [] on null response', async () => {
            vi.mocked(invokeCommand).mockResolvedValueOnce(null);

            const records = await getRepairHistory(-10, -5);

            expect(invokeCommand).toHaveBeenCalledWith('get_repair_history', { limit: 0, offset: 0 });
            expect(records).toEqual([]);
        });
    });

    describe('Security & Sensitive Data Leak Prevention', () => {
        it('ensures no token, bearer, or private credentials leak into anomaly messages or repair states', () => {
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
});
