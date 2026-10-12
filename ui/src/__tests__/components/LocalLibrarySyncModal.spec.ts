/**
 * Unit tests for LocalLibrarySyncModal.vue (F-01 / F-02)
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import LocalLibrarySyncModal from '../../components/LocalLibrarySyncModal.vue';
import { mockInvoke, resetMocks } from '../setup';
import { open } from '@tauri-apps/plugin-dialog';

describe('LocalLibrarySyncModal', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('renders when modelValue is true', () => {
        const wrapper = mount(LocalLibrarySyncModal, {
            props: { modelValue: true }
        });
        expect(wrapper.find('.fixed').exists()).toBe(true);
        expect(wrapper.text()).toContain('Local Library Sync');
        expect(wrapper.text()).toContain('Music Folder Path');
    });

    it('does not render when modelValue is false', () => {
        const wrapper = mount(LocalLibrarySyncModal, {
            props: { modelValue: false }
        });
        expect(wrapper.find('.fixed').exists()).toBe(false);
    });

    it('emits update:modelValue on cancel button click', async () => {
        const wrapper = mount(LocalLibrarySyncModal, {
            props: { modelValue: true }
        });
        const cancelBtn = wrapper.findAll('button').find(b => b.text().includes('Cancel'));
        expect(cancelBtn).toBeDefined();
        await cancelBtn!.trigger('click');
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([false]);
    });

    it('lets the user choose a local folder through the native dialog', async () => {
        vi.mocked(open).mockResolvedValueOnce('/tmp/syncify-music');
        const wrapper = mount(LocalLibrarySyncModal, { props: { modelValue: true } });
        await wrapper.find('[data-testid="local-folder-picker"]').trigger('click');
        await flushPromises();
        expect(open).toHaveBeenCalledWith({
            directory: true,
            multiple: false,
            title: 'Select Music Library Folder'
        });
        expect((wrapper.find('input[type="text"]').element as HTMLInputElement).value)
            .toBe('/tmp/syncify-music');
    });

    it('performs sync flow and displays report', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'sync_local_library_cmd') {
                return {
                    success: true,
                    directory: '/home/user/Music',
                    total_scanned: 120,
                    imported: 15,
                    duplicates_matched: 15,
                    duplicates_skipped: 0,
                    errors: []
                };
            }
            if (cmd === 'auto_resolve_duplicates') {
                return { total_clusters_scanned: 1, tracks_merged: 0 };
            }
            return null;
        });

        const wrapper = mount(LocalLibrarySyncModal, {
            props: { modelValue: true }
        });

        const input = wrapper.find('input[type="text"]');
        await input.setValue('/home/user/Music');

        const syncBtn = wrapper.findAll('button').find(b => b.text().includes('Start Sync'));
        expect(syncBtn).toBeDefined();
        await syncBtn!.trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('Sync Report');
        expect(wrapper.text()).toContain('120');
        expect(wrapper.text()).toContain('15');
        expect(wrapper.emitted('sync-completed')).toBeDefined();
    });
});
