import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

/**
 * [TASK-60] Frontend Component Hygiene & Orphan Purge Verification
 *
 * Ensures that:
 * 1. The 10 orphaned generic template stubs are purged from `ui/src/components/`.
 * 2. No remaining source files in `ui/src/` retain dangling imports to them.
 */
describe('Orphaned Components Hygiene (TASK-60)', () => {
  const PURGED_COMPONENTS = [
    'EmptyState.vue',
    'ErrorState.vue',
    'HelpTooltip.vue',
    'InlineHint.vue',
    'LoadingButton.vue',
    'LoadingSpinner.vue',
    'SkeletonLoader.vue',
    'ValidationError.vue',
    'ImageLoader.vue',
    'InfiniteScroll.vue',
  ]

  const componentsDir = path.resolve(__dirname, '../../components')
  const srcDir = path.resolve(__dirname, '../../')

  it('verifies none of the 10 purged components remain in ui/src/components/', () => {
    for (const file of PURGED_COMPONENTS) {
      const activeFilePath = path.join(componentsDir, file)
      expect(fs.existsSync(activeFilePath), `Expected ${file} to be removed from ui/src/components/`).toBe(false)
    }
  })

  it('verifies no source files in ui/src/ import any of the purged components', () => {
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

    for (const file of PURGED_COMPONENTS) {
      const componentName = file.replace('.vue', '')
      const importRegex = new RegExp(`from\\s+['"].*${componentName}(\\.vue)?['"]`, 'g')

      for (const srcFile of sourceFiles) {
        const code = fs.readFileSync(srcFile, 'utf-8')
        const matches = code.match(importRegex)
        expect(matches, `Found dangling import for ${componentName} in ${path.relative(srcDir, srcFile)}`).toBeNull()
      }
    }
  })
})
