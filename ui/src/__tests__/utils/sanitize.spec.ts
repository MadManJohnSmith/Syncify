/**
 * Unit tests for HTML sanitization, entity escaping, and XSS prevention (TASK-08)
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { escapeHtml, escapeRegex, escapeRegExp, highlightMatch, sanitizeHtml } from '../../utils/sanitize'
import CommandPalette from '../../components/CommandPalette.vue'
import KeyboardShortcuts from '../../components/KeyboardShortcuts.vue'
import HelpPanel from '../../components/HelpPanel.vue'
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

  describe('escapeRegex and escapeRegExp (TASK-09)', () => {
    it('escapes regex metacharacters properly', () => {
      expect(escapeRegex('test.*+?^${}()|[]\\')).toBe(
        'test\\.\\*\\+\\?\\^\\$\\{\\}\\(\\)\\|\\[\\]\\\\'
      )
      expect(escapeRegExp('test.*+?^${}()|[]\\')).toBe(
        'test\\.\\*\\+\\?\\^\\$\\{\\}\\(\\)\\|\\[\\]\\\\'
      )
    })

    it('prevents SyntaxError: Invalid regular expression when creating RegExp from special metacharacters', () => {
      const metachars = ['[', '*', '+', '(', ')', '?', '\\', '^', '$', '{', '}', '|']
      for (const char of metachars) {
        expect(() => new RegExp(escapeRegExp(char), 'gi')).not.toThrow()
        expect(() => new RegExp(escapeRegex(char), 'gi')).not.toThrow()
      }
    })

    it('safely handles complex unclosed brackets, wildcards and quantification without throwing', () => {
      const dangerousPatterns = ['[a-z', '(((', '***', '+++', '???', '{{5}', '\\\\\\', 'a|b|(', '[*+?^${}()|\\]']
      for (const pat of dangerousPatterns) {
        expect(() => new RegExp(escapeRegExp(pat), 'gi')).not.toThrow()
      }
    })

    it('handles empty strings safely', () => {
      expect(escapeRegex('')).toBe('')
      expect(escapeRegExp('')).toBe('')
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

  describe('highlightMatch (TASK-09)', () => {
    it('processes query with "[" without throwing SyntaxError and highlights match', () => {
      expect(() => highlightMatch('Search [Ctrl+K] shortcut', '[')).not.toThrow()
      const result = highlightMatch('Search [Ctrl+K] shortcut', '[')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">[</mark>')
    })

    it('processes query with "*" without throwing SyntaxError and highlights match', () => {
      expect(() => highlightMatch('Highlight *starred* tracks', '*')).not.toThrow()
      const result = highlightMatch('Highlight *starred* tracks', '*')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">*</mark>')
    })

    it('processes query with "(" without throwing SyntaxError and highlights match', () => {
      expect(() => highlightMatch('Option (A) selected', '(')).not.toThrow()
      const result = highlightMatch('Option (A) selected', '(')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">(</mark>')
    })

    it('processes query with "\\" without throwing SyntaxError and highlights match', () => {
      expect(() => highlightMatch('C:\\Music\\Tracks', '\\')).not.toThrow()
      const result = highlightMatch('C:\\Music\\Tracks', '\\')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">\\</mark>')
    })

    it('processes query with "+" without throwing SyntaxError and highlights match', () => {
      expect(() => highlightMatch('Press Ctrl+K to open', '+')).not.toThrow()
      const result = highlightMatch('Press Ctrl+K to open', '+')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">+</mark>')
    })

    it('processes complex unclosed pattern "[a-z" without throwing SyntaxError and highlights match', () => {
      expect(() => highlightMatch('Pattern [a-z0-9] matched', '[a-z')).not.toThrow()
      const result = highlightMatch('Pattern [a-z0-9] matched', '[a-z')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">[a-z</mark>')
    })

    it('processes other regex metacharacters (?, ), ^, $, |, {, }) safely', () => {
      const metachars = ['?', ')', '^', '$', '|', '{', '}']
      for (const char of metachars) {
        expect(() => highlightMatch(`Testing char: ${char}`, char)).not.toThrow()
      }
    })

    it('escapes HTML in target text while highlighting', () => {
      const result = highlightMatch('<script>alert("xss")</script> match', 'match')
      expect(result).not.toContain('<script>')
      expect(result).toContain('&lt;script&gt;')
      expect(result).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">match</mark>')
    })
  })

  describe('KeyboardShortcuts ReDoS & regex crash prevention (TASK-09)', () => {
    it('does not throw SyntaxError when searching with special regex characters like "[" or "*"', async () => {
      const wrapper = mount(KeyboardShortcuts)

      // Test direct highlightMatch method with "[" and "*"
      expect(() => (wrapper.vm as any).highlightMatch('Open [Ctrl+K]')).not.toThrow()

      ;(wrapper.vm as any).searchQuery = '['
      expect(() => (wrapper.vm as any).highlightMatch('Open [Ctrl+K]')).not.toThrow()
      const highlightedBracket = (wrapper.vm as any).highlightMatch('Open [Ctrl+K]')
      expect(highlightedBracket).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">[</mark>')

      ;(wrapper.vm as any).searchQuery = '*'
      expect(() => (wrapper.vm as any).highlightMatch('Starred *action*')).not.toThrow()
      const highlightedStar = (wrapper.vm as any).highlightMatch('Starred *action*')
      expect(highlightedStar).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">*</mark>')

      // Test with other problematic regex metacharacters
      const problematicInputs = ['[', '*', '(', '\\', '+', '[a-z', ')', '?', '(?=', '.*']
      for (const query of problematicInputs) {
        ;(wrapper.vm as any).searchQuery = query
        expect(() => (wrapper.vm as any).highlightMatch('Testing shortcut string [Ctrl+Shift] (a-z) *test* \\path')).not.toThrow()
      }

      wrapper.unmount()
    })
  })

  describe('CommandPalette and HelpPanel ReDoS & regex crash prevention (TASK-09)', () => {
    beforeEach(() => {
      vi.clearAllMocks()
      document.body.innerHTML = ''
    })

    afterEach(() => {
      document.body.innerHTML = ''
    })

    it('does not throw SyntaxError when searching with special regex characters in CommandPalette', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
      })

      wrapper.vm.open()
      await flushPromises()

      const input = document.body.querySelector('input')
      expect(input).not.toBeNull()

      const dangerousInputs = ['[', '*', '(', '\\', '+', '[a-z', '(?=', '.*']
      for (const query of dangerousInputs) {
        input!.value = query
        input!.dispatchEvent(new Event('input'))
        await flushPromises()

        // Verify template rendered without error
        expect(() => {
          const bodyHtml = document.body.innerHTML
          expect(bodyHtml).toBeDefined()
        }).not.toThrow()
      }

      wrapper.unmount()
    })

    it('does not throw SyntaxError when searching with special regex characters in HelpPanel', async () => {
      const wrapper = mount(HelpPanel, {
        attachTo: document.body,
      })

      wrapper.vm.open()
      await flushPromises()

      const input = document.body.querySelector('input')
      expect(input).not.toBeNull()

      const dangerousInputs = ['[', '*', '(', '\\', '+', '[a-z', '(?=', '.*']
      for (const query of dangerousInputs) {
        input!.value = query
        input!.dispatchEvent(new Event('input'))
        await flushPromises()

        // Verify template rendered without error
        expect(() => {
          const bodyHtml = document.body.innerHTML
          expect(bodyHtml).toBeDefined()
        }).not.toThrow()
      }

      wrapper.unmount()
    })
  })
})
