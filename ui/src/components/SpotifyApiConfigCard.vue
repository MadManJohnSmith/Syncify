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
          <h3 class="font-semibold text-gray-900 dark:text-white">Spotify — credenciales de la aplicación</h3>
          <span data-testid="spotify-api-status" :class="[
            'px-2 py-0.5 rounded-full text-xs font-medium flex items-center gap-1',
            configured ? 'bg-success/10 text-success' : 'bg-amber-500/10 text-amber-500'
          ]">
            <span class="material-symbols-outlined text-[14px]">{{ configured ? 'check_circle' : 'warning' }}</span>
            {{ configured ? 'Configurado' : 'No configurado' }}
          </span>
        </div>
        <p class="text-sm text-text-secondary mt-0.5">
          Necesarias para iniciar sesión con tu cuenta de Spotify desde la app instalada.
        </p>
      </div>
    </div>

    <!-- Not-configured hint -->
    <div v-if="!configured && !saving" class="mb-4 p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg">
      <p class="text-xs text-amber-600 dark:text-amber-400 leading-relaxed">
        Sin estas credenciales el botón «Conectar» de Spotify falla con
        <code class="font-mono">SPOTIFY_CLIENT_ID not set</code>.
        Son gratis y se obtienen en 2 minutos siguiendo los pasos de más abajo.
      </p>
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
          placeholder="Pega aquí el Client ID del dashboard de Spotify"
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
            :placeholder="secretMask !== '' ? secretMask : 'Pega aquí el Client Secret'"
            class="w-full px-3 py-2 pr-20 rounded-lg border border-gray-300 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/40 text-gray-900 dark:text-white text-sm focus:outline-none focus:ring-2 focus:ring-primary/40 font-mono"
          />
          <button
            type="button"
            @click="showSecret = !showSecret"
            class="absolute right-2 top-1/2 -translate-y-1/2 px-2 py-1 text-xs text-text-secondary hover:text-primary transition-colors flex items-center gap-1"
          >
            <span class="material-symbols-outlined text-[16px]">{{ showSecret ? 'visibility_off' : 'visibility' }}</span>
            {{ showSecret ? 'Ocultar' : 'Ver' }}
          </button>
        </div>
        <p v-if="secretMask !== ''" class="text-[11px] text-text-secondary mt-1">
          Guardado: <span class="font-mono">{{ secretMask }}</span> — deja el campo vacío para conservarlo.
        </p>
      </div>

      <div>
        <label class="block text-xs font-medium text-text-secondary uppercase mb-1" for="spotify-redirect-uri">
          Redirect URI (registrada en tu app de Spotify)
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
            {{ redirectCopied ? 'Copiada' : 'Copiar' }}
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
              ? 'bg-primary/50 text-white/70 cursor-not-allowed'
              : 'bg-primary text-white hover:bg-primary-hover shadow-sm'
          ]"
        >
          <span v-if="saving" class="material-symbols-outlined text-[18px] animate-spin">sync</span>
          Guardar credenciales
        </button>
        <button
          v-if="configured"
          type="button"
          @click="clearCredentials"
          :disabled="saving"
          class="px-4 py-2 rounded-lg text-sm text-error hover:bg-error/10 transition-colors disabled:opacity-50"
        >
          Borrar credenciales
        </button>
        <button
          type="button"
          @click="reload"
          :disabled="loading"
          class="px-3 py-2 text-sm text-text-secondary hover:text-primary transition-colors disabled:opacity-50"
        >
          Actualizar estado
        </button>
      </div>
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
          ¿Cómo consigo mis Client ID y Client Secret? (2 minutos)
        </span>
        <span class="material-symbols-outlined text-[20px] text-text-secondary transition-transform"
              :class="{ 'rotate-180': instructionsOpen }">expand_more</span>
      </button>

      <ol v-if="instructionsOpen" data-testid="spotify-instructions"
          class="mt-3 space-y-3 list-none pl-1 text-sm text-gray-700 dark:text-gray-300">
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">1</span>
          <p>
            Entra en
            <a href="https://developer.spotify.com/dashboard" target="_blank" rel="noopener noreferrer"
               class="text-primary underline underline-offset-2 break-all">developer.spotify.com/dashboard</a>
            e inicia sesión <strong>con tu cuenta de Spotify</strong> (la normal, de música).
          </p>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">2</span>
          <p>
            Pulsa <strong>«Create app»</strong>. El nombre y la descripción son libres
            (por ejemplo <em>«Syncify»</em>). Acepta los términos y crea la app.
          </p>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">3</span>
          <p>
            En <strong>«Redirect URI»</strong> registra EXACTAMENTE esta dirección
            (cópiala con el botón de arriba y pégala ahí):
          </p>
        </li>
        <li class="-mt-1.5 pl-9">
          <code data-testid="spotify-instructions-redirect-uri"
                class="block px-3 py-2 rounded-lg bg-gray-100 dark:bg-surface-highlight/60 border border-gray-200 dark:border-border-dark text-xs font-mono break-all select-all">{{ redirectUri }}</code>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">4</span>
          <p>
            En <strong>«Which API/SDKs are you planning to use?»</strong> marca
            <strong>«Web API»</strong>. («Web Playback SDK» no es necesaria para Syncify;
            márcala solo si además quieres usar este mismo dashboard para otros proyectos.)
            Después pulsa <strong>«Save»</strong>.
          </p>
        </li>
        <li class="flex gap-3">
          <span class="shrink-0 h-6 w-6 rounded-full bg-primary/10 text-primary text-xs font-bold flex items-center justify-center mt-0.5">5</span>
          <p>
            Dentro de tu app, entra en <strong>«Settings»</strong> y copia
            <strong>«Client ID»</strong>; luego pulsa <strong>«View client secret»</strong> y copia el secret.
            Pégalos arriba en sus campos y guarda. Listo: ya puedes pulsar
            <strong>«Conectar»</strong> en la tarjeta de Spotify.
          </p>
        </li>
        <li class="pl-9 pt-1">
          <p class="text-xs text-text-secondary leading-relaxed">
            🔒 El Client Secret se guarda <strong>cifrado</strong> en la base de datos local de la app
            y nunca se vuelve a mostrar completo (solo sus últimos 4 caracteres).
          </p>
        </li>
      </ol>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
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

async function reload() {
  loading.value = true
  try {
    const cfg = await getSpotifyApiConfig()
    config.value = cfg
    configured.value = cfg.configured
    clientId.value = cfg.clientId
    redirectUri.value = cfg.redirectUri
    if (!cfg.configured) {
      instructionsOpen.value = true
    }
  } catch (e) {
    console.error('No se pudo cargar la configuración de Spotify:', e)
  } finally {
    loading.value = false
  }
}

async function save() {
  saving.value = true
  try {
    const trimmed = clientSecret.value.trim()
    await saveSpotifyApiConfig(
      clientId.value,
      trimmed === '' ? null : trimmed,
      redirectUri.value
    )
    clientSecret.value = ''
    await reload()
    emit('saved', true)
  } catch (e) {
    console.error('No se pudieron guardar las credenciales:', e)
    throw e
  } finally {
    saving.value = false
  }
}

async function clearCredentials() {
  saving.value = true
  try {
    await saveSpotifyApiConfig('', '', '')
    clientId.value = ''
    clientSecret.value = ''
    await reload()
    emit('saved', false)
  } catch (e) {
    console.error('No se pudieron borrar las credenciales:', e)
    throw e
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

defineExpose({
  /** Open the collapsible instructions panel programmatically. */
  openInstructions() {
    instructionsOpen.value = true
  },
})
</script>
