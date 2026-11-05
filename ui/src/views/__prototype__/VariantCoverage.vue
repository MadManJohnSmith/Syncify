<script setup lang="ts">
import { computed, onMounted, reactive, ref, onUnmounted } from 'vue'
import {
  PROTO_NOW,
  PROTO_SERVICES,
  formatAbsolute,
  formatInt,
  formatPct,
  serviceFacts,
  type ServiceKey,
} from './accountsPrototypeData'
import ProtoBar from './ProtoBar.vue'
import ProtoButton from './ProtoButton.vue'
import ProtoChip from './ProtoChip.vue'
import ProtoMark from './ProtoMark.vue'

const services = ref(PROTO_SERVICES.map(service => ({ ...service, accounts: service.accounts.map(account => ({ ...account })) })))
const active = reactive<Record<ServiceKey, number>>({ spotify: 1, qobuz: 3, tidal: 7 })
const showAll = reactive<Record<string, boolean>>({})
const syncing = ref<ServiceKey | null>(null)
const toast = ref('')
let timer: number | undefined

const facts = computed(() =>
  services.value.map(service =>
    serviceFacts(
      {
        ...service,
        accounts: service.accounts.map(account => ({ ...account, isActive: account.id === active[service.key] })),
      },
      PROTO_NOW,
    ),
  ),
)

function visibleAccounts(service: ReturnType<typeof serviceFacts>) {
  return showAll[service.service.key] ? service.accounts : service.accounts.slice(0, 2)
}

function useAccount(serviceKey: ServiceKey, accountId: number) {
  active[serviceKey] = accountId
  const fact = facts.value.find(item => item.service.key === serviceKey)
  const account = fact?.accounts.find(item => item.account.id === accountId)
  toast.value = `Cuenta en uso: ${account?.label ?? ''}`
  window.clearTimeout(timer)
  timer = window.setTimeout(() => (toast.value = ''), 2200)
}

function sync(serviceKey: ServiceKey) {
  if (syncing.value) return
  syncing.value = serviceKey
  toast.value = `Escaneando favoritos de ${services.value.find(item => item.key === serviceKey)?.name}…`
  window.clearTimeout(timer)
  timer = window.setTimeout(() => {
    syncing.value = null
    toast.value = 'Escaneo simulado: no hay backend en el prototipo'
    timer = window.setTimeout(() => (toast.value = ''), 2600)
  }, 1200)
}

function scanAccount(serviceKey: ServiceKey, accountId: number) {
  useAccount(serviceKey, accountId)
  sync(serviceKey)
}

function toneFor(coverage: number | null, state: string): 'ok' | 'warn' | 'danger' | 'neutral' {
  if (state === 'never') return 'neutral'
  if (coverage == null) return 'warn'
  return coverage >= 0.9999 ? 'ok' : 'warn'
}

onMounted(() => {
  // El aviso global de atajos taparía las tarjetas en las capturas del prototipo.
  window.localStorage.setItem('syncify_seen_shortcut_hint', 'true')
})

