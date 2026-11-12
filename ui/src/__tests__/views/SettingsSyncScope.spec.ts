import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'

const settingsSource = readFileSync(
  path.resolve(process.cwd(), 'src/views/settings/SettingsSync.vue'),
  'utf8',
)
const accountsSource = readFileSync(
  path.resolve(process.cwd(), 'src/views/AccountsView.vue'),
  'utf8',
)
const readmeSource = readFileSync(path.resolve(process.cwd(), '../README.md'), 'utf8')

describe('sync settings shipped scope', () => {
  it('does not expose the retired incremental sync switch (#197 SB02)', () => {
    // Retired per the sync spec (#64): the sync always walks the full library
    // and no code ever read `incremental_sync` to decide the traversal.
    expect(settingsSource).not.toContain('Incremental sync only')
    expect(settingsSource).not.toMatch(/incrementalOnly/)
    expect(accountsSource).not.toContain('Incremental sync only')
    expect(accountsSource).not.toMatch(/incremental_sync/)
  })

  it('does not expose automatic sync controls without a runtime scheduler', () => {
    expect(settingsSource).not.toContain('Enable automatic library sync')
    expect(settingsSource).not.toContain('Sync interval')
    expect(settingsSource).not.toContain('Sync on startup')
    expect(readmeSource).not.toContain('programaciones y ejecución desatendida')
  })

  it('does not show disabled coming-soon rate-limit UI', () => {
    expect(settingsSource).not.toContain('Coming in next update')
    expect(settingsSource).not.toContain('Show per-service rate limits')
  })
})
