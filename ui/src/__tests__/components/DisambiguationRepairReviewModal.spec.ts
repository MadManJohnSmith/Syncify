/**
 * DisambiguationRepairReviewModal.spec.ts
 * S143B/S159: dry-run plan review and confirmed apply for retroactive track version
 * disambiguation repair (audio + LRC sidecar renames with SHA-256 rollback guardrails).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { confirm } from '@tauri-apps/plugin-dialog';
import DisambiguationRepairReviewModal from '@/components/DisambiguationRepairReviewModal.vue';
import { useToast } from '@/composables/useToast';
import { mockInvoke, resetMocks } from '../setup';

describe('DisambiguationRepairReviewModal', () => {
  beforeEach(() => {
    resetMocks();
    vi.clearAllMocks();
    const { toasts, dismiss, clearAllHistory } = useToast();
    for (const active of [...toasts.value]) dismiss(active.id);
    clearAllHistory();
  });

  // Wire payload produced by `plan_disambiguation_repair` (snake_case, like the rest of the IPC contract).
  const dryRunPlanPayload = {
    dry_run: true,
    items: [
      {
        track_id: 2507,
        isrc: 'USSM12345678',
        current_audio_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000.flac',
        target_audio_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000 (Remastered).flac',
        current_lrc_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000.lrc',
        target_lrc_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000 (Remastered).lrc',
        source_title: '19-2000 (Soulchild Remix)',
        display_title: '19-2000',
        file_disambiguator: 'Remastered',
        sha256_before: 'abc123hash',
        status: 'ready',
        baseline: {
          file_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000.flac',
          input_sha256: 'abc123hash',
          input_size: 4096,
          input_modified_at: 1755000000,
        },
        applied_actions: [],
        rollback_state: null,
        errors: [],
      },
    ],
    total_candidates: 1,
    total_renamed: 0,
    total_skipped: 0,
    errors: [],
    applied_actions: [],
    rollback_state: null,
  };

  const executedPlanPayload = {
    ...dryRunPlanPayload,
    dry_run: false,
    total_renamed: 1,
    items: [
      {
        ...dryRunPlanPayload.items[0],
        status: 'repaired_success',
        output_hashes: {
          file_hash_before: 'abc123hash',
          file_hash_after: 'abc123hash',
        },
        applied_actions: ['Renamed audio file', 'Renamed LRC sidecar', 'SQLiteUpdated'],
      },
    ],
  };

  it('computes and renders the dry-run plan with audio and LRC rename targets', async () => {
    const commands: string[] = [];
    mockInvoke((cmd) => {
      commands.push(cmd);
      if (cmd === 'plan_disambiguation_repair') return dryRunPlanPayload;
      return null;
    });

    const wrapper = mount(DisambiguationRepairReviewModal, {
      props: { modelValue: true },
    });
    await flushPromises();

    expect(commands).toContain('plan_disambiguation_repair');
    expect(wrapper.text()).toContain('Track Version Disambiguation Repair Review');
    expect(wrapper.text()).toContain('19-2000 (Remastered).flac');
    expect(wrapper.text()).toContain('19-2000 (Remastered).lrc');
    expect(wrapper.text()).toContain('Remastered');
    expect(wrapper.text()).toContain('abc123hash');
  });

  it('renders the empty state when the library has no ambiguous tracks', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'plan_disambiguation_repair') {
        return { ...dryRunPlanPayload, items: [], total_candidates: 0 };
      }
      return null;
    });

    const wrapper = mount(DisambiguationRepairReviewModal, {
      props: { modelValue: true },
    });
    await flushPromises();

    expect(wrapper.text()).toContain('No ambiguous tracks detected');
  });

  it('aborts without calling the execute command when the confirmation dialog is dismissed', async () => {
    const commands: string[] = [];
    mockInvoke((cmd) => {
      commands.push(cmd);
      if (cmd === 'plan_disambiguation_repair') return dryRunPlanPayload;
      return null;
    });
    vi.mocked(confirm).mockResolvedValueOnce(false);

    const wrapper = mount(DisambiguationRepairReviewModal, {
      props: { modelValue: true },
    });
    await flushPromises();

    const applyButton = wrapper.findAll('button').find((b) => b.text().includes('Apply repair'));
    expect(applyButton).toBeDefined();
    await applyButton!.trigger('click');
    await flushPromises();

    expect(confirm).toHaveBeenCalled();
    expect(commands).not.toContain('execute_disambiguation_repair');
  });

  it('executes the plan with explicit confirmation and surfaces the applied result', async () => {
    let executedArgs: Record<string, unknown> | null = null;
    mockInvoke((cmd, args) => {
      if (cmd === 'plan_disambiguation_repair') return dryRunPlanPayload;
      if (cmd === 'execute_disambiguation_repair') {
        executedArgs = args ?? {};
        return executedPlanPayload;
      }
      return null;
    });
    vi.mocked(confirm).mockResolvedValueOnce(true);

    const wrapper = mount(DisambiguationRepairReviewModal, {
      props: { modelValue: true },
    });
    await flushPromises();

    const applyButton = wrapper.findAll('button').find((b) => b.text().includes('Apply repair'));
    await applyButton!.trigger('click');
    await flushPromises();

    expect(executedArgs).not.toBeNull();
    expect((executedArgs as unknown as { confirmed: boolean }).confirmed).toBe(true);

    // The plan must reach the backend in the snake_case shape it deserializes back.
    const sentPlan = (executedArgs as unknown as { plan: Record<string, unknown> }).plan;
    expect(sentPlan.total_candidates).toBe(1);
    expect(sentPlan.items).toEqual([
      expect.objectContaining({
        track_id: 2507,
        current_audio_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000.flac',
        target_audio_path: '/Music/UPSAHL/2020 - 12345SEX/19-2000 (Remastered).flac',
      }),
    ]);
    expect(Object.keys(sentPlan)).not.toContain('totalCandidates');

    const toastTitles = useToast().toasts.value.map((t) => t.title);
    expect(toastTitles.some((title) => title.includes('Repaired 1 of 1 tracks'))).toBe(true);
  });

  it('surfaces backend execution failures without claiming success', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'plan_disambiguation_repair') return dryRunPlanPayload;
      if (cmd === 'execute_disambiguation_repair') throw new Error('SQLite transaction failed');
      return null;
    });
    vi.mocked(confirm).mockResolvedValueOnce(true);

    const wrapper = mount(DisambiguationRepairReviewModal, {
      props: { modelValue: true },
    });
    await flushPromises();

    const applyButton = wrapper.findAll('button').find((b) => b.text().includes('Apply repair'));
    await applyButton!.trigger('click');
    await flushPromises();

    const toastTitles = useToast().toasts.value.map((t) => t.title);
    expect(toastTitles).toContain('Disambiguation repair failed');
    expect(toastTitles.some((title) => title.includes('Repaired'))).toBe(false);
  });
});
