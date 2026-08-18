/**
 * DownloadPreflight.spec.ts
 * Tests for S138A: Preflight de Descargabilidad y Selección Segura de Lote
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import DownloadFavoritesModal from '@/components/DownloadFavoritesModal.vue';
import LibraryView from '@/views/LibraryView.vue';
import { preflightDownloadBatch, enqueueEligibleBatch, addBatchToQueue } from '@/api/queue';
import { mockInvoke, resetMocks } from '../setup';
import type { LibraryTrack } from '@/api/types';

describe('Download Preflight & Safe Batch Enqueuing (S138A)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    describe('API IPC Commands', () => {
        it('invokes preflight_download_batch with exact parameters', async () => {
            const invokeCalls: { cmd: string; args: any }[] = [];
            mockInvoke((cmd, args) => {
                invokeCalls.push({ cmd, args });
                if (cmd === 'preflight_download_batch') {
                    return {
                        summary: {
                            requested_total: 3,
                            eligible_total: 2,
                            ready_exact: 1,
                            ready_fallback: 1,
                            already_downloaded: 0,
                            already_queued: 0,
                            no_download_provider: 1,
                            ambiguous_source: 0,
                            rejected_quality: 0,
                            stale_source: 0,
                            requires_auth: 0,
                            network_retryable: 0,
                        },
                        tracks: [
                            {
                                track_id: 101,
                                title: 'Qobuz Track',
                                status: 'ReadyExactSource',
                                is_eligible: true,
                                resolved_service_name: 'qobuz',
                                reason: 'Direct source available',
                            },
                            {
                                track_id: 102,
                                title: 'ISRC Fallback Track',
                                status: 'ReadyFallbackExactIdentity',
                                is_eligible: true,
                                resolved_service_name: 'tidal',
                                reason: 'Resolved fallback via exact ISRC',
                            },
                            {
                                track_id: 103,
                                title: 'Spotify Only Track',
                                status: 'NoDownloadProvider',
                                is_eligible: false,
                                reason: 'Spotify tracks cannot be downloaded directly',
                            },
                        ],
                        estimated_size_mb: 70.0,
                    };
                }
                return null;
            });

            const res = await preflightDownloadBatch({
                trackIds: [101, 102, 103],
                qualityPreference: 'hires',
                strictQuality: true,
                allowFallback: true,
            });

            expect(invokeCalls.length).toBe(1);
            expect(invokeCalls[0].cmd).toBe('preflight_download_batch');
            expect(invokeCalls[0].args).toEqual({
                trackIds: [101, 102, 103],
                serviceName: undefined,
                qualityPreference: 'hires',
                strictQuality: true,
                allowFallback: true,
            });

            expect(res.summary.requested_total).toBe(3);
            expect(res.summary.eligible_total).toBe(2);
            expect(res.summary.ready_exact).toBe(1);
            expect(res.summary.ready_fallback).toBe(1);
            expect(res.summary.no_download_provider).toBe(1);
            expect(res.tracks[2].status).toBe('NoDownloadProvider');
        });

        it('invokes enqueue_eligible_batch to enqueue only eligible tracks', async () => {
            const invokeCalls: { cmd: string; args: any }[] = [];
            mockInvoke((cmd, args) => {
                invokeCalls.push({ cmd, args });
                if (cmd === 'enqueue_eligible_batch') {
                    return {
                        submitted: 3,
                        added: 2,
                        enqueued: 2,
                        deduplicated: 0,
                        skipped: 1,
                        summary: {
                            requested_total: 3,
                            eligible_total: 2,
                            ready_exact: 1,
                            ready_fallback: 1,
                            already_downloaded: 0,
                            already_queued: 0,
                            no_download_provider: 1,
                            ambiguous_source: 0,
                            rejected_quality: 0,
                            stale_source: 0,
                            requires_auth: 0,
                            network_retryable: 0,
                        },
                        tracks: [
                            { track_id: 1, title: 'T1', status: 'ReadyExactSource', is_eligible: true },
                            { track_id: 2, title: 'T2', status: 'ReadyFallbackExactIdentity', is_eligible: true },
                            { track_id: 3, title: 'T3', status: 'NoDownloadProvider', is_eligible: false },
                        ],
                    };
                }
                return null;
            });

            const res = await enqueueEligibleBatch({
                trackIds: [1, 2, 3],
                qualityPreference: 'HI_RES_LOSSLESS',
                allowFallback: true,
            });

            expect(invokeCalls[0].cmd).toBe('enqueue_eligible_batch');
            expect(res.submitted).toBe(3);
            expect(res.added).toBe(2);
            expect(res.skipped).toBe(1);
        });
    });

    describe('DownloadFavoritesModal Preflight Grid', () => {
        it('renders preflight breakdown showing requested, ready exact, ready fallback and exclusions', async () => {
            mockInvoke((cmd) => {
                if (cmd === 'download_favorites') {
                    return {
                        total_candidates: 100,
                        enqueued: 60,
                        ready_exact: 40,
                        ready_fallback: 20,
                        already_downloaded: 25,
                        already_queued: 10,
                        no_download_provider: 3,
                        unresolved_sources: 3,
                        stale_sources: 1,
                        ambiguous_sources: 1,
                        is_preflight: true,
                        estimated_size_mb: 2100.0,
                        message: 'Preflight summary: 100 candidate(s) total — 60 ready to queue',
                    };
                }
                return null;
            });

            const wrapper = mount(DownloadFavoritesModal, {
                props: { modelValue: true },
            });
            await flushPromises();

            // Select 'All' limit to trigger preflight
            const allBtn = wrapper.findAll('button').find(b => b.text().includes('10k+') || b.text().includes('All'));
            if (allBtn) {
                await allBtn.trigger('click');
                await flushPromises();
            }

            const startBtn = wrapper.findAll('button').find(b => b.text().includes('Run Preflight Check') || b.text().includes('Enqueue Downloads'));
            expect(startBtn).toBeDefined();
            await startBtn!.trigger('click');
            await flushPromises();

            expect(wrapper.text()).toContain('Preflight Verification Guardrail');
            expect(wrapper.text()).toContain('Requested');
            expect(wrapper.text()).toContain('Ready Exact');
            expect(wrapper.text()).toContain('Fallback');
            expect(wrapper.text()).toContain('Downloaded');
            expect(wrapper.text()).toContain('In Queue');
            expect(wrapper.text()).toContain('No Provider');
        });
    });
});
