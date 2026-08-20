/**
 * TidalRepairReviewModal.spec.ts
 * Comprehensive test suite for S158: Tidal Metadata & Path Repair Review UI (Dry-Run Only)
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import TidalRepairReviewModal from '@/components/TidalRepairReviewModal.vue';
import type { DownloadRepairDryRunItem } from '@/api/types';
import { mockInvoke, resetMocks } from '../setup';

describe('TidalRepairReviewModal', () => {
  beforeEach(() => {
    resetMocks();
    vi.clearAllMocks();
  });

  const sampleRepairItem1: DownloadRepairDryRunItem = {
    download_id: 918,
    old_track_id: 19495,
    new_track_id: 50,
    old_path: '/Music/Unknown Artist/2024 - Unknown Album/01 - Tidal Track 134683067.flac',
    new_path: '/Music/UPSAHL/2020 - 12345SEX/01 - 12345SEX.flac',
    old_title: 'Tidal Track 134683067',
    new_title: '12345SEX',
    old_artist: 'Unknown Artist',
    new_artist: 'UPSAHL',
    old_album: 'Unknown Album',
    new_album: '12345SEX',
    old_hash: 'a8f9c2d1e0b3456789abcdef0123456789abcdef0123456789abcdef01234567',
    expected_hash_after: null,
    flac_operation: 'Retag Vorbis comments and atomic move',
    lrc_operation: 'Fetch synchronized LRC and save sidecar',
    cover_operation: 'Download high-res artwork to cover.jpg',
    downloads_update: 'Update track_id 19495 -> 50, file_path, metadata_completeness -> 100',
    ghost_cleanup: 'Purge ghost track 19495 and ghost album 14156',
    rollback_plan: 'Transactional SQLite rollback on tag failure',
    planned_action: 'Enrich and move to canonical library path',
    confidence: 1.0,
    provenance: 'sqlite.track_sources + tracks',
    no_redownload_confirmed: true,
  };

  const sampleRepairItem2_symbolic: DownloadRepairDryRunItem = {
    download_id: 919,
    old_track_id: 19496,
    new_track_id: 43,
    old_path: '/Music/Unknown Artist/2024 - Unknown Album/01 - Tidal Track 280721704.flac',
    new_path: '/Music/David Bowie/2016 - Blackstar/01 - Blackstar [Tidal-280721704].flac',
    old_title: 'Tidal Track 280721704',
    new_title: '★ (Blackstar)',
    old_artist: 'Unknown Artist',
    new_artist: 'David Bowie',
    old_album: 'Unknown Album',
    new_album: 'Blackstar',
    old_hash: '3f7b2c1d9e8a76543210fedcba9876543210fedcba9876543210fedcba987654',
    expected_hash_after: null,
    flac_operation: 'Retag Vorbis comments and atomic move',
    lrc_operation: 'Fetch synchronized LRC and save sidecar',
    cover_operation: 'Verify existing cover.jpg',
    downloads_update: 'Update track_id 19496 -> 43, file_path, metadata_completeness -> 100',
    ghost_cleanup: 'Purge ghost track 19496 and ghost album 14157',
    rollback_plan: 'Transactional SQLite rollback on tag failure',
    planned_action: 'Enrich and move with disambiguated filename',
    confidence: 1.0,
    provenance: 'sqlite.track_sources + tracks',
    no_redownload_confirmed: true,
  };

  it('renders dry-run repair review with all required fields and safety warning', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        return [sampleRepairItem1];
      }
      return null;
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    // 1. Title and header
    expect(wrapper.text()).toContain('Tidal Metadata & Path Repair Review');
    expect(wrapper.text()).toContain('Dry-Run Only');

    // 2. Safety Warning Banner
    expect(wrapper.text()).toContain('No files or database records will be changed.');

    // 3. IDs
    expect(wrapper.text()).toContain('Download ID: #918');
    expect(wrapper.text()).toContain('Track ID: #19495 → #50');

    // 4. Metadata diff
    expect(wrapper.text()).toContain('12345SEX');
    expect(wrapper.text()).toContain('UPSAHL');
    expect(wrapper.text()).toContain('Unknown Artist');
    expect(wrapper.text()).toContain('Unknown Album');

    // 5. Paths
    expect(wrapper.text()).toContain('/Music/Unknown Artist/2024 - Unknown Album/01 - Tidal Track 134683067.flac');
    expect(wrapper.text()).toContain('/Music/UPSAHL/2020 - 12345SEX/01 - 12345SEX.flac');

    // 6. Operations & details
    expect(wrapper.text()).toContain('Retag Vorbis comments and atomic move');
    expect(wrapper.text()).toContain('Fetch synchronized LRC and save sidecar');
    expect(wrapper.text()).toContain('Download high-res artwork to cover.jpg');
    expect(wrapper.text()).toContain('Purge ghost track 19495 and ghost album 14156');
    expect(wrapper.text()).toContain('Confidence: 100%');
    expect(wrapper.text()).toContain('sqlite.track_sources + tracks');
    expect(wrapper.text()).toContain('No-Redownload Guarantee');
  });

  it('renders symbolic title path without garbling and with deterministic clean filename', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        return [sampleRepairItem2_symbolic];
      }
      return null;
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    // Symbolic star title in target metadata
    expect(wrapper.text()).toContain('★ (Blackstar)');
    expect(wrapper.text()).toContain('David Bowie');
    expect(wrapper.text()).toContain('Blackstar');

    // Clean ASCII deterministic new path
    expect(wrapper.text()).toContain('/Music/David Bowie/2016 - Blackstar/01 - Blackstar [Tidal-280721704].flac');
  });

  it('displays SHA-256 hash before repair', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        return [sampleRepairItem1];
      }
      return null;
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    expect(wrapper.text()).toContain('Runtime Input SHA-256:');
    expect(wrapper.text()).toContain('a8f9c2d1e0b34567...');
  });

  it('strictly contains no Apply action button in DOM', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        return [sampleRepairItem1, sampleRepairItem2_symbolic];
      }
      return null;
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    const buttons = wrapper.findAll('button');
    const buttonTexts = buttons.map((b) => b.text().toLowerCase());

    // Assert that no button contains "apply" or "execute" or "mutate"
    for (const btnText of buttonTexts) {
      expect(btnText).not.toContain('apply');
      expect(btnText).not.toContain('execute');
      expect(btnText).not.toContain('mutate');
    }

    // Verify expected dry-run actions ARE present
    expect(buttonTexts.some((t) => t.includes('copy repair plan'))).toBe(true);
    expect(buttonTexts.some((t) => t.includes('export json repair plan'))).toBe(true);
    expect(buttonTexts.some((t) => t.includes('re-run dry-run'))).toBe(true);
  });

  it('exports and copies clean plan containing zero secrets', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        return [sampleRepairItem1, sampleRepairItem2_symbolic];
      }
      return null;
    });

    let clipboardText = '';
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn(async (text: string) => {
          clipboardText = text;
        }),
      },
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    const copyBtn = wrapper.findAll('button').find((b) => b.text().includes('Copy repair plan'));
    expect(copyBtn).toBeDefined();
    await copyBtn!.trigger('click');
    await flushPromises();

    expect(navigator.clipboard.writeText).toHaveBeenCalled();
    expect(clipboardText.length).toBeGreaterThan(0);

    // Parse copied plan
    const parsed = JSON.parse(clipboardText);
    expect(Array.isArray(parsed)).toBe(true);
    expect(parsed.length).toBe(2);

    // Verify ZERO secret tokens/passwords/keys in exported JSON
    const secretRegex = /(token|secret|bearer|authorization|password|api_key|credential)/i;
    expect(secretRegex.test(clipboardText)).toBe(false);
  });

  it('handles error state gracefully with retry button', async () => {
    let callCount = 0;
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        callCount++;
        if (callCount === 1) {
          throw new Error('Database locked or connection failed');
        }
        return [sampleRepairItem1];
      }
      return null;
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    expect(wrapper.text()).toContain('Failed to compute repair dry-run');
    expect(wrapper.text()).toContain('Database locked or connection failed');

    // Click retry button
    const retryBtn = wrapper.findAll('button').find((b) => b.text().includes('Retry Dry-Run'));
    expect(retryBtn).toBeDefined();
    await retryBtn!.trigger('click');
    await flushPromises();

    expect(callCount).toBe(2);
    expect(wrapper.text()).toContain('12345SEX');
    expect(wrapper.text()).not.toContain('Failed to compute repair dry-run');
  });

  it('renders multiple repairs with aggregate count and individual cards', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_tidal_repair_dry_run') {
        return [sampleRepairItem1, sampleRepairItem2_symbolic];
      }
      return null;
    });

    const wrapper = mount(TidalRepairReviewModal, {
      props: {
        modelValue: true,
      },
    });
    await flushPromises();

    expect(wrapper.text()).toContain('2 Repair Items Found');
    const cards = wrapper.findAll('.repair-item-card');
    expect(cards.length).toBe(2);

    expect(cards[0].text()).toContain('Download ID: #918');
    expect(cards[1].text()).toContain('Download ID: #919');
  });
});
