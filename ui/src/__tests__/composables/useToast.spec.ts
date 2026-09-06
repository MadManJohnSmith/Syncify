/**
 * Unit tests for useToast composable (TASK-28)
 * Tests auto-dismiss, pauseToast, and resumeToast timing logic.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { useToast } from '@/composables/useToast'

describe('useToast pauseToast & resumeToast', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    const { clearAllHistory, toasts, dismiss } = useToast()
    clearAllHistory()
    toasts.value.forEach(t => dismiss(t.id))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('pauses auto-dismiss timer and freezes remaining time', () => {
    const { info, toasts, pauseToast, resumeToast } = useToast()
    const id = info('Test Title', 'Testing pause')
    expect(toasts.value.length).toBe(1)
    const toast = toasts.value[0]
    expect(toast.paused).toBe(false)
    expect(toast.duration).toBe(4000)

    // Advance 1500ms
    vi.advanceTimersByTime(1500)
    expect(toasts.value.length).toBe(1)

    // Pause the toast
    pauseToast(id)
    expect(toast.paused).toBe(true)
    expect(toast.timerRemaining).toBeLessThanOrEqual(2500)
    expect(toast.timerRemaining).toBeGreaterThanOrEqual(2400)

    // Advance 5000ms while paused - toast should NOT be dismissed
    vi.advanceTimersByTime(5000)
    expect(toasts.value.length).toBe(1)
    expect(toast.paused).toBe(true)

    // Resume the toast
    resumeToast(id)
    expect(toast.paused).toBe(false)

    // Advance 2000ms - still visible because remaining was ~2500ms
    vi.advanceTimersByTime(2000)
    expect(toasts.value.length).toBe(1)

    // Advance remaining 600ms - toast should dismiss
    vi.advanceTimersByTime(600)
    expect(toasts.value.length).toBe(0)
  })

  it('handles multiple pause and resume cycles cleanly', () => {
    const { warning, toasts, pauseToast, resumeToast } = useToast()
    const id = warning('Warning Title', 'Warning description') // duration 5000ms
    expect(toasts.value.length).toBe(1)
    const toast = toasts.value[0]

    // Advance 1000ms
    vi.advanceTimersByTime(1000)
    pauseToast(id)
    expect(toast.timerRemaining).toBeLessThanOrEqual(4000)
    expect(toast.timerRemaining).toBeGreaterThanOrEqual(3900)

    // Wait 2000ms while paused
    vi.advanceTimersByTime(2000)
    resumeToast(id)

    // Advance 2000ms
    vi.advanceTimersByTime(2000)
    pauseToast(id)
    expect(toast.timerRemaining).toBeLessThanOrEqual(2000)
    expect(toast.timerRemaining).toBeGreaterThanOrEqual(1900)

    // Resume and finish remaining
    resumeToast(id)
    vi.advanceTimersByTime(2100)
    expect(toasts.value.length).toBe(0)
  })

  it('safely handles non-existent IDs or non-auto-dismissing toasts', () => {
    const { error, toasts, pauseToast, resumeToast } = useToast()
    // Error toasts have duration = 0 (autoDismiss = false)
    const errorId = error('Fatal Error', 'Does not auto dismiss')
    expect(toasts.value.length).toBe(1)

    // Pausing non-existent id
    expect(() => pauseToast('invalid-id')).not.toThrow()
    expect(() => resumeToast('invalid-id')).not.toThrow()

    // Pausing non-auto-dismiss toast
    expect(() => pauseToast(errorId)).not.toThrow()
    expect(toasts.value[0].paused).toBe(false)
  })
})
