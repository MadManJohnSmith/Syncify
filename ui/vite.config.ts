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
    },
})




