<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  PROTO_SERVICES,
  formatInt,
  rawUtcStamp,
  type ProtoService,
  type ProtoSnapshot,
} from './accountsPrototypeData'
import ProtoMark from './ProtoMark.vue'

const toast = ref('')

interface BaselineCard {
  service: ProtoService
  snapshots: ProtoSnapshot[]
  reportedCard: number | null
  accounts: Array<{
    id: number
    label: string
    line: string
    isActive: boolean
  }>
}

const cards = computed<BaselineCard[]>(() =>
  PROTO_SERVICES.map(service => {
    const snapshots = service.accounts
      .map(account => account.snapshot)
      .filter((snapshot): snapshot is ProtoSnapshot => Boolean(snapshot))
    const everyReported = snapshots.length > 0 && snapshots.every(snapshot => snapshot.reported != null)
    const reportedCard = everyReported
      ? snapshots.reduce((sum, snapshot) => sum + (snapshot.reported ?? 0), 0)
      : null

    return {
      service,
      snapshots,
      reportedCard,
      accounts: service.accounts.map(account => {
        const snapshot = account.snapshot
        const label =
          account.displayName && account.email
            ? `${account.displayName} (${account.email})`
            : (account.email ?? account.displayName ?? 'Identidad no indicada')
        const line = snapshot
          ? `Reported favorites: ${snapshot.reported == null ? 'Not available' : formatInt(snapshot.reported)} · Imported favorites: ${formatInt(snapshot.imported)} · ${snapshot.completeness}${snapshot.completeness !== 'complete' ? ' · stale' : ''} · ${rawUtcStamp(snapshot.scannedAt)}`
          : 'Reported favorites: Not available · Imported favorites: Not available · no data · no time'
        return { id: account.id, label, line, isActive: account.isActive }
      }),
    }
  }),
)

