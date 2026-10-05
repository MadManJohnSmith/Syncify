/**
 * R11 (medición): coste del grafo reactivo de DownloadsView por evento de
 * progreso, con la estructura real de la vista.
 *
 * Ejecutar: npx tsx scripts/measure-downloads-reactivity.mts
 * Compara `SHAPE=before` (mapeo eager de las tres listas, el estado previo a
 * este carril) con el valor por defecto (mapeo perezoso: solo la porción
 * visible se materializa).
 */
import { computed, ref, shallowRef } from 'vue'

const N = 50_000
const SHAPE = process.env.SHAPE === 'before' ? 'before' : 'after'

const useShallow = SHAPE === 'after'
const raw: any = useShallow ? shallowRef<any[]>([]) : ref<any[]>([])
const progressTick = ref(0)
for (let i = 0; i < N; i++) {
  const status = i < 5 ? 'downloading' : i < 20 ? 'queued' : i < 40 ? 'failed' : 'completed'
  raw.value.push({
    id: i + 1,
    track_id: i + 1,
    status,
    title: `Track ${i}`,
    artist: `Artist ${i % 500}`,
    target_artist: `Artist ${i % 500}`,
    target_album: `Album ${i % 300}`,
    service: 'tidal',
    progress_percent: 0,
    completed_at: '2026-10-05 00:00:00',
  })
}
if (!useShallow) raw.value = raw.value

function current(): any[] {
  void progressTick.value
  return raw.value
}

function mapQueueItem(item: any) {
  return {
    id: item.id,
    trackId: item.track_id,
    title: item.target_title || item.title || 'Unknown Track',
    artist: item.target_artist || item.artist || 'Unknown Artist',
    album: item.target_album || 'Album',
    serviceBadgeClass: 'badge',
    quality: `Declared ${item.quality}`,
    qualityBadgeClass: 'badge',
    progress: item.progress_percent || 0,
    status: item.status,
  }
}
function mapCompletedItem(item: any) {
  return { ...mapQueueItem(item), resultLabel: 'ok', resultClass: 'ok' }
}
function mapFailedItem(item: any) {
  return { ...mapQueueItem(item), resultLabel: 'failed', resultClass: 'fail' }
}

const activeDownloads = computed(() => {
  void progressTick.value
  return raw.value.filter(i => i.status === 'downloading').map(mapQueueItem)
})

let queueItems: any, completedItems: any, failedItems: any
let queuedCount: any, completedItemsCount: any, failedItemsCount: any
let filteredQueueItems: any, filteredCompletedItems: any, filteredFailedItems: any
let visibleQueueItems: any, visibleCompletedSlice: any, visibleFailedSlice: any

if (SHAPE === 'before') {
  queueItems = computed(() => current().filter(i => i.status === 'queued').map(mapQueueItem))
  completedItems = computed(() =>
    current().filter(i => i.status === 'completed' || i.status === 'complete').map(mapCompletedItem),
  )
  failedItems = computed(() => current().filter(i => i.status === 'failed').map(mapFailedItem))
  filteredQueueItems = computed(() => queueItems.value)
  filteredCompletedItems = computed(() => completedItems.value)
  filteredFailedItems = computed(() => failedItems.value)
  visibleQueueItems = computed(() => filteredQueueItems.value.slice(0, 40).map((i: any) => ({ ...i, absoluteIndex: 0 })))
  visibleCompletedSlice = computed(() => filteredCompletedItems.value.slice(0, 50))
  visibleFailedSlice = computed(() => filteredFailedItems.value.slice(0, 50))
  queuedCount = computed(() => queueItems.value.length)
  completedItemsCount = computed(() => completedItems.value.length)
  failedItemsCount = computed(() => failedItems.value.length)
} else {
  const queuedItemsRaw = computed(() => current().filter(i => i.status === 'queued'))
  const completedItemsRaw = computed(() =>
    current().filter(i => i.status === 'completed' || i.status === 'complete'),
  )
  const failedItemsRaw = computed(() => current().filter(i => i.status === 'failed'))
  queuedCount = computed(() => queuedItemsRaw.value.length)
  completedItemsCount = computed(() => completedItemsRaw.value.length)
  failedItemsCount = computed(() => failedItemsRaw.value.length)
  filteredQueueItems = computed(() => queuedItemsRaw.value)
  filteredCompletedItems = computed(() => completedItemsRaw.value)
  filteredFailedItems = computed(() => failedItemsRaw.value)
  visibleQueueItems = computed(() => filteredQueueItems.value.slice(0, 40).map(mapQueueItem))
  visibleCompletedSlice = computed(() => filteredCompletedItems.value.slice(0, 50).map(mapCompletedItem))
  visibleFailedSlice = computed(() => filteredFailedItems.value.slice(0, 50).map(mapFailedItem))
}

// Lectura de todo lo que la vista dibuja en cada tick.
const sink = computed(() =>
  activeDownloads.value.length +
  visibleQueueItems.value.length +
  visibleCompletedSlice.value.length +
  visibleFailedSlice.value.length +
  queuedCount.value + completedItemsCount.value + failedItemsCount.value,
)

void sink.value

const EVENTS = 40
let findTotal = 0
let recomputeTotal = 0
for (let e = 0; e < EVENTS; e++) {
  const t0 = performance.now()
  const item = raw.value.find(q => q.id === 3)
  const find = performance.now() - t0

  item.progress_percent = e
  if (useShallow) progressTick.value += 1

  const t1 = performance.now()
  void sink.value
  const recompute = performance.now() - t1

  findTotal += find
  recomputeTotal += recompute
}

console.log(JSON.stringify({
  shape: SHAPE,
  items: N,
  events: EVENTS,
  findAvgMs: Number((findTotal / EVENTS).toFixed(3)),
  recomputeAvgMs: Number((recomputeTotal / EVENTS).toFixed(3)),
  avgMsPerProgressEvent: Number(((findTotal + recomputeTotal) / EVENTS).toFixed(3)),
  projectedMsPerSecondAt4x4: Number((((findTotal + recomputeTotal) / EVENTS) * 16).toFixed(1)),
}, null, 2))