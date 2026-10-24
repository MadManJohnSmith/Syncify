<script setup lang="ts">
import { computed, reactive, ref, onUnmounted } from 'vue'
import {
  PROTO_NOW,
  PROTO_SERVICES,
  formatAbsolute,
  formatInt,
  formatPct,
  rawUtcStamp,
  serviceFacts,
  type ServiceKey,
} from './accountsPrototypeData'
import ProtoBar from './ProtoBar.vue'
import ProtoButton from './ProtoButton.vue'
import ProtoChip from './ProtoChip.vue'
import ProtoMark from './ProtoMark.vue'

const services = ref(PROTO_SERVICES.map(service => ({ ...service, accounts: service.accounts.map(account => ({ ...account })) })))
const active = reactive<Record<ServiceKey, number>>({ spotify: 1, qobuz: 3, tidal: 7 })
const selected = ref<ServiceKey>('tidal')
const expanded = ref<number | null>(7)
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

const selectedFact = computed(() => facts.value.find(fact => fact.service.key === selected.value) ?? facts.value[0])
const selectedAccount = computed(() => selectedFact.value.accounts.find(account => account.account.id === expanded.value) ?? selectedFact.value.primary)

const coverage = computed(() => selectedAccount.value?.coverage ?? null)
const donutColor = computed(() => (coverage.value == null ? 'var(--text-3)' : coverage.value >= 0.9999 ? 'var(--ok)' : 'var(--warn)'))
const donutStyle = computed(() => ({
  background: `conic-gradient(${donutColor.value} ${(coverage.value ?? 0) * 100}%, var(--surface-2) 0)`,
}))

function announce(label: string) {
  toast.value = `${label} (prototipo)`
  window.clearTimeout(timer)
  timer = window.setTimeout(() => (toast.value = ''), 2200)
}

function chooseService(key: ServiceKey) {
  selected.value = key
  const fact = facts.value.find(item => item.service.key === key)
  expanded.value = fact?.primary?.account.id ?? null
}

function useAccount(accountId: number) {
  active[selected.value] = accountId
  expanded.value = accountId
  announce('Cuenta en uso actualizada')
}

