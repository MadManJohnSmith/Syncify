/**
 * SplashAndPathValidation.spec.ts
 * TASK-64: Reactive SplashScreen startup transition and robust PathSelector filesystem validation.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import SplashScreen from '@/components/SplashScreen.vue'
import PathSelector from '@/components/settings/PathSelector.vue'
import App from '@/App.vue'
import { resetMocks, mockInvoke } from '../setup'
import { open } from '@tauri-apps/plugin-dialog'

describe('SplashScreen.vue & App.vue Startup Lifecycle (TASK-64)', () => {
  beforeEach(() => {
    resetMocks()
    localStorage.clear()
    vi.restoreAllMocks()
  })

  afterEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  describe('SplashScreen.vue rendering & state handling', () => {
    it('renders normal loading state with progress and status text', () => {
      const wrapper = mount(SplashScreen, {
        props: {
          statusText: 'Connecting to database...',
          progress: 50,
          error: null,
        },
      })

      expect(wrapper.find('[data-testid="splash-loading"]').exists()).toBe(true)
      expect(wrapper.find('[data-testid="splash-error"]').exists()).toBe(false)
      expect(wrapper.find('[data-testid="splash-status"]').text()).toBe('Connecting to database...')
      expect(wrapper.text()).toContain('Syncify')
      expect(wrapper.text()).toContain('Your Unified Music Library')
    })

    it('renders error state and retry button when error prop is provided', async () => {
      const wrapper = mount(SplashScreen, {
        props: {
          error: 'Database connection failed: disk I/O error',
          statusText: 'Initialization error',
          progress: 45,
        },
      })

      expect(wrapper.find('[data-testid="splash-error"]').exists()).toBe(true)
      expect(wrapper.find('[data-testid="splash-loading"]').exists()).toBe(false)
      expect(wrapper.text()).toContain('Database connection failed: disk I/O error')

      const retryBtn = wrapper.find('[data-testid="splash-retry-btn"]')
      expect(retryBtn.exists()).toBe(true)

      await retryBtn.trigger('click')
      expect(wrapper.emitted('retry')).toBeTruthy()
      expect(wrapper.emitted('retry')!.length).toBe(1)
    })

    it('emits ready and complete when hide() is called', () => {
      const wrapper = mount(SplashScreen)
      const vm = wrapper.vm as any

      vm.hide()
      expect(wrapper.emitted('ready')).toBeTruthy()
      expect(wrapper.emitted('complete')).toBeTruthy()
    })
  })

  describe('App.vue splash screen reactive lifecycle', () => {
    it('initializes with showSplash = true and transitions to false upon successful initialization', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'run_health_check') {
          return {
            database_ok: true,
            python_ok: true,
            ffmpeg_available: true,
            chromaprint_available: true,
            services_configured: ['qobuz'],
            errors: [],
          }
        }
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_download_settings') {
          return {
            library_root: '/music/library',
            staging_root: '/music/library/.staging',
          }
        }
        return []
      })

      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
            StatusBar: true,
            NowPlayingBar: true,
            ToastNotifications: true,
            CommandPalette: true,
            KeyboardShortcuts: true,
            HelpPanel: true,
            QuickActionsFab: true,
            OnboardingWizard: true,
          },
        },
      })

      const vm = wrapper.vm as any
      // App starts with showSplash = true
      expect(vm.showSplash).toBeDefined()

      // Allow async initializeApp to complete
      await flushPromises()

      // Upon completion, showSplash must be false and splashError null
      expect(vm.showSplash).toBe(false)
      expect(vm.splashError).toBeNull()
      expect(vm.splashProgress).toBe(100)
    })

    it('keeps showSplash = true and displays error when database health check fails', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'run_health_check') {
          return {
            database_ok: false,
            python_ok: true,
            ffmpeg_available: true,
            chromaprint_available: true,
            services_configured: [],
            errors: ['SQLite database is locked or corrupted'],
          }
        }
        return []
      })

      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
            StatusBar: true,
            NowPlayingBar: true,
            ToastNotifications: true,
            CommandPalette: true,
            KeyboardShortcuts: true,
            HelpPanel: true,
            QuickActionsFab: true,
            OnboardingWizard: true,
          },
        },
      })

      await flushPromises()

      const vm = wrapper.vm as any
      expect(vm.showSplash).toBe(true)
      expect(vm.splashError).toContain('Database error')
      expect(vm.splashError).toContain('SQLite database is locked or corrupted')

      // SplashScreen component displays the error message
      const splashCmp = wrapper.findComponent(SplashScreen)
      expect(splashCmp.exists()).toBe(true)
      expect(splashCmp.props('error')).toContain('Database error')
    })

    it('allows retryInitialization to recover and dismiss splash after fixing backend error', async () => {
      let isDbBroken = true

      mockInvoke((cmd) => {
        if (cmd === 'run_health_check') {
          if (isDbBroken) {
            return {
              database_ok: false,
              python_ok: true,
              ffmpeg_available: true,
              chromaprint_available: true,
              services_configured: [],
              errors: ['Connection refused'],
            }
          }
          return {
            database_ok: true,
            python_ok: true,
            ffmpeg_available: true,
            chromaprint_available: true,
            services_configured: ['tidal'],
            errors: [],
          }
        }
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_download_settings') return {}
        return []
      })

      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
            StatusBar: true,
            NowPlayingBar: true,
            ToastNotifications: true,
            CommandPalette: true,
            KeyboardShortcuts: true,
            HelpPanel: true,
            QuickActionsFab: true,
            OnboardingWizard: true,
          },
        },
      })

      await flushPromises()
      const vm = wrapper.vm as any
      expect(vm.showSplash).toBe(true)
      expect(vm.splashError).toBeTruthy()

      // "Fix" database
      isDbBroken = false

      // Trigger retry
      vm.retryInitialization()
      await flushPromises()

      // Splash must now be dismissed
      expect(vm.showSplash).toBe(false)
      expect(vm.splashError).toBeNull()
    })
  })
})

describe('PathSelector.vue Robust Filesystem & IPC Validation (TASK-64)', () => {
  beforeEach(() => {
    resetMocks()
    vi.restoreAllMocks()
  })

  it('rejects empty paths with validation message', async () => {
    const wrapper = mount(PathSelector, {
      props: {
        label: 'Music Library',
        modelValue: '',
      },
    })

    const vm = wrapper.vm as any
    const isValid = await vm.validatePath('')

    expect(isValid).toBe(false)
    expect(vm.validationStatus).toEqual({
      valid: false,
      message: 'Path is required',
    })
    await nextTick()
    expect(wrapper.text()).toContain('Path is required')
  })

  it('rejects paths exceeding 255 characters', async () => {
    const wrapper = mount(PathSelector, {
      props: {
        label: 'Music Library',
        modelValue: '/var/music',
      },
    })

    const vm = wrapper.vm as any
    const longPath = '/music/' + 'a'.repeat(260)
    const isValid = await vm.validatePath(longPath)

    expect(isValid).toBe(false)
    expect(vm.validationStatus).toEqual({
      valid: false,
      message: 'Path exceeds 255 characters',
    })
  })

  it('rejects relative path sequences without calling backend IPC', async () => {
    const invokeSpy = vi.fn()
    mockInvoke(invokeSpy)

    const wrapper = mount(PathSelector, {
      props: {
        label: 'Music Library',
        modelValue: '',
      },
    })

    const vm = wrapper.vm as any

    // Relative path #1
    let isValid = await vm.validatePath('../relative/music')
    expect(isValid).toBe(false)
    expect(vm.validationStatus).toEqual({
      valid: false,
      message: 'Path must be an absolute path',
    })

    // Relative path #2
    isValid = await vm.validatePath('./local_folder')
    expect(isValid).toBe(false)
    expect(vm.validationStatus).toEqual({
      valid: false,
      message: 'Path must be an absolute path',
    })

    // Relative path #3
    isValid = await vm.validatePath('just_a_subfolder')
    expect(isValid).toBe(false)
    expect(vm.validationStatus).toEqual({
      valid: false,
      message: 'Path must be an absolute path',
    })

    // No validate_directory_path IPC invocation for relative paths
    expect(invokeSpy).not.toHaveBeenCalledWith('validate_directory_path', expect.anything())
  })

  it('invokes Tauri IPC validate_directory_path for valid absolute paths and accepts valid directories', async () => {
    mockInvoke((cmd, args) => {
      if (cmd === 'validate_directory_path') {
        const path = (args as { path: string })?.path
        return {
          valid: true,
          exists: true,
          is_dir: true,
          is_writable: true,
          available_bytes: 50000000000,
          drive_mounted: true,
          canonical_path: path,
          error_message: null,
        }
      }
      return null
    })

    const wrapper = mount(PathSelector, {
      props: {
        label: 'Download Folder',
        modelValue: '/home/user/Music',
      },
    })

    const vm = wrapper.vm as any
    const isValid = await vm.validatePath('/home/user/Music')

    expect(isValid).toBe(true)
    expect(vm.validationStatus).toEqual({
      valid: true,
      message: 'Valid directory',
    })
    await nextTick()
    expect(wrapper.find('[data-testid="path-status"]').classes()).toContain('text-emerald-500')
  })

  it('displays backend error message and marks invalid when filesystem reports unmounted drive', async () => {
    mockInvoke((cmd, args) => {
      if (cmd === 'validate_directory_path') {
        return {
          valid: false,
          exists: false,
          is_dir: false,
          is_writable: false,
          available_bytes: 0,
          drive_mounted: false,
          canonical_path: (args as { path: string })?.path,
          error_message: 'Drive or volume for path is not mounted or accessible',
        }
      }
      return null
    })

    const wrapper = mount(PathSelector, {
      props: {
        label: 'SD Card Library',
        modelValue: 'E:\\Lossless',
      },
    })

    const vm = wrapper.vm as any
    const isValid = await vm.validatePath('E:\\Lossless')

    expect(isValid).toBe(false)
    expect(vm.validationStatus.valid).toBe(false)
    expect(vm.validationStatus.message).toBe('Drive or volume for path is not mounted or accessible')

    await nextTick()
    expect(wrapper.find('[data-testid="path-status"]').classes()).toContain('text-amber-500')
    expect(wrapper.find('[data-testid="path-error-msg"]').text()).toContain('Drive or volume for path is not mounted or accessible')
  })

  it('does not emit change event when handleBlur encounters an invalid path', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'validate_directory_path') {
        return {
          valid: false,
          exists: true,
          is_dir: true,
          is_writable: false,
          available_bytes: 10000,
          drive_mounted: true,
          canonical_path: '/readonly/path',
          error_message: 'Directory is not writable',
        }
      }
      return null
    })

    const wrapper = mount(PathSelector, {
      props: {
        label: 'Target Directory',
        modelValue: '/readonly/path',
      },
    })

    const input = wrapper.find('input[type="text"]')
    await input.trigger('blur')
    await flushPromises()

    // Since the path is not writable/valid, 'change' must NOT be emitted
    expect(wrapper.emitted('change')).toBeFalsy()
  })

  it('validates selected path from browse dialog before emitting update:modelValue and change', async () => {
    const viOpen = vi.mocked(open)
    viOpen.mockResolvedValueOnce('/media/flac/selected')

    mockInvoke((cmd, args) => {
      if (cmd === 'validate_directory_path') {
        return {
          valid: true,
          exists: true,
          is_dir: true,
          is_writable: true,
          available_bytes: 1000000000,
          drive_mounted: true,
          canonical_path: (args as { path: string })?.path,
          error_message: null,
        }
      }
      return null
    })

    const wrapper = mount(PathSelector, {
      props: {
        label: 'Storage Root',
        modelValue: '/media/flac/old',
      },
    })

    const browseBtn = wrapper.findAll('button').find(b => b.text().includes('Browse...'))!
    await browseBtn.trigger('click')
    await flushPromises()

    expect(wrapper.emitted('update:modelValue')).toBeTruthy()
    expect(wrapper.emitted('update:modelValue')![0]).toEqual(['/media/flac/selected'])
    expect(wrapper.emitted('change')).toBeTruthy()
    expect(wrapper.emitted('change')![0]).toEqual(['/media/flac/selected'])
  })

  it('skips validation when component is disabled', async () => {
    const invokeSpy = vi.fn()
    mockInvoke(invokeSpy)

    const wrapper = mount(PathSelector, {
      props: {
        label: 'System Database',
        modelValue: '/var/lib/syncify.db',
        disabled: true,
      },
    })

    const vm = wrapper.vm as any
    const isValid = await vm.validatePath('/var/lib/syncify.db')

    expect(isValid).toBe(true)
    expect(vm.validationStatus).toBeNull()
    expect(invokeSpy).not.toHaveBeenCalled()
  })
})
