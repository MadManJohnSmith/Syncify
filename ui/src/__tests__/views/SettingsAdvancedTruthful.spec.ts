import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'

// #197 (SB02) — truthful copy for Advanced: no setting without a consumer may
// render. Hidden here: cache, import/worker timeouts, debug toggles (logging
// orphans with no reader). Kept: log_level/log_to_file (consumed by
// src-tauri/src/services/logging.rs) and the diagnostics/vacuum actions.
// Reopening rows live in docs/auditoria/matriz-ajustes.md.
const advancedSource = readFileSync(
  path.resolve(process.cwd(), 'src/views/settings/SettingsAdvanced.vue'),
  'utf8',
)

describe('advanced settings shipped scope (#197)', () => {
  it('does not render the cache setting that no code reads', () => {
    expect(advancedSource).not.toContain('Enable Cache')
    expect(advancedSource).not.toMatch(/cache_enabled/)
  })

  it('does not render the import/worker timeout settings', () => {
    expect(advancedSource).not.toContain('Max Imports')
    expect(advancedSource).not.toMatch(/max_concurrent_imports/)
    expect(advancedSource).not.toContain('Timeout (sec)')
    expect(advancedSource).not.toMatch(/worker_timeout_seconds/)
    expect(advancedSource).not.toMatch(/request_timeout_seconds/)
  })

  it('does not render the debug toggles that no code reads', () => {
    expect(advancedSource).not.toContain('Debug Mode')
    expect(advancedSource).not.toMatch(/debug_mode/)
    expect(advancedSource).not.toContain('API Logs')
    expect(advancedSource).not.toMatch(/verbose_api_logging/)
  })

  it('keeps the logging settings the logging runtime consumes', () => {
    expect(advancedSource).toContain('Log Level')
    expect(advancedSource).toContain('Log to File')
  })

  it('keeps the diagnostics and maintenance actions', () => {
    expect(advancedSource).toContain('Run Diagnostics')
    expect(advancedSource).toContain('Vacuum Database')
  })
})
