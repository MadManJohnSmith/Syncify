<script setup lang="ts">
import { computed, reactive, ref, onUnmounted } from 'vue'
import {
  PROTO_NOW,
  PROTO_SERVICES,
  formatAbsolute,
  formatInt,
  formatPct,
  formatShortDate,
  globalFacts,
  serviceFacts,
  type AccountFacts,
  type ServiceFacts,
  type ServiceKey,
} from './accountsPrototypeData'
import ProtoBar from './ProtoBar.vue'
import ProtoButton from './ProtoButton.vue'
import ProtoChip from './ProtoChip.vue'
import ProtoMark from './ProtoMark.vue'

type Filter = 'all' | 'attention' | 'scanned'

const services = ref(PROTO_SERVICES.map(service => ({ ...service, accounts: service.accounts.map(account => ({ ...account })) })))
const active = reactive<Record<ServiceKey, number>>({ spotify: 1, qobuz: 3, tidal: 7 })
const filter = ref<Filter>('all')
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

const isAttention = (account: AccountFacts) =>
  account.state !== 'ok' || (account.surplus ?? 0) > 0

const visibleGroups = computed(() =>
  facts.value
    .map(service => ({
      service,
      rows: service.accounts.filter(account => {
        if (filter.value === 'scanned') return account.reported != null
        if (filter.value === 'attention') return isAttention(account)
        return true
      }),
    }))
    .filter(group => group.rows.length > 0),
)

const attentionCount = computed(() => facts.value.flatMap(service => service.accounts).filter(isAttention).length)

function useAccount(serviceKey: ServiceKey, accountId: number) {
  active[serviceKey] = accountId
  toast.value = 'Cuenta en uso actualizada (prototipo)'
  window.clearTimeout(timer)
  timer = window.setTimeout(() => (toast.value = ''), 1800)
}

function announce(label: string) {
  toast.value = `${label} (prototipo)`
  window.clearTimeout(timer)
  timer = window.setTimeout(() => (toast.value = ''), 2200)
}

