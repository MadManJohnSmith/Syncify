import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'

// #197 (SB02) — truthful copy for Audio Processing: only settings the app
// honours may render. Transcoding (transcode_enabled/format/bitrate and
// keep_original_after_transcode) and artwork_max_size are hidden until their
// features exist; each one has a reopening row in
// docs/auditoria/matriz-ajustes.md.
const processingSource = readFileSync(
  path.resolve(process.cwd(), 'src/views/settings/SettingsProcessing.vue'),
  'utf8',
)

describe('audio processing shipped scope (#197)', () => {
  it('does not render the transcoding feature that no code consumes', () => {
    expect(processingSource).not.toContain('Enable transcoding')
    expect(processingSource).not.toContain('Target format')
    expect(processingSource).not.toContain('Bitrate')
    expect(processingSource).not.toContain('transcode_enabled')
    expect(processingSource).not.toContain('transcode_format')
    expect(processingSource).not.toContain('transcode_bitrate')
  })

  it('does not render the max artwork resolution setting', () => {
    expect(processingSource).not.toContain('Max artwork resolution')
    expect(processingSource).not.toMatch(/artwork_max_size/)
  })

  it('keeps the embedding and ReplayGain settings that are wired', () => {
    expect(processingSource).toContain('Embed lyrics in audio files')
    expect(processingSource).toContain('Embed album artwork')
    expect(processingSource).toContain('ReplayGain mode')
    expect(processingSource).toContain('Target loudness')
  })
})
