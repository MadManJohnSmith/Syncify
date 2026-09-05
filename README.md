<div align="center">

# 🎵 SYNCIFY

**Orquestador y Descargador de Bibliotecas Musicales en Alta Fidelidad**  
*Sincronización Multi-Servicio · Metadatos Bit-Perfect · Estándar Symfonium · Motor Híbrido Rust/Tauri & Python*

[![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-Core%20Engine-DEA584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Vue 3](https://img.shields.io/badge/Vue.js-v3%20SFC-4FC08D?style=flat-square&logo=vue.js&logoColor=white)](https://vuejs.org/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-v4.0-38B2AC?style=flat-square&logo=tailwind-css&logoColor=white)](https://tailwindcss.com/)
[![SQLite WAL](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57?style=flat-square&logo=sqlite&logoColor=white)](https://www.sqlite.org/)


</div>

---

## 📖 Descripción General

**Syncify** es una aplicación de escritorio de alto rendimiento diseñada para audiófilos y coleccionistas de música. Permite unificar, importar, enriquecer y descargar bibliotecas desde múltiples servicios de streaming (**Qobuz**, **Tidal**, **Spotify**, **Deezer**, **Apple Music** y **SoundCloud**), preservando la máxima fidelidad de audio (FLAC Hi-Res hasta 24-bit/192kHz) y estructurando archivos idóneos para servidores locales y reproductores avanzados como **Symfonium** y **Plexamp**.

### Capacidades Principales
- 🎧 **Descargas en Alta Fidelidad:** Pipelines nativos en Rust para extracción y desencriptado directo en FLAC (DASH/Qobuz) con fallback transparente vía SongLink/Odesli.
- 🏷️ **Etiquetado y Metadatos Canónicos:** Extracción multi-proveedor (MusicBrainz, AcoustID, Last.fm) con VorbisComments independientes para artistas múltiples (`ARTIST`), bandas sonoras y compilaciones (`COMPILATION=1`, `ALBUMARTIST=Various Artists`).
- 📜 **Letras Sincronizadas:** Cascada de 16 estrategias de resolución en 10 proveedores (LRCLIB, Apple Music TTML, Musixmatch, NetEase, Kugou) con guardado automático de archivos sidecar `.lrc`.
- 🖼️ **Arte de Portada Seguro:** Gestión de carátulas estáticas de alta resolución (>=1000px) y portadas animadas en contenedores de video compatibles, previniendo sobrecargas de memoria (OOM) en reproductores móviles.
- ⚡ **Arquitectura Reactiva y Resiliente:** Interfaz moderna en Vue 3 y TailwindCSS comunicada por IPC tipado con un backend multihilo en Rust protegido contra deadlocks y colisiones de snapshot en SQLite WAL.

---

## 🏛️ Arquitectura del Sistema

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         CAPA DE PRESENTACIÓN                             │
│       Vue 3 (Composition API) + TailwindCSS v4 + Vite + Lucide Icons     │
└────────────────────────────────────┬─────────────────────────────────────┘
                                     │ IPC Tipado (tauri::invoke)
┌────────────────────────────────────▼─────────────────────────────────────┐
│                          TAURI v2 RUST CORE                              │
│  ├── Commands (Library, Playlists, Settings, Queue, Metadata, Auth)      │
│  ├── Download Orchestrator (Tidal Pipeline, Qobuz Client, SongLink)      │
│  ├── Background Workers & Concurrency Managers (Atomics + Tokio Notify)  │
│  └── Crates de Dominio:                                                  │
│      ├── syncify-core-domain       ├── syncify-flac-writer               │
│      ├── syncify-lyrics-domain     ├── syncify-metadata-domain           │
│      └── syncify-tidal-downloader                                        │
└───────────────────┬──────────────────────────────────┬───────────────────┘
                    │ Subprocesos Async                │ sqlx Pool (WAL)
┌───────────────────▼──────────────────┐   ┌───────────▼───────────────────┐
│       PYTHON BRIDGES & SCRAPERS      │   │      PERSISTENCIA SQLITE      │
│  Playwright (OAuth / Session Capt)   │   │  59 Migraciones Estructuradas │
│  Mutagen (Audio Tagging & Probing)   │   │  Ledger de Descargas          │
│  AcoustID (Chromaprint fpcalc)       │   │  track_sources (Llave Maestra)│
└──────────────────────────────────────┘   └───────────────────────────────┘
```

---

## 📋 Auditoría Integral y Plan de Remediación

El proyecto cuenta con un sistema canónico de auditoría continua y aseguramiento de calidad estructurado bajo estándares estrictos de solo lectura (`READ-ONLY ARCHITECT`), que unifica todos los diagnósticos previos (técnicos, de calidad de audio, de streaming y de Symfonium):

| Artefacto | Descripción | Acceso Directo |
|---|---|---|





| **`scripts/verify_audit_consistency.py`** | Validador continuo de integridad física, cobertura y aciclicidad matemática (DFS). | `python3 scripts/verify_audit_consistency.py` |








---

## 🚀 Requisitos y Puesta en Marcha

### Prerrequisitos
- **Rust:** `1.75.0` o superior (`cargo`, `rustc`).
- **Node.js:** `v18.0.0` o superior (`npm` o `pnpm`).
- **Python:** `3.10` a `3.12` con `pip` y `virtualenv`.
- **Herramientas de Sistema:** `ffmpeg` y `fpcalc` (Chromaprint) instalados en el `$PATH`.

### Instalación y Compilación
```bash
# 1. Clonar el repositorio
git clone https://github.com/MadManJohnSmith/Syncify.git
cd Syncify

# 2. Instalar dependencias de Frontend
cd ui
npm install
cd ..

# 3. Configurar entorno Python
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
playwright install chromium

# 4. Validar integridad de los artefactos de auditoría
python3 scripts/verify_audit_consistency.py

# 5. Ejecutar en modo desarrollo
cargo tauri dev
```

---

## 📂 Organización del Repositorio

- **`src-tauri/`**: Núcleo nativo de la aplicación de escritorio en Rust (comandos, servicios, base de datos y orquestador).
- **`crates/`**: Crates del workspace especializados en lógica de dominio, tags Vorbis, cliente Tidal y letras.
- **`ui/`**: Interfaz gráfica en Vue 3 con TailwindCSS, componentes modales, visualizadores y composables reactivos.
- **`scripts/`**: Puentes Python auxiliares para scrapers web, cálculos acústicos y automatización de navegador.


---

## 📄 Licencia

Este proyecto se distribuye bajo los términos de la licencia especificada en el repositorio. Consulta el archivo de licencia correspondiente para más detalles.
