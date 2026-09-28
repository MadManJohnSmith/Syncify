import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { routes } from '@/main'

/**
 * [TASK-27] Frontend Component Hygiene & Orphan Purge Verification
 *
 * Ensures that:
 * 1. The 7 orphaned components/views are purged from `ui/src/components/` and `ui/src/views/`.
 * 2. Their dead/exclusive test suites are purged from `ui/src/__tests__/`.
 * 3. No remaining source files in `ui/src/` retain dangling imports to them.
 * 4. Active router configuration in `ui/src/main.ts` does not register QueueView and redirects `/queue` -> `/downloads`.
 */
describe('Orphaned Components Hygiene (TASK-27)', () => {
  const PURGED_COMPONENTS = [
    { name: 'AppUpdater.vue', originalPath: 'ui/src/components/AppUpdater.vue' },
    { name: 'TrackComparisonModal.vue', originalPath: 'ui/src/components/TrackComparisonModal.vue' },
    { name: 'TraySettings.vue', originalPath: 'ui/src/components/TraySettings.vue' },
    { name: 'SystemTray.vue', originalPath: 'ui/src/components/SystemTray.vue' },
    { name: 'ErrorHandler.vue', originalPath: 'ui/src/components/ErrorHandler.vue' },
    { name: 'DevParityMatrix.vue', originalPath: 'ui/src/components/DevParityMatrix.vue' },
    { name: 'QueueView.vue', originalPath: 'ui/src/views/QueueView.vue' },
  ]

  const PURGED_TESTS = [
    { name: 'DevParityMatrix.spec.ts', originalPath: 'ui/src/__tests__/components/DevParityMatrix.spec.ts' },
    { name: 'QueueView.spec.ts', originalPath: 'ui/src/__tests__/views/QueueView.spec.ts' },
  ]

  const rootDir = path.resolve(__dirname, '../../../../')
  const srcDir = path.resolve(__dirname, '../../')

  it('verifies none of the 7 orphaned components exist in active ui/src/ tree', () => {
    for (const comp of PURGED_COMPONENTS) {
      const fullPath = path.join(rootDir, comp.originalPath)
      expect(fs.existsSync(fullPath), `Expected ${comp.originalPath} to be removed`).toBe(false)
    }
  })

  it('verifies dead test suites for orphaned components are purged from ui/src/__tests__/', () => {
    for (const testFile of PURGED_TESTS) {
      const fullPath = path.join(rootDir, testFile.originalPath)
      expect(fs.existsSync(fullPath), `Expected ${testFile.originalPath} to be removed`).toBe(false)
    }
  })

  it('verifies no active source files retain dangling imports to purged components', () => {
    function scanFiles(dir: string): string[] {
      const results: string[] = []
      const entries = fs.readdirSync(dir, { withFileTypes: true })
      for (const entry of entries) {
        const fullPath = path.join(dir, entry.name)
        if (entry.isDirectory()) {
          if (entry.name !== '__tests__' && entry.name !== 'node_modules' && entry.name !== 'dist') {
            results.push(...scanFiles(fullPath))
          }
        } else if (entry.isFile() && (entry.name.endsWith('.vue') || entry.name.endsWith('.ts'))) {
          results.push(fullPath)
        }
      }
      return results
    }

    const sourceFiles = scanFiles(srcDir)
    expect(sourceFiles.length).toBeGreaterThan(0)

    for (const comp of PURGED_COMPONENTS) {
      const baseName = comp.name.replace(/\.(vue|ts)$/, '')
      const importRegex = new RegExp(`from\\s+['"].*${baseName}(\\.vue)?['"]`, 'g')

      for (const srcFile of sourceFiles) {
        const code = fs.readFileSync(srcFile, 'utf-8')
        const matches = code.match(importRegex)
        expect(matches, `Found dangling import for ${baseName} in ${path.relative(srcDir, srcFile)}`).toBeNull()
      }
    }
  })

  it('verifies router does not route QueueView and redirects /queue to /downloads', () => {
    // 1. Verify /queue redirect
    const queueRoute = routes.find((r) => r.path === '/queue')
    expect(queueRoute).toBeDefined()
    expect(queueRoute?.redirect).toBe('/downloads')

    // 2. Verify no route component references QueueView
    for (const route of routes) {
      if (route.component) {
        const compName = (route.component as any).name || (route.component as any).__name || ''
        expect(compName).not.toBe('QueueView')
      }
    }
  })
})
