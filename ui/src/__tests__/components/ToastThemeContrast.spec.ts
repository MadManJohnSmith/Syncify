import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import ToastNotifications from '@/components/ToastNotifications.vue'
import { confirm, useDialog, useToast } from '@/composables/useToast'

const themeCss = readFileSync(resolve(__dirname, '../../styles/main.css'), 'utf8')
function token(theme: string, name: string): string {
  const start = themeCss.indexOf(`[data-theme="${theme}"] {`)
  expect(start).toBeGreaterThan(-1)
  const end = themeCss.indexOf('}', start)
  const value = themeCss.slice(start, end).match(new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`))?.[1]
  expect(value).toBeDefined()
  return value!
}
function luminance(hex: string): number {
  const parts = [1, 3, 5].map(i => {
    const channel = parseInt(hex.slice(i, i + 2), 16) / 255
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
  })
  return parts[0] * 0.2126 + parts[1] * 0.7152 + parts[2] * 0.0722
}
function ratio(a: string, b: string): number {
  const hi = Math.max(luminance(a), luminance(b))
  const lo = Math.min(luminance(a), luminance(b))
  return (hi + 0.05) / (lo + 0.05)
}

// WF-INK-01: validate the pairing the component renders against actual theme tokens.
describe('ToastNotifications theme contrast', () => {
  beforeEach(() => {
    document.documentElement.dataset.theme = 'tinta'
    useToast().toasts.value.forEach(t => useToast().dismiss(t.id))
  })
  afterEach(async () => {
    while (useDialog().activeDialog.value) {
      useDialog().resolveDialog(null)
      await flushPromises()
    }
    delete document.documentElement.dataset.theme
    document.body.innerHTML = ''
  })

  it.each(['tinta', 'consola', 'riso'])('%s confirm and progress ink pair with accent at >= 4.5:1', async theme => {
    document.documentElement.dataset.theme = theme
    const host = mount(ToastNotifications, { attachTo: document.body })
    const pending = confirm('Remove this track from the queue?', { title: 'Remove Track', variant: 'warning' })
    await flushPromises()
    const button = document.querySelector('.dialog-confirm') as HTMLButtonElement
    expect(button).not.toBeNull()
    expect(button.classList.contains('bg-accent')).toBe(true)
    expect(button.classList.contains('text-on-accent')).toBe(true)
    expect(button.classList.contains('text-white')).toBe(false)
    const progressId = useToast().progress('Importing', 40)
    await flushPromises()
    const card = host.find('.toast-progress')
    expect(card.exists()).toBe(true)
    expect(card.find('.toast-message p').classes()).toContain('text-on-accent')
    expect(card.find('.toast-icon span').classes()).toContain('text-on-accent')
    expect(card.find('.bg-on-accent').exists()).toBe(true)
    // El cierre del toast de progreso también va al token.
    const closeButton = card.findAll('button')[0]
    expect(closeButton.find('span').classes().join(' ')).toContain('text-on-accent')
    expect(ratio(token(theme, 'accent'), token(theme, 'on-accent'))).toBeGreaterThanOrEqual(4.5)
    useToast().dismiss(progressId)
    button.click()
    await expect(pending).resolves.toBe(true)
    host.unmount()
  })

  it('preserves the danger background and white ink', async () => {
    const host = mount(ToastNotifications, { attachTo: document.body })
    const pending = confirm('Delete?', { title: 'Delete', variant: 'danger' })
    await flushPromises()
    const button = document.querySelector('.dialog-confirm') as HTMLButtonElement
    expect(button.classList.contains('bg-error')).toBe(true)
    expect(button.classList.contains('text-white')).toBe(true)
    expect(button.classList.contains('text-on-accent')).toBe(false)
    button.click()
    await expect(pending).resolves.toBe(true)
    host.unmount()
  })
})
