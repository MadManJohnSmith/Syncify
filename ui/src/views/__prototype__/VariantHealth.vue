<script setup lang="ts">
import { computed, reactive, ref, onUnmounted } from 'vue'
import {
  PROTO_NOW,
  PROTO_SERVICES,
  formatAbsolute,
  formatInt,
  formatPct,
  globalFacts,
  serviceFacts,
  type ServiceFacts,
  type ServiceKey,
} from './accountsPrototypeData'
import ProtoBar from './ProtoBar.vue'
import ProtoButton from './ProtoButton.vue'
import ProtoChip from './ProtoChip.vue'
import ProtoMark from './ProtoMark.vue'

interface AttentionItem {
  id: string
  service: ServiceFacts
  tone: 'warn' | 'danger' | 'info'
  icon: string
  title: string
  detail: string
  action: string
}

const services = ref(PROTO_SERVICES.map(service => ({ ...service, accounts: service.accounts.map(account => ({ ...account })) })))
const active = reactive<Record<ServiceKey, number>>({ spotify: 1, qobuz: 3, tidal: 7 })
const expanded = reactive<Record<string, boolean>>({ tidal: true })
const toast = ref('')
let timer: number | undefined

const facts = computed<ServiceFacts[]>(() =>
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

const totals = computed(() => globalFacts(facts.value))

const attention = computed<AttentionItem[]>(() => {
  const items: AttentionItem[] = []
  for (const service of facts.value) {
    if (service.missingAccounts > 0) {
      items.push({
        id: `${service.service.key}-missing`,
        service,
        tone: 'warn',
        icon: 'cloud_off',
        title: `${service.missingAccounts} ${service.missingAccounts === 1 ? 'cuenta sin escanear' : 'cuentas sin escanear'} en ${service.service.name}`,
        detail: 'El proveedor no devolvió favoritos para esas cuentas. No significa que estén vacías.',
        action: 'Escanear',
      })
    }
    if (service.partialAccounts > 0) {
      const partials = service.accounts.filter(account => account.state === 'partial')
      const missing = partials.reduce((sum, account) => sum + (account.missing ?? 0), 0)
      items.push({
        id: `${service.service.key}-partial`,
        service,
        tone: 'warn',
        icon: 'pending',
        title: `${service.partialAccounts} ${service.partialAccounts === 1 ? 'escaneo incompleto' : 'escaneos incompletos'} en ${service.service.name}`,
        detail: missing
          ? `Faltan ${formatInt(missing)} favoritos de los reportados. Reintentar el escaneo suele completarlos.`
          : 'El proveedor no entregó todas las páginas; puede requerir un reintento.',
        action: 'Reintentar',
      })
    }
    if (service.staleAccounts > 0) {
      items.push({
        id: `${service.service.key}-stale`,
        service,
        tone: 'info',
        icon: 'schedule',
        title: `${service.staleAccounts} ${service.staleAccounts === 1 ? 'escaneo viejo' : 'escaneos viejos'} en ${service.service.name}`,
        detail: 'La última foto tiene más de 24 horas. No es lo mismo que un escaneo incompleto.',
        action: 'Actualizar',
      })
    }
    const surplus = service.accounts.reduce((sum, account) => sum + (account.surplus ?? 0), 0)
    if (surplus > 0) {
      items.push({
        id: `${service.service.key}-surplus`,
        service,
        tone: 'info',
        icon: 'change_circle',
        title: `${formatInt(surplus)} más de lo reportado en ${service.service.name}`,
        detail: 'El proveedor declara menos favoritos de los importados. Puede haber cambiado el total o faltar deduplicar.',
        action: 'Revisar',
      })
    }
  }
  return items
})

const servicesNeedingAttention = computed(() => new Set(attention.value.map(item => item.service.service.key)).size)

function announce(label: string) {
  toast.value = `${label} (prototipo)`
  window.clearTimeout(timer)
  timer = window.setTimeout(() => (toast.value = ''), 2200)
}

function useAccount(serviceKey: ServiceKey, accountId: number) {
  active[serviceKey] = accountId
  announce('Cuenta en uso actualizada')
}

onUnmounted(() => window.clearTimeout(timer))
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex items-start justify-between px-8 pb-4 pt-6">
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-2xl font-semibold tracking-tight text-ink">Estado de las conexiones</h1>
          <span class="rounded-full border border-dashed border-accent/40 px-2 py-0.5 text-[10px] uppercase tracking-[0.14em] text-accent">Variante C · Salud</span>
        </div>
        <p class="mt-1 max-w-2xl text-[13px] text-ink-2">
          Primero lo que requiere una acción; el detalle queda a un clic. Cada aviso dice qué regla lo produjo.
        </p>
      </div>
      <ProtoButton variant="primary" icon="sync" @click="announce('Sincronizar todo')">Sincronizar todo</ProtoButton>
    </div>

    <!-- Metrics -->
    <div class="mx-8 mb-4 grid grid-cols-2 gap-3 lg:grid-cols-4">
      <div class="rounded-xl border border-line bg-surface/60 p-3">
        <p class="flex items-center gap-1 text-[10px] uppercase tracking-[0.12em] text-ink-3">
          Cobertura de escaneos
          <span class="material-symbols-outlined cursor-help text-[12px]" title="Suma de importados ÷ suma de reportados, solo cuentas con escaneo.">info</span>
        </p>
        <p class="mt-1 text-xl font-semibold leading-none text-ink">{{ formatPct(totals.coverage) }}</p>
        <ProtoBar class="mt-2" :value="totals.coverage" tone="accent" height="sm" />
        <p class="mt-1 text-[10px] text-ink-3">{{ formatInt(totals.imported) }} de {{ formatInt(totals.reported) }}</p>
      </div>
      <div class="rounded-xl border border-line bg-surface/60 p-3">
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Cuentas sin escanear</p>
        <p class="mt-1 text-xl font-semibold leading-none" :class="totals.missingAccounts ? 'text-warn' : 'text-ok'">{{ totals.missingAccounts }}</p>
        <p class="mt-3 text-[10px] text-ink-3">de {{ totals.totalAccounts }} cuentas · nunca son «0 favoritos»</p>
      </div>
      <div class="rounded-xl border border-line bg-surface/60 p-3">
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Escaneos incompletos</p>
        <p class="mt-1 text-xl font-semibold leading-none" :class="totals.partialAccounts ? 'text-warn' : 'text-ok'">{{ totals.partialAccounts }}</p>
        <p class="mt-3 text-[10px] text-ink-3">de {{ totals.scannedAccounts }} escaneos · no es antigüedad</p>
      </div>
      <div class="rounded-xl border border-line bg-surface/60 p-3">
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Servicios con avisos</p>
        <p class="mt-1 text-xl font-semibold leading-none text-ink">{{ servicesNeedingAttention }} <span class="text-[11px] font-normal text-ink-3">de {{ facts.length }}</span></p>
        <p class="mt-3 text-[10px] text-ink-3">sesiones válidas · sin errores de acceso</p>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-8 pb-28 custom-scrollbar">
      <div class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1.55fr)_minmax(280px,1fr)]">
        <div class="space-y-4">
          <!-- Attention -->
          <section class="rounded-2xl border border-line bg-surface/60">
            <div class="flex items-center justify-between border-b border-line px-4 py-3">
              <h2 class="text-[13px] font-semibold text-ink">Requiere atención</h2>
              <span class="text-[10px] text-ink-3">{{ attention.length }} {{ attention.length === 1 ? 'aviso' : 'avisos' }} · ordenados por impacto</span>
            </div>
            <ul class="divide-y divide-line/60">
              <li v-for="item in attention" :key="item.id" class="flex items-start gap-3 px-4 py-3">
                <div
                  class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full"
                  :class="item.tone === 'warn' ? 'bg-warn/12 text-warn' : item.tone === 'danger' ? 'bg-error/12 text-error' : 'bg-info/12 text-info'"
                >
                  <span class="material-symbols-outlined text-[16px]">{{ item.icon }}</span>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <p class="text-[12px] font-medium text-ink">{{ item.title }}</p>
                    <ProtoChip :tone="item.tone === 'warn' ? 'warn' : item.tone === 'danger' ? 'danger' : 'info'">
                      {{ item.tone === 'warn' ? 'Acción' : 'Informativo' }}
                    </ProtoChip>
                  </div>
                  <p class="mt-0.5 text-[11px] leading-snug text-ink-3">{{ item.detail }}</p>
                </div>
                <ProtoButton size="sm" variant="ghost" @click="announce(item.action)">{{ item.action }}</ProtoButton>
              </li>
              <li v-if="!attention.length" class="flex items-center gap-2 px-4 py-6 text-[12px] text-ink-2">
                <span class="material-symbols-outlined text-[18px] text-ok">check_circle</span>
                Todo escaneado y completo.
              </li>
            </ul>
          </section>

          <!-- Services -->
          <section class="space-y-2">
            <div class="flex items-center justify-between px-1">
              <h2 class="text-[13px] font-semibold text-ink">Servicios</h2>
              <button class="text-[11px] text-ink-3 hover:text-accent" @click="announce('Añadir conexión')">+ Añadir conexión</button>
            </div>

            <div v-for="fact in facts" :key="fact.service.key" class="overflow-hidden rounded-xl border border-line bg-surface/60">
              <div class="flex items-center gap-3 px-3 py-2.5">
                <ProtoMark :icon="fact.service.icon" :mark-class="fact.service.markClass" size="sm" />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <p class="text-[13px] font-semibold text-ink">{{ fact.service.name }}</p>
                    <ProtoChip :tone="fact.stateTone === 'ok' ? 'ok' : fact.stateTone === 'offline' ? 'danger' : 'warn'" dot>{{ fact.stateLabel }}</ProtoChip>
                  </div>
                  <p class="mt-0.5 truncate text-[10px] text-ink-3">{{ fact.primary?.label }} · {{ fact.scannedAccounts }}/{{ fact.totalAccounts }} cuentas escaneadas</p>
                </div>
                <div class="hidden items-center gap-5 text-right sm:flex">
                  <div>
                    <p class="text-[10px] text-ink-3">Favoritos</p>
                    <p class="text-[12px] font-semibold tabular-nums text-ink">{{ formatInt(fact.favoritesInLibrary) }}</p>
                  </div>
                  <div class="w-24">
                    <p class="text-[10px] text-ink-3">Cobertura {{ formatPct(fact.primaryCoverage) }}</p>
                    <ProtoBar class="mt-1" :value="fact.primaryCoverage" :tone="fact.primaryCoverage == null ? 'neutral' : fact.primaryCoverage >= 0.9999 ? 'ok' : 'warn'" height="sm" />
                  </div>
                </div>
                <ProtoButton size="sm" variant="ghost" icon="sync" @click="announce(`Sincronizar ${fact.service.name}`)">
                  <span class="sr-only">Sincronizar {{ fact.service.name }}</span>
                </ProtoButton>
                <button
                  class="rounded p-1 text-ink-3 hover:bg-elevated hover:text-ink"
                  :aria-expanded="Boolean(expanded[fact.service.key])"
                  @click="expanded[fact.service.key] = !expanded[fact.service.key]"
                >
                  <span class="material-symbols-outlined text-[18px]">{{ expanded[fact.service.key] ? 'expand_less' : 'expand_more' }}</span>
                </button>
              </div>

              <div v-if="expanded[fact.service.key]" class="border-t border-line/60 bg-elevated/15 px-3 py-2">
                <div
                  v-for="account in fact.accounts"
                  :key="account.account.id"
                  class="flex items-center gap-2 rounded-lg px-1.5 py-1.5 hover:bg-elevated/40"
                >
                  <button
                    class="flex h-4.5 w-4.5 items-center justify-center rounded-full border"
                    :class="account.isActive ? 'border-accent bg-accent text-on-accent' : 'border-line-strong text-transparent hover:border-accent'"
                    :aria-label="`Usar ${account.label}`"
                    @click="useAccount(fact.service.key, account.account.id)"
                  >
                    <span v-if="account.isActive" class="material-symbols-outlined text-[10px]">check</span>
                  </button>
                  <p class="min-w-0 flex-1 truncate text-[11px] text-ink-2">{{ account.label }}</p>
                  <span class="text-[10px] tabular-nums text-ink-3">{{ account.reported == null ? 'Sin datos' : `${formatInt(account.imported)} / ${formatInt(account.reported)}` }}</span>
                  <ProtoChip :tone="account.state === 'ok' ? 'ok' : account.state === 'never' ? 'neutral' : 'warn'">{{ account.stateLabel }}</ProtoChip>
                  <span class="w-20 text-right text-[9px] text-ink-3" :title="formatAbsolute(account.scannedAt)">{{ account.scannedRelative }}</span>
                </div>
              </div>
            </div>
          </section>
        </div>

        <!-- Rules -->
        <aside class="h-fit rounded-2xl border border-line bg-surface/60 p-4">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px] text-accent">calculate</span>
            <h2 class="text-[13px] font-semibold text-ink">Cómo se calcula</h2>
          </div>
          <dl class="mt-3 space-y-3 text-[11px] leading-snug">
            <div>
              <dt class="font-medium text-ink">Cobertura de una cuenta</dt>
              <dd class="text-ink-3"><code class="rounded bg-elevated px-1 py-0.5 text-[10px] text-ink-2">importados ÷ reportados</code> del último escaneo. Sin reportado no hay porcentaje.</dd>
            </div>
            <div>
              <dt class="font-medium text-ink">Faltan / sobran</dt>
              <dd class="text-ink-3"><code class="rounded bg-elevated px-1 py-0.5 text-[10px] text-ink-2">reportado − importado</code>. Si sobran, se muestra como anomalía, nunca como 100 %.</dd>
            </div>
            <div>
              <dt class="font-medium text-ink">Frescura</dt>
              <dd class="text-ink-3">Menos de 6 h: reciente · 6–24 h: puede estar viejo · más de 24 h: desactualizado.</dd>
            </div>
            <div>
              <dt class="font-medium text-ink">Incompleto</dt>
              <dd class="text-ink-3">El proveedor no entregó todas las páginas (<code class="text-[10px]">completeness = partial</code>). No es lo mismo que viejo.</dd>
            </div>
            <div>
              <dt class="font-medium text-ink">Favoritos de biblioteca</dt>
              <dd class="text-ink-3">Pistas únicas. Las cuentas no se suman: la misma canción en dos cuentas cuenta una vez.</dd>
            </div>
          </dl>
          <p class="mt-4 border-t border-line pt-3 text-[10px] leading-relaxed text-ink-3">
            Ningún aviso se dispara por «parecer raro»: cada uno tiene una regla y un número detrás.
          </p>
        </aside>
      </div>
    </div>

    <Transition name="fade">
      <div v-if="toast" class="fixed bottom-24 left-1/2 z-50 -translate-x-1/2 rounded-lg border border-line bg-shell px-3 py-2 text-xs text-ink shadow-xl">{{ toast }}</div>
    </Transition>
  </div>
</template>
