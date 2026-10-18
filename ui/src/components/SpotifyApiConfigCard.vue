<template>
  <div class="spotify-api-config rounded-xl border bg-white dark:bg-surface-dark p-6"
       :class="configured ? 'border-gray-200 dark:border-border-dark' : 'border-amber-500/50'">

    <!-- Header -->
    <div class="flex items-center gap-4 mb-4">
      <div class="h-12 w-12 rounded-xl flex items-center justify-center text-white text-2xl font-bold"
           style="background: linear-gradient(135deg, #1DB954, #169c46)">
        <span class="material-symbols-outlined text-[26px]">graphic_eq</span>
      </div>
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2 flex-wrap">
          <h3 class="font-semibold text-gray-900 dark:text-white">Spotify — app credentials</h3>
          <span data-testid="spotify-api-status" :class="[
            'px-2 py-0.5 rounded-full text-xs font-medium flex items-center gap-1',
            configured ? 'bg-success/10 text-success' : 'bg-amber-500/10 text-amber-500'
          ]">
            <span class="material-symbols-outlined text-[14px]">{{ configured ? 'check_circle' : 'warning' }}</span>
            {{ configured ? 'Configured' : 'Not configured' }}
          </span>
        </div>
        <p class="text-sm text-text-secondary mt-0.5">
          Required to sign in with your Spotify account from the installed app.
        </p>
      </div>
    </div>

    <!-- Not-configured hint -->
    <div v-if="!configured" class="mb-4 p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg">
      <p class="text-xs text-amber-600 dark:text-amber-400 leading-relaxed">
        Without these credentials, Spotify's "Connect" button fails with
        <code class="font-mono">SPOTIFY_CLIENT_ID not set</code>.
        They are free and take 2 minutes to get by following the steps below.
      </p>
    </div>

    <!-- Error banner (load / save / clear failures) -->
    <div
      v-if="error !== ''"
      data-testid="spotify-api-error"
      role="alert"
      class="mb-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 text-sm flex items-start gap-2"
    >
      <span class="material-symbols-outlined text-[18px] mt-0.5">error</span>
      <span class="flex-1">{{ error }}</span>
      <button
        type="button"
        aria-label="Dismiss error"
        class="shrink-0 text-red-500 hover:text-red-700 transition-colors"
        @click="error = ''"
      >
        <span class="material-symbols-outlined text-[16px]">close</span>
      </button>
    </div>

    <!-- Success feedback (transient) -->
    <div
      v-if="successMessage !== ''"
      data-testid="spotify-api-success"
      role="status"
      class="mb-4 p-3 rounded-lg bg-success/10 border border-success/20 text-success text-sm flex items-center gap-2"
    >
      <span class="material-symbols-outlined text-[18px]">check_circle</span>
      {{ successMessage }}
    </div>

    <!-- Form -->
    <div class="space-y-3">
      <div>
        <label class="block text-xs font-medium text-text-secondary uppercase mb-1" for="spotify-client-id">
          Client ID
        </label>
        <input
          id="spotify-client-id"
          v-model="clientId"
          type="text"
          autocomplete="off"
          spellcheck="false"
          placeholder="Paste the Client ID from your Spotify dashboard here"
          class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/40 text-gray-900 dark:text-white text-sm focus:outline-none focus:ring-2 focus:ring-primary/40 font-mono"
        />
      </div>

      <div>
        <label class="block text-xs font-medium text-text-secondary uppercase mb-1" for="spotify-client-secret">
          Client Secret
        </label>
        <div class="relative">
          <input
            id="spotify-client-secret"
            v-model="clientSecret"
            :type="showSecret ? 'text' : 'password'"
            autocomplete="new-password"
            spellcheck="false"
            :placeholder="secretMask !== '' ? secretMask : 'Paste the Client Secret here'"
            class="w-full px-3 py-2 pr-20 rounded-lg border border-gray-300 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/40 text-gray-900 dark:text-white text-sm focus:outline-none focus:ring-2 focus:ring-primary/40 font-mono"
          />
          <button
            type="button"
            @click="showSecret = !showSecret"
            class="absolute right-2 top-1/2 -translate-y-1/2 px-2 py-1 text-xs text-text-secondary hover:text-primary transition-colors flex items-center gap-1"
          >
            <span class="material-symbols-outlined text-[16px]">{{ showSecret ? 'visibility_off' : 'visibility' }}</span>
            {{ showSecret ? 'Hide' : 'Show' }}
          </button>
        </div>
        <p v-if="secretMask !== ''" class="text-[11px] text-text-secondary mt-1">
          Saved: <span class="font-mono">{{ secretMask }}</span> — leave the field empty to keep it.
        </p>
      </div>

      <div>
        <label class="block text-xs font-medium text-text-secondary uppercase mb-1" for="spotify-redirect-uri">
          Redirect URI (registered in your Spotify app)
        </label>
        <div class="flex items-center gap-2">
          <input
            id="spotify-redirect-uri"
            :value="redirectUri"
            type="text"
            readonly
            data-testid="spotify-redirect-uri"
            class="flex-1 px-3 py-2 rounded-lg border border-gray-200 dark:border-border-dark bg-gray-100 dark:bg-surface-highlight/60 text-gray-700 dark:text-gray-300 text-sm font-mono cursor-default select-all"
          />
          <button
            type="button"
            @click="copyRedirectUri"
            class="px-3 py-2 rounded-lg border border-gray-300 dark:border-border-dark text-sm text-gray-700 dark:text-gray-300 hover:border-primary hover:text-primary transition-colors flex items-center gap-1 shrink-0"
          >
            <span class="material-symbols-outlined text-[16px]">{{ redirectCopied ? 'check' : 'content_copy' }}</span>
            {{ redirectCopied ? 'Copied' : 'Copy' }}
          </button>
        </div>
      </div>

      <div class="flex items-center gap-2 pt-1 flex-wrap">
        <button
          type="button"
          data-testid="spotify-api-save"
          @click="save"
          :disabled="saving || !canSave"
          :class="[
            'px-4 py-2 rounded-lg text-sm font-semibold transition-colors flex items-center gap-2',
            saving || !canSave
              ? 'bg-primary/50 text-on-accent/70 cursor-not-allowed'
              : 'bg-primary text-on-accent hover:bg-primary-hover shadow-sm'
          ]"
        >
          <span v-if="saving" class="material-symbols-outlined text-[18px] animate-spin">sync</span>
          Save credentials
        </button>
        <button
          v-if="configured"
          type="button"
          @click="clearCredentials"
          :disabled="saving"
          class="px-4 py-2 rounded-lg text-sm text-error hover:bg-error/10 transition-colors disabled:opacity-50"
        >
          Clear credentials
        </button>
        <button
          type="button"
          @click="reload"
          :disabled="loading"
          class="px-3 py-2 text-sm text-text-secondary hover:text-primary transition-colors disabled:opacity-50"
        >
          Refresh status
        </button>
      </div>
      <p v-if="!canSave" class="text-xs text-text-secondary pt-1">
        Enter the Client ID{{ configured ? '' : ' and Client Secret' }} to enable saving.
      </p>
    </div>

    <!-- Onboarding instructions (collapsible) -->
    <div class="mt-4 border-t border-gray-200 dark:border-border-dark pt-3">
      <button
        type="button"
        data-testid="spotify-instructions-toggle"
        @click="instructionsOpen = !instructionsOpen"
        class="w-full flex items-center justify-between text-left group"
      >
        <span class="flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-white group-hover:text-primary transition-colors">
          <span class="material-symbols-outlined text-[18px] text-primary">help</span>
          How do I get my Client ID and Client Secret? (2 minutes)
        </span>
        <span class="material-symbols-outlined text-[20px] text-text-secondary transition-transform"
              :class="{ 'rotate-180': instructionsOpen }">expand_more</span>
      </button>

      <ol v-if="instructionsOpen" data-testid="spotify-instructions"
          class="mt-3 space-y-3 list-none pl-1 text-sm text-gray-700 dark:text-gray-300">
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">1</span>
          <p>
            Go to
            <a href="https://developer.spotify.com/dashboard" target="_blank" rel="noopener noreferrer"
               class="text-primary underline underline-offset-2 break-all">developer.spotify.com/dashboard</a>
            and sign in <strong>with your Spotify account</strong> (a regular music account).
          </p>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">2</span>
          <p>
            Click <strong>"Create app"</strong>. The name and description are up to you
            (for example <em>"Syncify"</em>). Accept the terms and create the app.
          </p>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">3</span>
          <p>
            Under <strong>"Redirect URI"</strong> register EXACTLY this address
            (copy it with the button above and paste it there):
          </p>
        </li>
        <li class="-mt-1.5 pl-9">
          <code data-testid="spotify-instructions-redirect-uri"
                class="block px-3 py-2 rounded-lg bg-gray-100 dark:bg-surface-highlight/60 border border-gray-200 dark:border-border-dark text-xs font-mono break-all select-all">{{ redirectUri }}</code>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">4</span>
          <p>
            Under <strong>"Which API/SDKs are you planning to use?"</strong> check
            <strong>"Web API"</strong>. ("Web Playback SDK" is not needed for Syncify;
            check it only if you also want to use this same dashboard for other projects.)
            Then click <strong>"Save"</strong>.
          </p>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">5</span>
          <p>
            Inside your app, go to <strong>"Settings"</strong> and copy the
            <strong>"Client ID"</strong>; then click <strong>"View client secret"</strong> and copy the secret.
            Paste them into their fields above and save. Done: you can now click
            <strong>"Connect"</strong> on the Spotify card.
          </p>
        </li>
        <li class="pl-9 pt-1">
          <p class="text-xs text-text-secondary leading-relaxed">
            🔒 The Client Secret is stored <strong>encrypted</strong> in the app's local database
            and is never shown in full again (only its last 4 characters).
          </p>
        </li>
      </ol>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  getSpotifyApiConfig,
  saveSpotifyApiConfig,
  SPOTIFY_DEFAULT_REDIRECT_URI,
  type SpotifyApiConfig,
} from '@/api/accounts'

