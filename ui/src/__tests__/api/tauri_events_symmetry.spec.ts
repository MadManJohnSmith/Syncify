import { describe, it, expect } from 'vitest'
import { TauriEvents } from '@/api/tauri'
import { TauriEvents as BusTauriEvents } from '@/composables/useEventBus'
import fs from 'node:fs'
import path from 'node:path'

describe('TASK-118: IPC Event Name Symmetry between Frontend and Rust Backend', () => {
    it('exports identical canonical TauriEvents from both @/api/tauri and @/composables/useEventBus', () => {
        expect(TauriEvents).toBeDefined()
        expect(BusTauriEvents).toBeDefined()
        expect(TauriEvents).toEqual(BusTauriEvents)
    })

    it('defines canonical constants for all 5 diagnosed misaligned pairs', () => {
        // 1. enrichment-progress vs enrichment_progress
        expect(TauriEvents.ENRICHMENT_PROGRESS).toBe('enrichment-progress')
        expect(TauriEvents.ENRICHMENT_PROGRESS_ALT).toBe('enrichment_progress')

        // 2. background-enrichment-status vs syncify:enrichment_event
        expect(TauriEvents.BACKGROUND_ENRICHMENT_STATUS).toBe('background-enrichment-status')
        expect(TauriEvents.ENRICHMENT_EVENT).toBe('syncify:enrichment_event')

        // 3. sync-failed vs import-failed
        expect(TauriEvents.SYNC_FAILED).toBe('sync-failed')
        expect(TauriEvents.IMPORT_FAILED).toBe('import-failed')

        // 4. scan-progress, scan-complete, organize-progress vs syncify:progress
        expect(TauriEvents.SCAN_PROGRESS).toBe('scan-progress')
        expect(TauriEvents.SCAN_COMPLETE).toBe('scan-complete')
        expect(TauriEvents.ORGANIZE_PROGRESS).toBe('organize-progress')
        expect(TauriEvents.ORGANIZE_COMPLETE).toBe('organize-complete')
        expect(TauriEvents.PROGRESS).toBe('syncify:progress')

        // 5. auth-state-updated in accounts / auth flow
        expect(TauriEvents.AUTH_STATE_UPDATED).toBe('auth-state-updated')
        expect(TauriEvents.AUTH_SESSION_EXPIRED).toBe('auth-session-expired')
    })

    it('verifies all Rust backend app.emit / window.emit calls have matching representations in TauriEvents or known protocol', () => {
        const srcTauriPath = path.resolve(__dirname, '../../../../src-tauri/src')
        if (!fs.existsSync(srcTauriPath)) {
            // If running in isolated sandbox where parent paths are relative to ui
            return
        }

        const emittedEvents = new Set<string>()
        const emitRegex = /\.emit\(\s*"([^"]+)"/g

        function scanDir(dir: string) {
            const files = fs.readdirSync(dir)
            for (const file of files) {
                const fullPath = path.join(dir, file)
                const stat = fs.statSync(fullPath)
                if (stat.isDirectory()) {
                    scanDir(fullPath)
                } else if (file.endsWith('.rs')) {
                    const content = fs.readFileSync(fullPath, 'utf8')
                    let match: RegExpExecArray | null
                    while ((match = emitRegex.exec(content)) !== null) {
                        emittedEvents.add(match[1])
                    }
                }
            }
        }

        scanDir(srcTauriPath)

        // Known internal or specialized startup events that may not need direct UI eventBus bindings
        const knownInternalEvents = new Set([
            'startup:error',
            'startup:complete',
            'system:health',
            'tempo:batch_progress',
            'favorites:export_progress',
        ])

        const eventValues = new Set<string>(Object.values(TauriEvents))

        // Ensure key emitted events are in TauriEvents
        const requiredEmittedEvents = [
            'enrichment-progress',
            'enrichment_progress',
            'background-enrichment-status',
            'syncify:enrichment_event',
            'sync-failed',
            'import-failed',
            'syncify:progress',
            'scan-progress',
            'scan-complete',
            'organize-progress',
            'organize-complete',
            'auth-state-updated',
            'auth-session-expired',
            'syncify:download_progress',
            'syncify:notification',
            'syncify:log_event',
        ]

        for (const req of requiredEmittedEvents) {
            expect(emittedEvents.has(req), `Rust backend must emit event "${req}"`).toBe(true)
            expect(eventValues.has(req), `TauriEvents must define event "${req}"`).toBe(true)
        }

        // Check that any emitted event is either in TauriEvents or in known internal events
        for (const emitted of emittedEvents) {
            const isCovered = eventValues.has(emitted) || knownInternalEvents.has(emitted)
            expect(isCovered, `Emitted event "${emitted}" should be recognized in TauriEvents`).toBe(true)
        }
    })

    it('verifies that Frontend Vue components and composables listen using canonical TauriEvents', () => {
        const uiSrcPath = path.resolve(__dirname, '../../')
        const rawListenCalls: string[] = []
        // Regex to detect listen('string-literal') or eventBus.on('string-literal')
        const rawStringListenerRegex = /(?:listen<[^>]*>|eventBus\.on)\(\s*['"]([a-zA-Z0-9_\-:]+)['"]/g

        // Allow test files to use string literals in testing
        function scanDir(dir: string) {
            const files = fs.readdirSync(dir)
            for (const file of files) {
                const fullPath = path.join(dir, file)
                const stat = fs.statSync(fullPath)
                if (stat.isDirectory()) {
                    if (file !== '__tests__' && file !== 'node_modules') {
                        scanDir(fullPath)
                    }
                } else if (file.endsWith('.vue') || file.endsWith('.ts')) {
                    const content = fs.readFileSync(fullPath, 'utf8')
                    let match: RegExpExecArray | null
                    while ((match = rawStringListenerRegex.exec(content)) !== null) {
                        rawListenCalls.push(`${path.relative(uiSrcPath, fullPath)}: ${match[1]}`)
                    }
                }
            }
        }

        scanDir(uiSrcPath)

        // In production code under views/ and composables/, no raw event literals should be listened to without TauriEvents
        expect(rawListenCalls).toEqual([])
    })
})