onUnmounted(() => window.clearTimeout(timer))
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex items-start justify-between px-8 pb-5 pt-6">
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-2xl font-semibold tracking-tight text-ink">Connected Services</h1>
          <span class="rounded-full border border-dashed border-accent/40 px-2 py-0.5 text-[10px] uppercase tracking-[0.14em] text-accent">Variante A · Cobertura</span>
        </div>
        <p class="mt-1 max-w-2xl text-[13px] text-ink-2">
          Una cifra, un alcance. La biblioteca no se suma entre cuentas; cada escaneo se compara con lo que declara su proveedor.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <ProtoButton variant="ghost" icon="key">Credenciales</ProtoButton>
        <ProtoButton variant="primary" icon="add">Añadir conexión</ProtoButton>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-8 pb-28 custom-scrollbar">
      <div class="grid grid-cols-1 items-stretch gap-5 xl:grid-cols-3">
        <article
          v-for="fact in facts"
          :key="fact.service.key"
          class="flex min-h-[520px] flex-col rounded-2xl border border-line bg-surface/70 p-5"
        >
          <!-- Header -->
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 items-center gap-3">
              <ProtoMark :icon="fact.service.icon" :mark-class="fact.service.markClass" />
              <div class="min-w-0">
                <h2 class="text-[17px] font-semibold leading-tight text-ink">{{ fact.service.name }}</h2>
                <p class="mt-0.5 truncate text-[11px] text-ink-3" :title="fact.primary?.label">
                  {{ fact.primary?.account.displayName || fact.primary?.account.email || 'Sin cuenta' }}
                </p>
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <ProtoChip tone="ok" dot>Conectado</ProtoChip>
              <button class="rounded-md p-1 text-ink-3 hover:bg-elevated hover:text-ink" title="Más acciones" @click="toast = 'Menú de la tarjeta (prototipo)'">
                <span class="material-symbols-outlined text-[18px]">more_horiz</span>
              </button>
            </div>
          </div>

          <!-- Hero: one honest library number + the active account's coverage -->
          <div class="mt-4 rounded-xl border border-line/70 bg-elevated/25 p-3.5">
            <div class="flex items-start justify-between gap-3">
              <div>
                <p class="flex items-center gap-1 text-[10px] uppercase tracking-[0.14em] text-ink-3">
                  Favoritos en tu biblioteca
                  <span
                    class="material-symbols-outlined cursor-help text-[13px] text-ink-3"
                    title="Pistas únicas guardadas como favoritas en tu biblioteca. No se suman las cuentas: la misma pista en dos cuentas cuenta una vez."
                  >info</span>
                </p>
                <p class="mt-1 flex items-baseline gap-2">
                  <span class="text-[34px] font-semibold leading-none tracking-tight text-ink">{{ formatInt(fact.favoritesInLibrary) }}</span>
                  <span class="text-[11px] text-ink-3">pistas únicas</span>
                </p>
              </div>
              <ProtoChip :tone="fact.stateTone === 'ok' ? 'ok' : fact.stateTone === 'offline' ? 'danger' : 'warn'" dot>
                {{ fact.stateLabel }}
              </ProtoChip>
            </div>

            <template v-if="fact.primary?.snapshot">
              <div class="mt-3 flex items-end justify-between gap-3 text-[11px]">
                <span class="text-ink-2">
                  Escaneo de favoritos ·
                  <strong class="font-semibold text-ink">{{ formatInt(fact.primary.imported) }}</strong>
                  de {{ formatInt(fact.primary.reported) }} reportados
                </span>
                <span class="tabular-nums font-semibold text-ink">{{ formatPct(fact.primary.coverage) }}</span>
              </div>
              <ProtoBar class="mt-1.5" :value="fact.primary.coverage" :tone="toneFor(fact.primary.coverage, fact.primary.state)" />
              <div class="mt-1.5 flex items-center justify-between gap-2 text-[10px] text-ink-3">
                <span class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-[12px]">schedule</span>
                  <span :title="formatAbsolute(fact.primary.scannedAt)">{{ fact.primary.scannedRelative }}</span>
                </span>
                <span v-if="fact.primary.surplus" class="text-warn">+{{ formatInt(fact.primary.surplus) }} sobre lo reportado</span>
                <span v-else-if="fact.primary.missing" class="text-ink-3">faltan {{ formatInt(fact.primary.missing) }}</span>
                <span v-else class="text-ok">todo lo reportado está importado</span>
              </div>
            </template>
            <template v-else>
              <div class="mt-3 flex items-center justify-between gap-2">
                <p class="text-[11px] text-ink-3">Esta cuenta todavía no tiene un escaneo de favoritos.</p>
                <ProtoButton size="sm" variant="ghost" icon="play_arrow" @click="sync(fact.service.key)">Escanear</ProtoButton>
              </div>
            </template>
          </div>

          <!-- Secondary numbers, each with its own label -->
          <div class="mt-3 grid grid-cols-2 gap-2 border-t border-line/60 pt-3">
            <div class="rounded-lg bg-elevated/20 px-2.5 py-2">
              <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Biblioteca</p>
              <p class="mt-0.5 text-sm font-semibold text-ink">{{ formatInt(fact.tracksInLibrary) }} <span class="text-[10px] font-normal text-ink-3">pistas</span></p>
            </div>
            <div class="rounded-lg bg-elevated/20 px-2.5 py-2">
              <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Playlists</p>
              <p class="mt-0.5 text-sm font-semibold text-ink">{{ formatInt(fact.playlists) }} <span class="text-[10px] font-normal text-ink-3">listas</span></p>
            </div>
          </div>

          <!-- Accounts -->
          <div class="mt-4 flex min-h-0 flex-col">
            <div class="flex items-center justify-between">
              <p class="text-[11px] font-medium text-ink-2">
                Cuentas <span class="text-ink-3">· {{ fact.totalAccounts }}</span>
              </p>
              <button class="text-[11px] text-ink-3 hover:text-accent" @click="toast = 'Gestionar cuentas (prototipo)'">Gestionar</button>
            </div>

            <div class="mt-2 space-y-1.5">
              <div
                v-for="account in visibleAccounts(fact)"
                :key="account.account.id"
                class="group rounded-lg border px-2.5 py-2 transition-colors"
                :class="account.isActive ? 'border-accent/30 bg-accent/5' : 'border-line/50 bg-transparent hover:bg-elevated/30'"
              >
                <div class="flex items-center gap-2">
                  <button
                    class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold"
                    :class="account.isActive ? 'bg-accent/20 text-accent' : 'bg-elevated text-ink-3 hover:text-ink'"
                    :aria-label="account.isActive ? 'Cuenta en uso' : `Usar ${account.label}`"
                    :title="account.isActive ? 'Cuenta en uso' : 'Usar esta cuenta'"
                    @click="useAccount(fact.service.key, account.account.id)"
                  >
                    <span v-if="account.isActive" class="material-symbols-outlined text-[13px]">check</span>
                    <span v-else>{{ account.initial }}</span>
                  </button>
                  <div class="min-w-0 flex-1">
                    <p class="truncate text-[11.5px] font-medium leading-tight text-ink" :title="account.label">{{ account.label }}</p>
                  </div>
                  <ProtoChip v-if="account.isActive" tone="accent">En uso</ProtoChip>
                  <ProtoChip v-else-if="account.state === 'never'" tone="neutral" dot>Sin escanear</ProtoChip>
                  <ProtoChip v-else :tone="account.state === 'ok' ? 'ok' : 'warn'" dot>{{ account.stateLabel }}</ProtoChip>
                  <button class="rounded p-0.5 text-ink-3 opacity-0 transition-opacity hover:text-ink group-hover:opacity-100" title="Acciones de la cuenta" @click="toast = 'Acciones de cuenta (prototipo)'">
                    <span class="material-symbols-outlined text-[15px]">more_horiz</span>
                  </button>
                </div>

                <div v-if="account.snapshot" class="mt-1.5 flex items-center gap-2 pl-8">
                  <ProtoBar class="flex-1" :value="account.coverage" :tone="toneFor(account.coverage, account.state)" height="sm" />
                  <span class="tabular-nums text-[10px] text-ink-3">{{ formatInt(account.imported) }} / {{ formatInt(account.reported) }}</span>
                </div>
                <div v-else class="mt-1 flex items-center justify-between pl-8">
                  <span class="text-[10px] text-ink-3">El proveedor aún no devolvió datos</span>
                  <button class="text-[10px] font-medium text-accent hover:underline" @click="scanAccount(fact.service.key, account.account.id)">Escanear</button>
                </div>
              </div>
            </div>

            <button
              v-if="fact.accounts.length > 2"
              class="mt-1.5 self-start text-[10px] text-ink-3 hover:text-ink"
              @click="showAll[fact.service.key] = !showAll[fact.service.key]"
            >
              {{ showAll[fact.service.key] ? 'Ocultar cuentas' : `+ ${fact.accounts.length - 2} cuentas más` }}
            </button>

            <p v-if="fact.issues.length" class="mt-2 text-[10px] text-ink-3">{{ fact.issues.join(' · ') }}</p>
          </div>

          <!-- Footer pinned: every card has its actions at the same height -->
          <div class="mt-auto flex items-center gap-2 border-t border-line/60 pt-4">
            <ProtoButton
              variant="primary"
              class="flex-1"
              :icon="syncing === fact.service.key ? 'progress_activity' : 'sync'"
              :disabled="Boolean(syncing)"
              @click="sync(fact.service.key)"
            >
              {{ syncing === fact.service.key ? 'Sincronizando…' : 'Sincronizar' }}
            </ProtoButton>
            <ProtoButton variant="ghost" icon="person_add" @click="toast = 'Añadir cuenta (prototipo)'">Añadir</ProtoButton>
            <ProtoButton variant="ghost" icon="more_vert" aria-label="Más opciones" @click="toast = 'Desconectar, compras, credenciales…'">
              <span class="sr-only">Más opciones</span>
            </ProtoButton>
          </div>
        </article>
      </div>

      <p class="mt-4 text-[11px] leading-relaxed text-ink-3">
        <span class="material-symbols-outlined align-[-3px] text-[13px]">info</span>
        «Reportado» es lo que declara el proveedor para esa cuenta. «Importado» son las pistas de su último escaneo.
        La biblioteca muestra pistas únicas y por eso no coincide con ninguno de los dos: es otra unidad.
      </p>
    </div>

    <Transition name="fade">
      <div v-if="toast" class="fixed bottom-24 left-1/2 z-50 -translate-x-1/2 rounded-lg border border-line bg-shell px-3 py-2 text-xs text-ink shadow-xl">{{ toast }}</div>
    </Transition>
  </div>
</template>