onUnmounted(() => window.clearTimeout(timer))
import ProtoEmpty from './ProtoEmpty.vue'
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex items-start justify-between px-8 pb-4 pt-6">
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-2xl font-semibold tracking-tight text-ink">Cuentas y cobertura</h1>
          <span class="rounded-full border border-dashed border-accent/40 px-2 py-0.5 text-[10px] uppercase tracking-[0.14em] text-accent">Variante B · Libro mayor</span>
        </div>
        <p class="mt-1 max-w-2xl text-[13px] text-ink-2">
          Cada fila es una cuenta y cada número tiene un dueño. Compara lo reportado con lo importado sin sumar bibliotecas.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <ProtoButton variant="ghost" icon="sync" @click="announce('Sincronizar todo')">Sincronizar todo</ProtoButton>
        <ProtoButton variant="primary" icon="add">Añadir cuenta</ProtoButton>
      </div>
    </div>

    <!-- Summary strip -->
    <div class="mx-8 mb-3 grid grid-cols-2 gap-x-5 gap-y-2 rounded-xl border border-line bg-surface/60 px-4 py-3 md:grid-cols-4">
      <div>
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Servicios</p>
        <p class="mt-0.5 text-lg font-semibold leading-tight text-ink">{{ facts.length }} <span class="text-[11px] font-normal text-ok">conectados</span></p>
      </div>
      <div>
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Cuentas</p>
        <p class="mt-0.5 text-lg font-semibold leading-tight text-ink">{{ totals.totalAccounts }} <span class="text-[11px] font-normal text-ink-3">en total</span></p>
      </div>
      <div>
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Sin escanear</p>
        <p class="mt-0.5 text-lg font-semibold leading-tight" :class="totals.missingAccounts ? 'text-warn' : 'text-ink'">
          {{ totals.missingAccounts }} <span class="text-[11px] font-normal text-ink-3">cuentas</span>
        </p>
      </div>
      <div>
        <p class="text-[10px] uppercase tracking-[0.12em] text-ink-3">Cobertura de escaneos</p>
        <p class="mt-0.5 flex items-baseline gap-2 text-lg font-semibold leading-tight text-ink">
          {{ formatPct(totals.coverage) }}
          <span class="text-[11px] font-normal text-ink-3">{{ formatInt(totals.imported) }} / {{ formatInt(totals.reported) }}</span>
        </p>
      </div>
    </div>

    <!-- Filters -->
    <div class="mx-8 mb-3 flex items-center gap-1.5">
      <button
        v-for="option in ([
          { id: 'all', label: 'Todas', count: totals.totalAccounts },
          { id: 'attention', label: 'Para revisar', count: attentionCount },
          { id: 'scanned', label: 'Con escaneo', count: totals.scannedAccounts },
        ] as const)"
        :key="option.id"
        class="rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors"
        :class="filter === option.id ? 'bg-accent/15 text-accent' : 'text-ink-3 hover:bg-elevated hover:text-ink-2'"
        @click="filter = option.id"
      >
        {{ option.label }} <span class="opacity-70">{{ option.count }}</span>
      </button>
      <span class="ml-auto text-[10px] text-ink-3">Cobertura = importado ÷ reportado · solo cuentas escaneadas</span>
    </div>

    <div class="flex-1 overflow-y-auto px-8 pb-28 custom-scrollbar">
      <div class="overflow-hidden rounded-2xl border border-line bg-surface/60">
        <table class="w-full border-collapse text-left">
          <thead>
            <tr class="border-b border-line bg-elevated/30 text-[10px] uppercase tracking-[0.1em] text-ink-3">
              <th class="w-8 py-2.5 pl-4" />
              <th class="py-2.5 pr-3 font-medium">Cuenta</th>
              <th class="w-28 py-2.5 pr-3 font-medium">Estado</th>
              <th class="w-24 py-2.5 pr-3 text-right font-medium">Reportados</th>
              <th class="w-24 py-2.5 pr-3 text-right font-medium">Importados</th>
              <th class="w-44 py-2.5 pr-3 font-medium">Cobertura</th>
              <th class="w-28 py-2.5 pr-3 font-medium">Último escaneo</th>
              <th class="w-10 py-2.5 pr-3" />
            </tr>
          </thead>
          <tbody v-for="group in visibleGroups" :key="group.service.service.key">
            <tr class="border-y border-line/70 bg-elevated/20">
              <td colspan="8" class="px-4 py-2">
                <div class="flex items-center gap-3">
                  <ProtoMark :icon="group.service.service.icon" :mark-class="group.service.service.markClass" size="sm" />
                  <div class="min-w-0">
                    <p class="text-[13px] font-semibold leading-tight text-ink">{{ group.service.service.name }}</p>
                    <p class="text-[10px] text-ink-3">
                      {{ group.service.scannedAccounts }} de {{ group.service.totalAccounts }} cuentas con escaneo
                      <span v-if="group.service.partialAccounts"> · {{ group.service.partialAccounts }} {{ group.service.partialAccounts === 1 ? 'incompleta' : 'incompletas' }}</span>
                      · {{ formatInt(group.service.favoritesInLibrary) }} favoritos en biblioteca
                    </p>
                  </div>
                  <div v-if="group.service.reportedAcross != null" class="ml-auto flex items-center gap-4 text-[10px]">
                    <span class="text-ink-3">Suma de cuentas</span>
                    <span class="tabular-nums text-ink-2">{{ formatInt(group.service.importedAcross) }} / {{ formatInt(group.service.reportedAcross) }}</span>
                    <span class="tabular-nums font-semibold text-ink">{{ formatPct(group.service.coverageAcross) }}</span>
                    <span
                      v-if="group.service.scannedAccounts > 1"
                      class="cursor-help rounded-full bg-warn/10 px-1.5 py-0.5 text-[9px] text-warn"
                      title="Cada cuenta se escanea por separado. Sumarlas puede contar dos veces a la misma persona; no es una cifra de biblioteca."
                    >puede solaparse</span>
                  </div>
                  <ProtoButton size="sm" variant="ghost" icon="sync" class="ml-2" @click="announce(`Sincronizar ${group.service.service.name}`)">Sincronizar</ProtoButton>
                </div>
              </td>
            </tr>

            <tr
              v-for="account in group.rows"
              :key="account.account.id"
              class="group border-b border-line/40 transition-colors last:border-0 hover:bg-elevated/25"
              :class="account.isActive && 'bg-accent/[0.04]'"
            >
              <td class="py-2.5 pl-4">
                <button
                  class="flex h-5 w-5 items-center justify-center rounded-full border transition-colors"
                  :class="account.isActive ? 'border-accent bg-accent text-on-accent' : 'border-line-strong text-transparent hover:border-accent'"
                  :aria-label="`Usar ${account.label}`"
                  :title="account.isActive ? 'Cuenta en uso' : 'Usar esta cuenta'"
                  @click="useAccount(group.service.service.key, account.account.id)"
                >
                  <span v-if="account.isActive" class="material-symbols-outlined text-[12px]">check</span>
                </button>
              </td>
              <td class="max-w-[320px] py-2.5 pr-3">
                <div class="flex items-center gap-2">
                  <p class="truncate text-[12px] font-medium text-ink" :title="account.label">{{ account.label }}</p>
                  <ProtoChip v-if="account.isActive" tone="accent">En uso</ProtoChip>
                </div>
                <p class="mt-0.5 truncate text-[10px] text-ink-3">{{ account.credentialLabel }}</p>
              </td>
              <td class="py-2.5 pr-3">
                <ProtoChip :tone="account.state === 'ok' ? 'ok' : account.state === 'never' ? 'neutral' : 'warn'" dot>
                  {{ account.stateLabel }}
                </ProtoChip>
              </td>
              <td class="py-2.5 pr-3 text-right text-[12px] tabular-nums text-ink-2">
                {{ account.reported == null ? '—' : formatInt(account.reported) }}
              </td>
              <td class="py-2.5 pr-3 text-right text-[12px] tabular-nums text-ink">
                {{ account.imported == null ? '—' : formatInt(account.imported) }}
              </td>
              <td class="py-2.5 pr-3">
                <div class="flex items-center gap-2">
                  <ProtoBar class="w-24" :value="account.coverage" :tone="account.state === 'ok' ? 'ok' : account.state === 'never' ? 'neutral' : 'warn'" height="sm" />
                  <span class="w-11 text-[10px] tabular-nums text-ink-2">{{ formatPct(account.coverage) }}</span>
                </div>
                <p v-if="account.surplus" class="mt-1 text-[9px] text-warn">+{{ formatInt(account.surplus) }} sobre lo reportado</p>
                <p v-else-if="account.missing" class="mt-1 text-[9px] text-ink-3">faltan {{ formatInt(account.missing) }}</p>
              </td>
              <td class="py-2.5 pr-3">
                <p class="text-[11px] text-ink-2">{{ account.state === 'never' ? 'Sin escanear' : account.scannedRelative }}</p>
                <p class="text-[9px] text-ink-3" :title="account.scannedAbsolute">{{ account.scannedAt ? formatShortDate(account.scannedAt) : 'sin registro' }}</p>
              </td>
              <td class="py-2.5 pr-3">
                <button class="rounded p-1 text-ink-3 opacity-0 transition-opacity hover:bg-elevated hover:text-ink group-hover:opacity-100" title="Acciones de la cuenta" @click="announce('Acciones de la cuenta')">
                  <span class="material-symbols-outlined text-[16px]">more_horiz</span>
                </button>
              </td>
            </tr>
          </tbody>
        </table>

        <ProtoEmpty
          v-if="!visibleGroups.length"
          title="No hay cuentas con este filtro"
          hint="Hay cuentas, pero ninguna pasa el filtro activo."
          action-label="Ver todas las cuentas"
          @action="filter = 'all'"
        />
      </div>

      <div class="mt-4 flex flex-wrap items-center gap-x-5 gap-y-1 text-[10px] text-ink-3">
        <span><strong class="text-ink-2">Reportados:</strong> lo que declara el proveedor para esa cuenta.</span>
        <span><strong class="text-ink-2">Importados:</strong> pistas que devolvió su último escaneo.</span>
        <span><strong class="text-ink-2">Sin escanear ≠ 0:</strong> no hubo respuesta, no es que la cuenta esté vacía.</span>
      </div>
    </div>

    <Transition name="fade">
      <div v-if="toast" class="fixed bottom-24 left-1/2 z-50 -translate-x-1/2 rounded-lg border border-line bg-shell px-3 py-2 text-xs text-ink shadow-xl">{{ toast }}</div>
    </Transition>
  </div>
</template>
