import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import path from 'path'
import fs from 'fs'

// FE-PERF-01 (#49): collector de la sonda de navegación (ui/src/perf/navigationProbe.ts).
// SOLO existe en `vite dev` arrancado con VITE_NAVPERF=1: el harness
// (tools/nav-perf.sh) fija VITE_NAVPERF_COLLECT y la sonda hace POST de sus
// muestras aquí. Sin esa variable el middleware no se registra y ni el build
// de producción ni los tests lo ven.
function navPerfCollector(): Plugin {
    return {
        name: 'navperf-collector',
        configureServer(server) {
            if (process.env.VITE_NAVPERF !== '1') return
            const outFile = process.env.VITE_NAVPERF_COLLECT
            if (!outFile) {
                server.config.logger.warn('[navperf-collector] VITE_NAVPERF=1 sin VITE_NAVPERF_COLLECT: resultados no se guardan')
                return
            }
            server.middlewares.use('/__navperf_collect', (req, res) => {
                const chunks: Buffer[] = []
                req.on('data', (c: Buffer) => chunks.push(c))
                req.on('end', () => {
                    try {
                        fs.appendFileSync(outFile, Buffer.concat(chunks).toString('utf8') + '\n')
                        res.statusCode = 204
                    } catch (err) {
                        res.statusCode = 500
                    }
                    res.end()
                })
            })
        },
    }
}

export default defineConfig({
    plugins: [vue(), navPerfCollector()],
    clearScreen: false,
    server: {
        port: 5173,
        strictPort: true,
    },
    envPrefix: ['VITE_', 'TAURI_'],
    resolve: {
        alias: {
            '@': path.resolve(__dirname, 'src')
        }
    },
    build: {
        target: ['es2021', 'chrome100', 'safari13'],
        minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
        sourcemap: !!process.env.TAURI_DEBUG,
        // FE-PERF-03 (#50): política de reparto de chunks. NO hay
        // `manualChunks` a propósito: el reparto lo hacen los límites de
        // `import()` dinámico (todas las vistas en `src/main.ts`), que Rollup
        // convierte en un chunk por vista más los chunks compartidos que
        // surjan. Confirmado con medición en la ventana real
        // (tools/nav-perf.sh, sonda ui/src/perf/navigationProbe.ts):
        // chunk principal 452.50 kB -> 399.00 kB raw y arranque (page origin
        // -> primera vista pintada) 1656 -> 1480 ms (mediana de 3 pasadas;
        // artefactos en ~/.local/state/syncify-fperf03/). Sin vendor split:
        // el harness mide el arranque en `vite dev`, donde manualChunks no
        // aplica, y separar vendor no reduce bytes ni parseo en disco local;
        // fragmentar por tamaño sin datos que lo justifiquen está vetado por
        // el issue. Verificación anti-huérfanos/anti-duplicación: grafo de
        // referencias entre chunks y `sources` de los sourcemaps (con
        // TAURI_DEBUG=1) — 24 chunks, todos alcanzables, cero fuentes en más
        // de un chunk.
    },
})




