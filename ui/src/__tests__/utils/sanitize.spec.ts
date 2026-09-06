/**
 * Unit tests for HTML sanitization, entity escaping, and XSS prevention (TASK-08)
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { escapeHtml, escapeRegex, sanitizeHtml } from '../../utils/sanitize'
import CommandPalette from '../../components/CommandPalette.vue'
import * as libraryApi from '../../api/library'

vi.mock('vue-router', () => ({
  useRouter: () => ({
    push: vi.fn(),
  }),
}))

vi.mock('../../api/library', () => ({
  searchTracks: vi.fn(),
}))

describe('sanitize utilities (TASK-08)', () => {
  describe('escapeHtml', () => {
    it('neutralizes malicious XSS payloads with HTML entities', () => {
      expect(escapeHtml('<img src=x onerror=alert(1)>')).toBe(
        '&lt;img src=x onerror=alert(1)&gt;'
      )
      expect(escapeHtml("<script>alert('xss')</script>")).toBe(
        '&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;'
      )
    })

    it('escapes &, <, >, ", and \' characters properly', () => {
      expect(escapeHtml('Tom & Jerry "Special" <Edition> \'2024\'')).toBe(
        'Tom &amp; Jerry &quot;Special&quot; &lt;Edition&gt; &#39;2024&#39;'
      )
    })

    it('handles empty strings and nullish values safely', () => {
      expect(escapeHtml('')).toBe('')
      expect(escapeHtml(null as unknown as string)).toBe('')
      expect(escapeHtml(undefined as unknown as string)).toBe('')
    })
  })

  describe('escapeRegex', () => {
    it('escapes regex metacharacters properly', () => {
      expect(escapeRegex('test.*+?^${}()|[]\\')).toBe(
        'test\\.\\*\\+\\?\\^\\$\\{\\}\\(\\)\\|\\[\\]\\\\'
      )
    })

    it('handles empty strings safely', () => {
      expect(escapeRegex('')).toBe('')
    })
  })

  describe('sanitizeHtml', () => {
    it('removes executable tags and blocks (<script>, <iframe>, <object>, etc.)', () => {
      const malicious = '<p>Normal text</p><script>alert("xss")</script><iframe src="//evil.com"></iframe>'
      const cleaned = sanitizeHtml(malicious)
      expect(cleaned).not.toContain('<script')
      expect(cleaned).not.toContain('alert("xss")')
      expect(cleaned).not.toContain('<iframe')
      expect(cleaned).toContain('<p>Normal text</p>')
    })

    it('removes inline event handlers like onerror and onload', () => {
      const malicious = '<img src="cover.jpg" onerror="alert(1)" onload="evil()">'
      const cleaned = sanitizeHtml(malicious)
      expect(cleaned).not.toContain('onerror')
      expect(cleaned).not.toContain('onload')
      expect(cleaned).toContain('<img src="cover.jpg">')
    })

    it('neutralizes javascript: URLs in href and src attributes', () => {
      const malicious = '<a href="javascript:alert(1)">Click me</a><a href=\'javascript:evil()\'>Other</a>'
      const cleaned = sanitizeHtml(malicious)
      expect(cleaned).not.toContain('javascript:')
      expect(cleaned).toContain('<a href="#">Click me</a>')
      expect(cleaned).toContain('<a href="#">Other</a>')
    })

    it('strips svg, style, embed, and object tags completely', () => {
      const malicious = '<svg onload="alert(1)"><circle /></svg><style>body{display:none}</style><embed src="bad.swf">'
      const cleaned = sanitizeHtml(malicious)
      expect(cleaned).not.toContain('<svg')
      expect(cleaned).not.toContain('<style')
      expect(cleaned).not.toContain('<embed')
    })

    it('preserves safe and legitimate markup', () => {
      const safe = '<p>Hello <strong>world</strong> and <em>welcome</em></p>'
      expect(sanitizeHtml(safe)).toBe(safe)
    })
  })

  describe('CommandPalette XSS mitigation', () => {
    beforeEach(() => {
      vi.clearAllMocks()
      document.body.innerHTML = ''
    })

    afterEach(() => {
      document.body.innerHTML = ''
    })

    it('does not render unescaped executable tags when tracks have malicious titles', async () => {
      const maliciousTrack = {
        id: 1,
        title: '<img src=x onerror=alert(1)>Malicious Title',
        artist: 'Unknown',
        album: 'Unknown',
        duration_secs: 180,
        service: 'local',
        quality: 'FLAC',
      }

      vi.mocked(libraryApi.searchTracks).mockResolvedValue({
        tracks: [maliciousTrack as any],
        total: 1,
        offset: 0,
        limit: 10,
        has_more: false,
      })

      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
      })

      // Open palette
      wrapper.vm.open()
      await flushPromises()

      // Set search query that triggers search via teleported input
      const input = document.body.querySelector('input')
      expect(input).not.toBeNull()
      input!.value = 'Malicious'
      input!.dispatchEvent(new Event('input'))
      await flushPromises()

      // Allow debounce/search promise to settle
      await new Promise((resolve) => setTimeout(resolve, 350))
      await flushPromises()

      const html = document.body.innerHTML
      // Should NOT contain a live unescaped <img> with onerror
      expect(html).not.toMatch(/<img\s+src=x\s+onerror/i)
      // Should contain escaped entity
      expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;')

      wrapper.unmount()
      document.body.innerHTML = ''
    })
  })
})
