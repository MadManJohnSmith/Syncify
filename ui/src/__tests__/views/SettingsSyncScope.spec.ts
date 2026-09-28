import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const settingsSource = readFileSync(
  fileURLToPath(new URL('../../views/settings/SettingsSync.vue', import.meta.url)),
  'utf8',
)
const readmeSource = readFileSync(
  fileURLToPath(new URL('../../../../README.md', import.meta.url)),
  'utf8',
)

describe('sync settings shipped scope', () => {
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