const emit = defineEmits<{
  /** Fired after credentials were saved or cleared successfully. */
  (e: 'saved', configured: boolean): void
}>()

const loading = ref(false)
const saving = ref(false)
const configured = ref(false)
const config = ref<SpotifyApiConfig | null>(null)

// User-visible error banner (load / validation / save / clear failures).
const error = ref('')
// Transient in-card success feedback (complements the parent's toast).
const successMessage = ref('')
let successTimer: ReturnType<typeof setTimeout> | null = null

// Masked secret as stored server-side ('' when not configured).
const secretMask = computed(() => config.value?.secretMask ?? '')

// Form state
const clientId = ref('')
const clientSecret = ref('')
const showSecret = ref(false)
const instructionsOpen = ref(false)
const redirectUri = ref(SPOTIFY_DEFAULT_REDIRECT_URI)
const redirectCopied = ref(false)

const canSave = computed(() => {
  const idOk = clientId.value.trim() !== ''
  // With a stored secret, an empty field means "keep it".
  const secretOk = clientSecret.value.trim() !== '' || configured.value
  return idOk && secretOk
})

/** Canonical error extraction (same pattern as OnboardingWizard/useAccounts). */
function toUserMessage(e: unknown, fallback: string): string {
  if (e instanceof Error && e.message.trim() !== '') return e.message
  return fallback
}

