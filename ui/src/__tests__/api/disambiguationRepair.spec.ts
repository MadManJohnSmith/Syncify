import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    planDisambiguationRepair,
    executeDisambiguationRepair,
    normalizeDisambiguationRepairReport,
} from '../../api/metadata';
import type { DisambiguationRepairReport } from '../../api/types';

// Mock the Tauri invokeCommand bridge
vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri';

const rawPlanPayload = {
    dryRun: true,
    items: [
        {
            trackId: 2507,
            isrc: 'USSM12345678',
            currentAudioPath: '/music/Album/19-2000.flac',
            targetAudioPath: '/music/Album/19-2000 (Remastered).flac',
            currentLrcPath: '/music/Album/19-2000.lrc',
            targetLrcPath: '/music/Album/19-2000 (Remastered).lrc',
            sourceTitle: '19-2000 (Soulchild Remix)',
            displayTitle: '19-2000',
            fileDisambiguator: 'Remastered',
            sha256Before: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
            status: 'ready',
            baseline: {
                filePath: '/music/Album/19-2000.flac',
                inputSha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
                inputSize: 4096,
                inputModifiedAt: 1755000000,
                audioContentHash: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
                lrcPath: '/music/Album/19-2000.lrc',
                lrcSha256: '2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae',
                lrcSize: 128,
                lrcModifiedAt: 1755000001,
            },
            appliedActions: [],
            rollbackState: null,
            errors: [],
        },
    ],
    totalCandidates: 1,
    totalRenamed: 0,
    totalSkipped: 0,
    errors: [],
    appliedActions: [],
    rollbackState: null,
};

const normalizedPlan: DisambiguationRepairReport = {
    dry_run: true,
    items: [
        {
            track_id: 2507,
            isrc: 'USSM12345678',
            current_audio_path: '/music/Album/19-2000.flac',
            target_audio_path: '/music/Album/19-2000 (Remastered).flac',
            current_lrc_path: '/music/Album/19-2000.lrc',
            target_lrc_path: '/music/Album/19-2000 (Remastered).lrc',
            source_title: '19-2000 (Soulchild Remix)',
            display_title: '19-2000',
            file_disambiguator: 'Remastered',
            sha256_before: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
            status: 'ready',
            baseline: {
                file_path: '/music/Album/19-2000.flac',
                input_sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
                input_size: 4096,
                input_modified_at: 1755000000,
                audio_content_hash: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
                lrc_path: '/music/Album/19-2000.lrc',
                lrc_sha256: '2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae',
                lrc_size: 128,
                lrc_modified_at: 1755000001,
            },
            output_hashes: null,
            applied_actions: [],
            rollback_state: null,
        },
    ],
    total_candidates: 1,
    total_renamed: 0,
    total_skipped: 0,
    errors: [],
    applied_actions: [],
    rollback_state: null,
};

describe('Disambiguation Repair API (S143B/S159)', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('planDisambiguationRepair Contract & Normalization', () => {
        it('normalizes the camelCase backend dry-run payload into the snake_case contract', async () => {
            vi.mocked(invokeCommand).mockResolvedValueOnce(rawPlanPayload);

            const result = await planDisambiguationRepair();

            expect(invokeCommand).toHaveBeenCalledWith('plan_disambiguation_repair');
            expect(result).toEqual(normalizedPlan);
            expect(result.dry_run).toBe(true);
            expect(result.items[0].target_audio_path).toBe('/music/Album/19-2000 (Remastered).flac');
            expect(result.items[0].target_lrc_path).toBe('/music/Album/19-2000 (Remastered).lrc');
        });

        it('falls back to safe defaults when the backend payload is empty', () => {
            const result = normalizeDisambiguationRepairReport(null);

            expect(result.dry_run).toBe(true);
            expect(result.items).toEqual([]);
            expect(result.total_candidates).toBe(0);
            expect(result.errors).toEqual([]);
        });

        it('normalizes an execution report with per-item hashes, actions, and rollback state', () => {
            const result = normalizeDisambiguationRepairReport({
                dryRun: false,
                items: [
                    {
                        track_id: 2507,
                        current_audio_path: '/music/Album/19-2000.flac',
                        target_audio_path: '/music/Album/19-2000 (Remastered).flac',
                        status: 'repaired_success',
                        output_hashes: {
                            file_hash_before: 'abc123',
                            file_hash_after: 'abc123',
                            lrc_hash_before: 'def456',
                            lrc_hash_after: 'def456',
                        },
                        applied_actions: ['Renamed audio', 'Renamed LRC', 'SQLiteUpdated'],
                        rollback_state: null,
                        errors: [],
                    },
                ],
                total_candidates: 1,
                total_renamed: 1,
                total_skipped: 0,
                errors: [],
                applied_actions: ['Renamed audio'],
                rollback_state: null,
            });

            expect(result.dry_run).toBe(false);
            expect(result.total_renamed).toBe(1);
            expect(result.items[0].output_hashes?.file_hash_after).toBe('abc123');
            expect(result.items[0].applied_actions).toEqual(['Renamed audio', 'Renamed LRC', 'SQLiteUpdated']);
        });
    });

    describe('executeDisambiguationRepair Confirmation Guard', () => {
        it('strictly rejects execution when confirmed is false without calling IPC', async () => {
            await expect(executeDisambiguationRepair(normalizedPlan, false)).rejects.toThrow(
                /execution requires explicit confirmation/i
            );
            expect(invokeCommand).not.toHaveBeenCalled();
        });

        it('strictly rejects when the plan is missing or has no candidates', async () => {
            await expect(
                executeDisambiguationRepair(null as unknown as DisambiguationRepairReport, true)
            ).rejects.toThrow(/non-empty disambiguation repair plan is required/i);
            await expect(
                executeDisambiguationRepair({ ...normalizedPlan, items: [] }, true)
            ).rejects.toThrow(/non-empty disambiguation repair plan is required/i);
            expect(invokeCommand).not.toHaveBeenCalled();
        });

        it('sends the plan back to the backend for baseline revalidation when confirmed is true', async () => {
            vi.mocked(invokeCommand).mockResolvedValueOnce({
                ...rawPlanPayload,
                dryRun: false,
                totalRenamed: 1,
            });

            const result = await executeDisambiguationRepair(normalizedPlan, true);

            expect(invokeCommand).toHaveBeenCalledWith('execute_disambiguation_repair', {
                plan: normalizedPlan,
                confirmed: true,
            });
            expect(result.dry_run).toBe(false);
            expect(result.total_renamed).toBe(1);
        });
    });
});
