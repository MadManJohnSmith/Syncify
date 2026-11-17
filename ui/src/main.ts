import { createApp } from 'vue'
import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import App from './App.vue'
import './styles/main.css'

// FE-PERF-03 (#50): TODAS las vistas son `import()` dinámico. Antes
// DashboardView, AlbumDetailView y ArtistDetailView se importaban aquí de
// forma estática y engordaban el chunk principal (452 kB raw / 140 kB gz):
// el arranque cargaba y parseaba de golpe también AlbumDetail y ArtistDetail,
// que no son la primera pantalla. Con imports dinámicos el chunk principal
// queda reducido al shell (App + router + API) y cada vista viaja en su
// chunk; la medición antes/después está en ~/.local/state/syncify-fperf03/
// y en el PR. Nota: la primera navegación a Dashboard añade el fetch de su
// chunk, que solapa con el init del backend mientras corre la splash.
export const routes: RouteRecordRaw[] = [
    { path: '/', redirect: '/dashboard' },
    { path: '/dashboard', name: 'Dashboard', component: () => import('./views/DashboardView.vue') },
    { path: '/library', name: 'Library', component: () => import('./views/LibraryView.vue') },
    // L03 (#100): armazón de pestañas de la biblioteca (spec #96). El
    // contenido lo completan L07 (álbumes), L08 (artistas) y L10
    // (duplicados); el placeholder muestra el motivo, no simula nada.
    // Playlists y descargas conservan sus vistas y rutas propias.
    { path: '/library/albums', name: 'LibraryAlbums', component: () => import('./views/LibraryTabPlaceholder.vue'), meta: { libraryTab: 'albums' } },
    { path: '/library/artists', name: 'LibraryArtists', component: () => import('./views/LibraryTabPlaceholder.vue'), meta: { libraryTab: 'artists' } },
    { path: '/library/duplicates', name: 'LibraryDuplicates', component: () => import('./views/LibraryTabPlaceholder.vue'), meta: { libraryTab: 'duplicates' } },
    { path: '/playlists', name: 'Playlists', component: () => import('./views/PlaylistView.vue') },
    { path: '/downloads', name: 'Downloads', component: () => import('./views/DownloadsView.vue') },
    { path: '/queue', redirect: '/downloads' },
    { path: '/accounts', name: 'Accounts', component: () => import('./views/AccountsView.vue') },
    // PROTOTIPO desechable (solo dev): prototipo del tab de accounts.
    // Se elimina al cerrar la decision; no entra en el bundle de produccion.
    ...(import.meta.env.DEV
      ? [
          {
            path: '/__prototype',
            name: 'prototype-index',
            component: () => import('./views/__prototype__/Indice.vue'),
          },
          {
            path: '/__prototype/accounts',
            name: 'AccountsPrototype',
            component: () => import('./views/__prototype__/AccountsPrototype.vue'),
          },
        ]
      : []),
    // Settings is lazy-loaded to reduce bundle size (Sprint 31)
    { path: '/settings', name: 'Settings', component: () => import('./views/SettingsView.vue') },
    { path: '/logs', name: 'Logs', component: () => import('./views/LogsView.vue') },
    { path: '/metadata', name: 'Metadata', component: () => import('./views/MetadataView.vue') },
    { path: '/lyrics', name: 'Lyrics', component: () => import('./views/LyricsView.vue') },
    { path: '/migration', name: 'Migration', component: () => import('./views/MigrationView.vue') },
    // Sprint 4: Detail Views (FE-PERF-03: también dinámicas)
    { path: '/album/:id', name: 'AlbumDetail', component: () => import('./views/AlbumDetailView.vue') },
    { path: '/artist/:id', name: 'ArtistDetail', component: () => import('./views/ArtistDetailView.vue') },
    // R17: search lives ONLY as a modal (App.vue). There is no /search route: an
    // old link falls into the catch-all and lands on /dashboard.
    { path: '/:pathMatch(.*)*', redirect: '/dashboard' },
]

export const router = createRouter({
    history: createWebHistory(),
    routes,
})

if (typeof document !== 'undefined') {
    const mountEl = document.getElementById('app')
    if (mountEl) {
        const app = createApp(App).use(router)
        // FE-PERF-01: sonda de navegación. Chunk separado que SOLO se descarga
        // en la ventana real de desarrollo con la medición pedida
        // (VITE_NAVPERF=1 o ?__navperf=1): cero coste en producción y en tests.
        const inTauri =
            typeof window !== 'undefined' &&
            typeof (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ !== 'undefined'
        if (inTauri && (import.meta.env.VITE_NAVPERF === '1' || window.location.search.includes('__navperf=1'))) {
            void import('./perf/navigationProbe')
                .then((m) => m.installNavigationProbe(app, router))
                .catch((err) => console.warn('[navperf] probe no instalada:', err))
        }
        app.mount(mountEl)
    }
}