function simulate(label: string) {
  toast.value = `${label}: acción simulada (prototipo)`
  window.setTimeout(() => (toast.value = ''), 2200)
}
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex items-start justify-between px-8 pb-5 pt-6">
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-2xl font-semibold tracking-tight text-ink">Connected Services</h1>
          <span class="rounded-full border border-line px-2 py-0.5 text-[10px] uppercase tracking-[0.14em] text-ink-3">Hoy · referencia</span>
        </div>
        <p class="mt-1 text-[13px] text-ink-2">Réplica de la pantalla actual con los mismos datos de la captura.</p>
      </div>
      <div class="flex items-center gap-2">
        <button class="rounded-lg border border-line px-3 py-2 text-xs font-medium text-ink-2" @click="simulate('Credenciales Spotify')">Spotify Credentials</button>
        <button class="rounded-lg bg-accent px-4 py-2 text-xs font-semibold text-on-accent" @click="simulate('Añadir conexión')">Add Connection</button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-8 pb-28 custom-scrollbar">
      <div class="grid grid-cols-1 items-start gap-5 xl:grid-cols-3">
        <article
          v-for="card in cards"
          :key="card.service.key"
          class="group relative rounded-2xl border border-line bg-surface/70 p-5"
        >
          <div class="absolute right-3 top-3 flex gap-1 text-ink-3">
            <button class="rounded p-1" title="Credenciales" @click="simulate('Credenciales')"><span class="material-symbols-outlined text-[17px]">key</span></button>
            <button class="rounded p-1" title="Ajustes" @click="simulate('Ajustes')"><span class="material-symbols-outlined text-[17px]">settings</span></button>
          </div>

          <div class="mb-4 flex items-center gap-3">
            <ProtoMark :icon="card.service.icon" :mark-class="card.service.markClass" />
            <div>
              <h2 class="text-[17px] font-semibold text-ink">{{ card.service.name }}</h2>
              <div class="mt-1 inline-flex items-center gap-1 rounded-full bg-ok/10 px-2 py-0.5 text-[11px] font-medium text-ok">
                <span class="material-symbols-outlined text-[13px]">check_circle</span>
                Connected
              </div>
            </div>
          </div>

          <div class="grid grid-cols-3 gap-2">
            <div class="rounded-xl bg-elevated/50 py-2.5 text-center">
              <span class="material-symbols-outlined text-[17px] text-ink-2">music_note</span>
              <p class="mt-0.5 text-lg font-semibold leading-tight text-ink">{{ formatInt(card.service.library.tracks) }}</p>
              <p class="text-[9px] uppercase tracking-wide text-ink-3">Total imported</p>
              <p class="mt-0.5 text-[9px] text-ink-3">Reported favorites: {{ card.reportedCard == null ? 'Not available' : formatInt(card.reportedCard) }}</p>
            </div>
            <div class="rounded-xl bg-elevated/50 py-2.5 text-center">
              <span class="material-symbols-outlined text-[17px] text-ink-2">queue_music</span>
              <p class="mt-0.5 text-lg font-semibold leading-tight text-ink">{{ formatInt(card.service.library.playlists) }}</p>
              <p class="text-[9px] uppercase tracking-wide text-ink-3">Playlists</p>
            </div>
            <div class="rounded-xl bg-elevated/50 py-2.5 text-center">
              <span class="material-symbols-outlined text-[17px] text-ink-2">favorite</span>
              <p class="mt-0.5 text-lg font-semibold leading-tight text-ink">{{ formatInt(card.service.library.favorites) }}</p>
              <p class="text-[9px] uppercase tracking-wide text-ink-3">Favorites</p>
            </div>
          </div>

          <p class="mt-3 text-center text-[11px] text-ink-3">Last synced: Just now</p>

          <div class="mt-3 space-y-2">
            <div
              v-for="account in card.accounts"
              :key="account.id"
              class="rounded-xl border px-3 py-2.5"
              :class="account.isActive ? 'border-accent/25 bg-accent/5' : 'border-transparent bg-elevated/30'"
            >
              <div class="flex items-start justify-between gap-2">
                <p class="truncate text-[12px] font-medium text-ink">{{ account.label }}</p>
                <span v-if="account.isActive" class="rounded bg-accent/15 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-accent">Active</span>
              </div>
              <p class="mt-1 text-[10px] leading-relaxed text-ink-3">{{ account.line }}</p>
              <div class="mt-1.5 flex flex-wrap gap-x-3 gap-y-1 text-[10px] font-medium text-ink-2">
                <button v-if="!account.isActive" class="hover:text-ink" @click="simulate('Usar esta cuenta')">Use this account</button>
                <button class="hover:text-ink" @click="simulate('Reconectar')">Reconnect</button>
                <button class="hover:text-ink" @click="simulate('Desconectar')">Disconnect</button>
                <button class="text-ink-3 hover:text-error" @click="simulate('Eliminar')">Remove</button>
              </div>
            </div>
          </div>

          <button class="mt-3 flex items-center gap-1 text-[11px] text-ink-2" @click="simulate('Gestionar cuentas')">
            <span class="material-symbols-outlined text-[13px]">arrow_right</span>
            Manage accounts ({{ card.accounts.length }})
          </button>

          <div class="mt-4 flex items-center gap-2">
            <button class="flex h-9 flex-1 items-center justify-center gap-1.5 rounded-full bg-ink text-[12px] font-semibold text-base" @click="simulate('Sincronizar')">
              <span class="material-symbols-outlined text-[17px]">graphic_eq</span>
              Sync
            </button>
            <button v-if="card.service.key === 'qobuz'" class="h-9 rounded-full border border-warn/30 px-3 text-[11px] text-warn" @click="simulate('Compras')">Purchases</button>
            <button class="h-9 rounded-full bg-elevated px-3 text-[11px] text-ink-2" @click="simulate('Desconectar')">Disconnect</button>
            <button class="h-9 rounded-full bg-elevated px-3 text-[11px] text-ink-2" @click="simulate('Añadir cuenta')">+ Add account</button>
          </div>
        </article>
      </div>
    </div>

    <Transition name="fade">
      <div v-if="toast" class="fixed bottom-24 left-1/2 z-50 -translate-x-1/2 rounded-lg border border-line bg-shell px-3 py-2 text-xs text-ink shadow-xl">{{ toast }}</div>
    </Transition>
  </div>
</template>
