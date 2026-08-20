/**
 * RepairHistoryModal.spec.ts
 * Comprehensive unit tests for S163: Applied Repairs History Modal UI (Append-Only Audit Trail)
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import RepairHistoryModal from '@/components/RepairHistoryModal.vue';
import type { RepairHistoryRecord } from '@/api/types';
import { mockInvoke, resetMocks } from '../setup';

describe('RepairHistoryModal', () => {
  beforeEach(() => {
    resetMocks();
    vi.clearAllMocks();
  });

  const sampleRecordSuccess: RepairHistoryRecord = {
    id: 1,
    repair_id: 'rep_dl_918_1724123456000',
    timestamp: '2026-08-20 03:00:00',
    download_id: 918,
    old_track_id: 19495,
    new_track_id: 50,
    old_path: '/Music/Syncify/Unknown Artist/Unknown Album/01 - Tidal Track 134683067.flac',
    new_path: '/Music/Syncify/Radiohead/1997 - OK Computer/01 - Airbag.flac',
    input_file_hash: 'a8f9c2d1e0b3456789abcdef0123456789abcdef0123456789abcdef01234567',
    output_file_hash: 'b7e8d1c0f9a234567890abcdef01234567890abcdef01234567890abcdef0123',
    audio_payload_hash_before: 'flac_frames:11223344556677889900aabbccddeeff',
    audio_payload_hash_after: 'flac_frames:11223344556677889900aabbccddeeff',
    baseline_validation: 'valid',
    actions: [
      'validated_baseline',
      'tags_applied',
      'audio_payload_invariance_verified',
      'moved_audio',
      'database_updated',
      'ghost_cleanup: track_id 19495'
    ],
    rollback_state: null,
    provenance: 'tidal_pipeline.re_enrich',
    result: 'success',
    details_json: null,
  };

  const sampleRecordFailed: RepairHistoryRecord = {
    id: 2,
    repair_id: 'rep_dl_919_1724123499000',
    timestamp: '2026-08-20 03:05:00',
    download_id: 919,
    old_track_id: 19496,
    new_track_id: 43,
    old_path: '/Music/Syncify/Unknown Artist/Unknown Album/02 - Tidal Track 280721704.flac',
    new_path: '/Music/Syncify/David Bowie/2016 - Blackstar/01 - Blackstar.flac',
    input_file_hash: '3f7b2c1d9e8a76543210fedcba9876543210fedcba9876543210fedcba987654',
    output_file_hash: null,
    audio_payload_hash_before: 'flac_frames:99887766554433221100ffeeddccbbaa',
    audio_payload_hash_after: null,
    baseline_validation: 'repair_input_changed',
    actions: ['validated_baseline'],
    rollback_state: 'AbortedWithoutMutation: Baseline validation failed',
    provenance: 'tidal_pipeline.re_enrich',
    result: 'failed',
    details_json: null,
  };

  it('renders loading state initially on open', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_repair_history') {
        return new Promise(() => {}); // never resolves
      }
      return null;
    });

    const wrapper = mount(RepairHistoryModal, {
      props: {
        modelValue: true,
      },
    });

    expect(wrapper.text()).toContain('Loading repair history...');
  });

  it('renders error state on command failure and allows retry', async () => {
    let callCount = 0;
    mockInvoke((cmd) => {
      if (cmd === 'get_repair_history') {
        callCount++;
        if (callCount === 1) {
          throw new Error('Database locked');
        }
        return [sampleRecordSuccess];
      }
      return null;
    });

    const wrapper = mount(RepairHistoryModal, {
      props: {
        modelValue: true,
      },
    });

    await flushPromises();

    expect(wrapper.text()).toContain('Failed to load repair history');
    expect(wrapper.text()).toContain('Database locked');

    // Click retry
    const retryBtn = wrapper.find('button.bg-red-600');
    expect(retryBtn.exists()).toBe(true);
    await retryBtn.trigger('click');
    await flushPromises();

    expect(wrapper.text()).toContain('rep_dl_918_1724123456000');
    expect(wrapper.text().toLowerCase()).toContain('success');
  });

  it('renders empty state when no repair records exist', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_repair_history') {
        return [];
      }
      return null;
    });

    const wrapper = mount(RepairHistoryModal, {
      props: {
        modelValue: true,
      },
    });

    await flushPromises();

    expect(wrapper.text()).toContain('No Applied Repairs Found');
    expect(wrapper.text()).toContain('No repairs have been executed yet');
  });

  it('renders audit history cards with full hashes, invariance badge, and actions', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_repair_history') {
        return [sampleRecordSuccess, sampleRecordFailed];
      }
      return null;
    });

    const wrapper = mount(RepairHistoryModal, {
      props: {
        modelValue: true,
      },
    });

    await flushPromises();

    expect(wrapper.text()).toContain('Showing 2 audit records');
    expect(wrapper.text()).toContain('rep_dl_918_1724123456000');
    expect(wrapper.text()).toContain('rep_dl_919_1724123499000');
    expect(wrapper.text()).toContain('Download #918');
    expect(wrapper.text()).toContain('Download #919');
    expect(wrapper.text()).toContain('Invariant');
    expect(wrapper.text()).toContain('tags_applied');
    expect(wrapper.text()).toContain('database_updated');
    expect(wrapper.text()).toContain('Rollback Event:');
    expect(wrapper.text()).toContain('AbortedWithoutMutation');
  });

  it('filters records by search query and result dropdown', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_repair_history') {
        return [sampleRecordSuccess, sampleRecordFailed];
      }
      return null;
    });

    const wrapper = mount(RepairHistoryModal, {
      props: {
        modelValue: true,
      },
    });

    await flushPromises();

    // Filter by Result: success
    const select = wrapper.find('select');
    await select.setValue('success');

    expect(wrapper.text()).toContain('rep_dl_918_1724123456000');
    expect(wrapper.text()).not.toContain('rep_dl_919_1724123499000');

    // Filter by Result: failed
    await select.setValue('failed');
    expect(wrapper.text()).not.toContain('rep_dl_918_1724123456000');
    expect(wrapper.text()).toContain('rep_dl_919_1724123499000');

    // Reset filter and search by query
    await select.setValue('all');
    const searchInput = wrapper.find('input[type="text"]');
    await searchInput.setValue('Airbag');
    expect(wrapper.text()).toContain('rep_dl_918_1724123456000');
    expect(wrapper.text()).not.toContain('rep_dl_919_1724123499000');
  });

  it('emits update:modelValue false on close button click', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_repair_history') {
        return [sampleRecordSuccess];
      }
      return null;
    });

    const wrapper = mount(RepairHistoryModal, {
      props: {
        modelValue: true,
      },
    });

    await flushPromises();

    const closeBtn = wrapper.find('button[title="Close modal"]');
    await closeBtn.trigger('click');

    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([false]);
  });
});
