/**
 * Utility functions for HTML sanitization and escaping
 * Mitigates XSS vulnerabilities when rendering dynamic content or search highlights
 */

const HTML_ESCAPE_MAP: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
}

/**
 * Escapes HTML control characters (&, <, >, ", ') to their corresponding safe entities.
 */
export function escapeHtml(str: string | null | undefined): string {
  if (str === null || str === undefined) return ''
  const s = typeof str === 'string' ? str : String(str)
  if (s.length === 0) return ''
  return s.replace(/[&<>"']/g, (char) => HTML_ESCAPE_MAP[char] || char)
}

/**
 * Escapes regex special characters to prevent invalid syntax or ReDoS when building dynamic RegExps.
 */
export function escapeRegex(str: string | null | undefined): string {
  if (str === null || str === undefined) return ''
  const s = typeof str === 'string' ? str : String(str)
  if (s.length === 0) return ''
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

export const escapeRegExp = escapeRegex

/**
 * Safely highlights matching text substrings by escaping HTML entities and regex metacharacters.
 */
export function highlightMatch(text: string, query: string): string {
  const escapedText = escapeHtml(text)
  const q = (query || '').trim()
  if (!q) return escapedText
  const safeQuery = escapeRegExp(escapeHtml(q))
  const regex = new RegExp(`(${safeQuery})`, 'gi')
  return escapedText.replace(regex, (_match, p1) => `<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">${p1}</mark>`)
}

const DANGEROUS_TAGS = ['script', 'iframe', 'object', 'embed', 'style', 'svg']

/**
 * Strips executable / high-risk tags, inline event attributes (e.g. onload, onerror),
 * and unsafe protocols (e.g. javascript:) while preserving harmless markup.
 */
export function sanitizeHtml(html: string): string {
  if (!html) return ''

  let result = html
  let previous: string

  // Iteratively strip dangerous tags and their content until fixed point is reached
  do {
    previous = result
    for (const tag of DANGEROUS_TAGS) {
      // Remove paired blocks e.g. <script>...</script>
      const blockRegex = new RegExp(`<\\s*${tag}\\b[^>]*>[\\s\\S]*?<\\s*\\/\\s*${tag}\\s*>`, 'gi')
      result = result.replace(blockRegex, '')
      // Remove unclosed or self-closing tags e.g. <script ...> or </script>
      const tagRegex = new RegExp(`<\\s*\\/?\\s*${tag}\\b[^>]*>`, 'gi')
      result = result.replace(tagRegex, '')
    }
  } while (result !== previous)

  // Remove inline event handlers (on* attributes: onerror, onclick, onload, etc.)
  result = result.replace(/\s+on[a-zA-Z]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, '')

  // Neutralize javascript: URLs in attributes (href="javascript:...", src="javascript:...", etc.)
  result = result.replace(/(href|src|action|formaction)\s*=\s*(["'])\s*(?:javascript|data\s*:\s*text\/html):[\s\S]*?\2/gi, '$1="#"')
  result = result.replace(/(href|src|action|formaction)\s*=\s*(?:javascript|data\s*:\s*text\/html):[^\s>]*/gi, '$1="#"')

  return result
}