function showSuccess(message: string) {
  successMessage.value = message
  if (successTimer !== null) clearTimeout(successTimer)
  successTimer = setTimeout(() => { successMessage.value = '' }, 2500)
}

/** Load current credentials status. Returns true on success (error banner set on failure). */
async function reload(): Promise<boolean> {
  loading.value = true
  try {
    const cfg = await getSpotifyApiConfig()
    config.value = cfg
    configured.value = cfg.configured
    clientId.value = cfg.clientId
    redirectUri.value = cfg.redirectUri
    error.value = ''
    if (!cfg.configured) {
      instructionsOpen.value = true
    }
    return true
  } catch (e) {
    console.error('Could not load the Spotify configuration:', e)
    error.value = toUserMessage(
      e,
      'Could not read the Spotify credentials status from the database. Click "Refresh status" to try again.'
    )
    return false
  } finally {
    loading.value = false
  }
}

async function save() {
  if (saving.value) return
  // Pre-save validation: never invoke the backend with incomplete data.
  if (clientId.value.trim() === '') {
    error.value = 'The Client ID is required. Copy it from the Spotify dashboard (Settings → Client ID).'
    return
  }
  if (clientSecret.value.trim() === '' && !configured.value) {
    error.value = 'The Client Secret is required the first time. Copy it from the Spotify dashboard (View client secret).'
    return
  }
  saving.value = true
  error.value = ''
  try {
    const trimmed = clientSecret.value.trim()
    await saveSpotifyApiConfig(
      clientId.value,
      trimmed === '' ? null : trimmed,
      redirectUri.value
    )
    clientSecret.value = ''
    emit('saved', true)
    const reloaded = await reload()
    if (reloaded) {
      showSuccess('Credentials saved successfully.')
    }
  } catch (e) {
    console.error('Could not save the credentials:', e)
    successMessage.value = ''
    error.value = toUserMessage(
      e,
      'Could not save the credentials. Check the values and try again.'
    )
  } finally {
    saving.value = false
  }
}

async function clearCredentials() {
  if (saving.value) return
  saving.value = true
  error.value = ''
  try {
    await saveSpotifyApiConfig('', '', '')
    clientId.value = ''
    clientSecret.value = ''
    emit('saved', false)
    const reloaded = await reload()
    if (reloaded) {
      showSuccess('Credentials cleared.')
    }
  } catch (e) {
    console.error('Could not clear the credentials:', e)
    successMessage.value = ''
    error.value = toUserMessage(
      e,
      'Could not clear the credentials. Try again.'
    )
  } finally {
    saving.value = false
  }
}

async function copyRedirectUri() {
  try {
    await navigator.clipboard.writeText(redirectUri.value)
  } catch {
    // Clipboard API can be unavailable inside some WebViews: fall back.
    const textarea = document.createElement('textarea')
    textarea.value = redirectUri.value
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    try { document.execCommand('copy') } finally { document.body.removeChild(textarea) }
  }
  redirectCopied.value = true
  setTimeout(() => { redirectCopied.value = false }, 2000)
}

onMounted(reload)

onBeforeUnmount(() => {
  if (successTimer !== null) clearTimeout(successTimer)
})

defineExpose({
  /** Open the collapsible instructions panel programmatically. */
  openInstructions() {
    instructionsOpen.value = true
  },
})
</script>
