import { createApp } from 'vue'
import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import App from './App.vue'
import './styles/main.css'

// Import views
import DashboardView from './views/DashboardView.vue'
import AlbumDetailView from './views/AlbumDetailView.vue'
import ArtistDetailView from './views/ArtistDetailView.vue'

export const routes: RouteRecordRaw[] = [
    { path: '/', redirect: '/dashboard' },
    { path: '/dashboard', name: 'Dashboard', component: DashboardView },
    { path: '/library', name: 'Library', component: () => import('./views/LibraryView.vue') },
    { path: '/playlists', name: 'Playlists', component: () => import('./views/PlaylistView.vue') },
    { path: '/downloads', name: 'Downloads', component: () => import('./views/DownloadsView.vue') },
    { path: '/queue', redirect: '/downloads' },
    { path: '/accounts', name: 'Accounts', component: () => import('./views/AccountsView.vue') },
    // Settings is lazy-loaded to reduce bundle size (Sprint 31)
    { path: '/settings', name: 'Settings', component: () => import('./views/SettingsView.vue') },
    { path: '/logs', name: 'Logs', component: () => import('./views/LogsView.vue') },
    { path: '/metadata', name: 'Metadata', component: () => import('./views/MetadataView.vue') },
    { path: '/lyrics', name: 'Lyrics', component: () => import('./views/LyricsView.vue') },
    { path: '/migration', name: 'Migration', component: () => import('./views/MigrationView.vue') },
    // Sprint 4: Detail Views
    { path: '/album/:id', name: 'AlbumDetail', component: AlbumDetailView },
    { path: '/artist/:id', name: 'ArtistDetail', component: ArtistDetailView },
    { path: '/search', name: 'Search', component: () => import('./views/SearchView.vue') },
    { path: '/:pathMatch(.*)*', redirect: '/dashboard' },
]

export const router = createRouter({
    history: createWebHistory(),
    routes,
})

if (typeof document !== 'undefined') {
    const mountEl = document.getElementById('app')
    if (mountEl) {
        createApp(App).use(router).mount(mountEl)
    }
}
