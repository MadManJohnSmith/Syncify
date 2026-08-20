import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import DevParityMatrix from '../../components/DevParityMatrix.vue';

describe('DevParityMatrix.vue', () => {
  it('renders correctly in test/dev environment with all 20 cases', () => {
    const wrapper = mount(DevParityMatrix);
    expect(wrapper.find('.dev-parity-matrix').exists()).toBe(true);

    const rows = wrapper.findAll('.case-row');
    expect(rows.length).toBe(20);

    const equivalentBadges = wrapper.findAll('.badge-equivalent');
    expect(equivalentBadges.length).toBe(18);

    const regressionBadges = wrapper.findAll('.badge-regression');
    expect(regressionBadges.length).toBe(0);
  });

  it('displays summary statistics matching 0 regressions', () => {
    const wrapper = mount(DevParityMatrix);
    const stats = wrapper.findAll('.stat-num');
    expect(stats[0].text()).toBe('20'); // Total
    expect(stats[1].text()).toBe('18'); // Equivalent
    expect(stats[2].text()).toBe('1');  // Intentional UI
    expect(stats[3].text()).toBe('1');  // Intentional CLI
    expect(stats[4].text()).toBe('0');  // Regressions
  });

  it('does not expose personal paths, tokens, or live credentials in rendered HTML', () => {
    const wrapper = mount(DevParityMatrix);
    const html = wrapper.html();

    expect(html).not.toMatch(/Bearer\s+[A-Za-z0-9-_]+/i);
    expect(html).not.toMatch(/api_key|client_secret|password/i);
    expect(html).not.toMatch(/C:\\Users\\tardis/i);
    expect(html).not.toMatch(/\/home\/tardis/i);
  });
});