onUnmounted(() => window.clearTimeout(timer))
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex items-start justify-between px-8 pb-4 pt-6">
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-2xl font-semibold tracking-tight text-ink">Connected Services</h1>
          <span class="rounded-full border border-dashed border-accent/40 px-2 py-0.5 text-[10px] uppercase tracking-[0.14em] text-accent">Variante D · Detalle</span>
        </div>
        <p class="mt-1 max-w-2xl text-[13px] text-ink-2">Una conexión a la vez: la lista queda quieta y el detalle responde a la cuenta seleccionada.</p>
      </div>
      <ProtoButton variant="primary" icon="add">Añadir conexión</ProtoButton>
    </div>

    <div class="flex-1 overflow-hidden px-8 pb-24">
      <div class="grid h-full grid-cols-[248px_minmax(0,1fr)] gap-4">
        <!-- Master -->
        <aside class="flex min-h-0 flex-col">
          <div class="flex-1 space-y-2 overflow-y-auto pr-1 custom-scrollbar">
            <button
              v-for="fact in facts"
              :key="fact.service.key"
              class="w-full rounded-xl border p-3 text-left transition-colors"
              :class="selected === fact.service.key ? 'border-accent/40 bg-accent/[0.06]' : 'border-line bg-surface/50 hover:bg-surface'"
              @click="chooseService(fact.service.key)"
            >
              <div class="flex items-center gap-2.5">
                <ProtoMark :icon="fact.service.icon" :mark-class="fact.service.markClass" size="sm" />
                <div class="min-w-0">
                  <p class="text-[13px] font-semibold leading-tight text-ink">{{ fact.service.name }}</p>
                  <p class="truncate text-[10px] text-ink-3">{{ fact.primary?.label }}</p>
                </div>
                <span
                  class="ml-auto h-1.5 w-1.5 shrink-0 rounded-full"
                  :class="fact.stateTone === 'ok' ? 'bg-ok' : fact.stateTone === 'offline' ? 'bg-error' : 'bg-warn'"
                  :title="fact.stateLabel"
                />
              </div>

              <div class="mt-2.5 flex items-baseline justify-between">
                <p class="text-[11px] text-ink-3"><span class="text-[15px] font-semibold text-ink">{{ formatInt(fact.favoritesInLibrary) }}</span> favoritos</p>
                <p class="text-[10px] tabular-nums text-ink-3">{{ formatPct(fact.primaryCoverage) }}</p>
              </div>
              <ProtoBar class="mt-1" :value="fact.primaryCoverage" :tone="fact.primaryCoverage == null ? 'neutral' : fact.primaryCoverage >= 0.9999 ? 'ok' : 'warn'" height="sm" />
              <p class="mt-1.5 text-[10px] text-ink-3">{{ fact.scannedAccounts }}/{{ fact.totalAccounts }} cuentas escaneadas · {{ fact.service.library.playlists }} playlists</p>
            </button>
          </div>
          <button class="mt-2 rounded-xl border border-dashed border-line px-3 py-2.5 text-left text-[11px] text-ink-3 hover:border-accent/40 hover:text-ink" @click="announce('Añadir conexión')">
            <span class="material-symbols-outlined align-[-4px] text-[15px]">add</span> Otra conexión
          </button>
        </aside>

        <!-- Detail -->
        <section class="flex min-h-0 flex-col overflow-y-auto rounded-2xl border border-line bg-surface/60 custom-scrollbar">
          <div class="flex items-start justify-between gap-3 border-b border-line px-5 py-4">
            <div class="flex min-w-0 items-center gap-3">
              <ProtoMark :icon="selectedFact.service.icon" :mark-class="selectedFact.service.markClass" size="lg" />
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <h2 class="text-lg font-semibold leading-tight text-ink">{{ selectedFact.service.name }}</h2>
                  <ProtoChip tone="ok" dot>Conectado</ProtoChip>
                </div>
                <p class="mt-0.5 truncate text-[11px] text-ink-3">Cuenta en uso: {{ selectedFact.primary?.label ?? '—' }}</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <ProtoButton variant="primary" icon="sync" @click="announce(`Sincronizar ${selectedFact.service.name}`)">Sincronizar</ProtoButton>
              <ProtoButton variant="ghost" icon="person_add" @click="announce('Añadir cuenta')">Añadir</ProtoButton>
              <ProtoButton variant="ghost" icon="more_horiz" aria-label="Más acciones" @click="announce('Desconectar · Credenciales · Compras')"><span class="sr-only">Más</span></ProtoButton>
            </div>
          </div>

          <!-- Library stats -->
          <div class="grid grid-cols-4 gap-3 border-b border-line px-5 py-4">
            <div class="rounded-xl border border-line/70 bg-elevated/20 px-3 py-2.5">
              <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Biblioteca</p>
              <p class="mt-1 text-xl font-semibold leading-none text-ink">{{ formatInt(selectedFact.tracksInLibrary) }}</p>
              <p class="mt-1 text-[10px] text-ink-3">pistas de esta conexión</p>
            </div>
            <div class="rounded-xl border border-line/70 bg-elevated/20 px-3 py-2.5">
              <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Favoritos</p>
              <p class="mt-1 text-xl font-semibold leading-none text-ink">{{ formatInt(selectedFact.favoritesInLibrary) }}</p>
              <p class="mt-1 text-[10px] text-ink-3">pistas únicas en biblioteca</p>
            </div>
            <div class="rounded-xl border border-line/70 bg-elevated/20 px-3 py-2.5">
              <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Playlists</p>
              <p class="mt-1 text-xl font-semibold leading-none text-ink">{{ formatInt(selectedFact.playlists) }}</p>
              <p class="mt-1 text-[10px] text-ink-3">listas importadas</p>
            </div>
            <div class="flex items-center gap-3 rounded-xl border border-line/70 bg-elevated/20 px-3 py-2.5">
              <div class="relative h-12 w-12 shrink-0 rounded-full" :style="donutStyle">
                <div class="absolute inset-[4px] flex items-center justify-center rounded-full bg-surface text-[10px] font-semibold text-ink">
                  {{ coverage == null ? '—' : Math.round((coverage ?? 0) * 100) }}
                </div>
              </div>
              <div>
                <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Cobertura</p>
                <p class="mt-0.5 text-[13px] font-semibold leading-tight text-ink">{{ formatPct(coverage) }}</p>
                <p class="text-[10px] text-ink-3">del escaneo visible</p>
              </div>
            </div>
          </div>

          <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1.55fr)_minmax(240px,1fr)] gap-0">
            <!-- Accounts -->
            <div class="border-r border-line px-5 py-4">
              <div class="flex items-center justify-between">
                <h3 class="text-[12px] font-semibold text-ink">Cuentas <span class="font-normal text-ink-3">· {{ selectedFact.totalAccounts }}</span></h3>
                <span class="text-[10px] text-ink-3">Marca el círculo para cambiar la cuenta en uso</span>
              </div>

              <div class="mt-2.5 space-y-1.5">
                <div
                  v-for="account in selectedFact.accounts"
                  :key="account.account.id"
                  class="rounded-lg border transition-colors"
                  :class="[
                    expanded === account.account.id ? 'border-accent/30 bg-accent/[0.04]' : 'border-line/50 hover:bg-elevated/25',
                    account.isActive ? 'ring-0' : '',
                  ]"
                >
                  <div class="flex cursor-pointer items-center gap-2.5 px-2.5 py-2" @click="expanded = expanded === account.account.id ? null : account.account.id">
                    <button
                      class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border"
                      :class="account.isActive ? 'border-accent bg-accent text-on-accent' : 'border-line-strong text-transparent hover:border-accent'"
                      :aria-label="`Usar ${account.label}`"
                      :title="account.isActive ? 'Cuenta en uso' : 'Usar esta cuenta'"
                      @click.stop="useAccount(account.account.id)"
                    >
                      <span v-if="account.isActive" class="material-symbols-outlined text-[12px]">check</span>
                    </button>
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-1.5">
                        <p class="truncate text-[12px] font-medium text-ink" :title="account.label">{{ account.label }}</p>
                        <ProtoChip v-if="account.isActive" tone="accent">En uso</ProtoChip>
                      </div>
                      <p class="truncate text-[10px] text-ink-3">{{ account.credentialLabel }}</p>
                    </div>
                    <div class="hidden w-28 text-right sm:block">
                      <p class="text-[10px] tabular-nums text-ink-2">{{ account.reported == null ? '—' : `${formatInt(account.imported)} / ${formatInt(account.reported)}` }}</p>
                      <ProtoBar class="mt-1" :value="account.coverage" :tone="account.state === 'ok' ? 'ok' : account.state === 'never' ? 'neutral' : 'warn'" height="sm" />
                    </div>
                    <ProtoChip :tone="account.state === 'ok' ? 'ok' : account.state === 'never' ? 'neutral' : 'warn'" dot>{{ account.stateLabel }}</ProtoChip>
                    <span class="material-symbols-outlined text-[16px] text-ink-3">{{ expanded === account.account.id ? 'expand_less' : 'expand_more' }}</span>
                  </div>

                  <div v-if="expanded === account.account.id" class="border-t border-line/60 px-3 py-2.5">
                    <dl class="grid grid-cols-2 gap-x-4 gap-y-2 text-[10px] sm:grid-cols-4">
                      <div>
                        <dt class="text-ink-3">Reportados</dt>
                        <dd class="text-[12px] font-semibold tabular-nums text-ink">{{ account.reported == null ? '—' : formatInt(account.reported) }}</dd>
                      </div>
                      <div>
                        <dt class="text-ink-3">Importados</dt>
                        <dd class="text-[12px] font-semibold tabular-nums text-ink">{{ account.imported == null ? '—' : formatInt(account.imported) }}</dd>
                      </div>
                      <div>
                        <dt class="text-ink-3">Faltan</dt>
                        <dd class="text-[12px] font-semibold tabular-nums" :class="account.missing ? 'text-warn' : 'text-ink'">{{ account.missing == null ? '—' : formatInt(account.missing) }}</dd>
                      </div>
                      <div>
                        <dt class="text-ink-3">Completitud</dt>
                        <dd class="text-[12px] font-semibold text-ink">{{ account.completeness === 'partial' ? 'Parcial' : account.completeness === 'complete' ? 'Completa' : 'Sin datos' }}</dd>
                      </div>
                    </dl>
                    <div class="mt-2.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-[10px] text-ink-3">
                      <span class="inline-flex items-center gap-1">
                        <span class="material-symbols-outlined text-[12px]">schedule</span>
                        {{ account.scannedAt ? `${account.scannedAbsolute} · ${account.scannedRelative}` : 'Sin escaneo' }}
                      </span>
                      <span v-if="account.scannedAt" class="font-mono text-[9px] text-ink-3/80" title="Valor crudo que muestra la pantalla actual (UTC, sin zona)">UTC {{ rawUtcStamp(account.scannedAt) }}</span>
                      <span class="ml-auto flex gap-1.5">
                        <ProtoButton size="sm" variant="ghost" icon="play_arrow" @click="announce('Escanear cuenta')">Escanear</ProtoButton>
                        <ProtoButton size="sm" variant="danger" icon="link_off" @click="announce('Desconectar cuenta')">Desconectar</ProtoButton>
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Context -->
            <div class="px-4 py-4">
              <div class="rounded-xl border border-line/70 bg-surface p-3">
                <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Último movimiento</p>
                <p class="mt-1 text-[12px] font-medium text-ink">{{ selectedFact.primary?.scannedAt ? selectedFact.primary.scannedRelative : 'Sin escaneos' }}</p>
                <p class="mt-0.5 text-[10px] text-ink-3">{{ selectedFact.primary?.scannedAt ? selectedFact.primary.scannedAbsolute : 'Ejecuta un escaneo para empezar' }}</p>
                <ProtoBar class="mt-2.5" :value="selectedFact.primaryCoverage" :tone="selectedFact.primaryCoverage == null ? 'neutral' : selectedFact.primaryCoverage >= 0.9999 ? 'ok' : 'warn'" height="sm" />
                <p class="mt-1 text-[10px] text-ink-3">{{ formatPct(selectedFact.primaryCoverage) }} de lo reportado por la cuenta en uso</p>
              </div>

              <details class="group mt-3 rounded-xl border border-line/70 bg-surface p-3" open>
                <summary class="flex cursor-pointer list-none items-center gap-1.5 text-[11px] font-medium text-ink">
                  <span class="material-symbols-outlined text-[15px] text-accent">calculate</span>
                  Cómo se calcula
                  <span class="material-symbols-outlined ml-auto text-[15px] text-ink-3 group-open:rotate-180">expand_more</span>
                </summary>
                <ul class="mt-2 space-y-2 text-[10px] leading-relaxed text-ink-3">
                  <li><strong class="text-ink-2">Cobertura</strong> = importados ÷ reportados. Si faltan páginas, no llega a 100 %.</li>
                  <li><strong class="text-ink-2">Favoritos</strong> = pistas únicas de la biblioteca; no se suman cuentas.</li>
                  <li><strong class="text-ink-2">Reportado</strong> lo dice el proveedor; <strong class="text-ink-2">importado</strong> lo devuelve el escaneo.</li>
                  <li><strong class="text-ink-2">Sin escanear</strong> no es 0: es ausencia de datos.</li>
                </ul>
              </details>

              <div class="mt-3 rounded-xl border border-line/70 bg-surface p-3">
                <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Cuentas de esta conexión</p>
                <p class="mt-1 text-[11px] text-ink-2">{{ selectedFact.scannedAccounts }} con escaneo · {{ selectedFact.missingAccounts }} sin datos · {{ selectedFact.partialAccounts }} incompletas</p>
                <button class="mt-2 text-[10px] font-medium text-accent hover:underline" @click="announce('Gestionar cuentas')">Gestionar todas las cuentas</button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>

    <Transition name="fade">
      <div v-if="toast" class="fixed bottom-24 left-1/2 z-50 -translate-x-1/2 rounded-lg border border-line bg-shell px-3 py-2 text-xs text-ink shadow-xl">{{ toast }}</div>
    </Transition>
  </div>
</template>
