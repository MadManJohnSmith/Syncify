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

  const dryRunPlanPayload = {
    dryRun: true,
    items: [
      {
        trackId: 2507,
        isrc: 'USSM12345678',
        currentAudioPath: '/Music/UPSAHL/2020 - 12345SEX/19-2000.flac',
        targetAudioPath: '/Music/UPSAHL/2020 - 12345SEX/19-2000 (Remastered).flac',
        currentLrcPath: '/Music/UPSAHL/2020 - 12345SEX/19-2000.lrc',
        targetLrcPath: '/Music/UPSAHL/2020 - 12345SEX/19-2000 (Remastered).lrc',
        sourceTitle: '19-2000 (Soulchild Remix)',
        displayTitle: '19-2000',
        fileDisambiguator: 'Remastered',
        sha256Before: 'abc123hash',
        status: 'ready',
        baseline: {
          filePath: '/Music/UPSAHL/2020 - 12345SEX/19-2000.flac',
          inputSha256: 'abc123hash',
          inputSize: 4096,
          inputModifiedAt: 1755000000,
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

  const executedPlanPayload = {
    ...dryRunPlanPayload,
    dryRun: false,
    totalRenamed: 1,
    items: [
      {
        ...dryRunPlanPayload.items[0],
        status: 'repaired_success',
        outputHashes: {
          fileHashBefore: 'abc123hash',
          fileHashAfter: 'abc123hash',
        },
        appliedActions: ['Renamed audio file', 'Renamed LRC sidecar', 'SQLiteUpdated'],
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
        return { ...dryRunPlanPayload, items: [], totalCandidates: 0 };
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
