<div align="center">

# Syncify

**Tu biblioteca musical, en máxima calidad y bajo tu control.**
Sincroniza, descarga y organiza tu música desde tus servicios de streaming favoritos, lista para Symfonium, Plexamp o cualquier reproductor local.

[![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-Core%20Engine-DEA584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Vue 3](https://img.shields.io/badge/Vue.js-v3%20SFC-4FC08D?style=flat-square&logo=vue.js&logoColor=white)](https://vuejs.org/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-v4.0-38B2AC?style=flat-square&logo=tailwind-css&logoColor=white)](https://tailwindcss.com/)
[![SQLite WAL](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57?style=flat-square&logo=sqlite&logoColor=white)](https://www.sqlite.org/)

</div>

---

## Qué es Syncify

Syncify es una aplicación de escritorio para quienes llevan su música en serio: conecta tus cuentas de streaming, importa tus favoritos y playlists completos, y descárgalos en la mejor calidad disponible, organizados y etiquetados como una colección profesional.

Funciona sobre tu propia biblioteca local (`~/Music/Syncify`), con carpetas por artista y álbum, portadas, letras y toda la información que reproductores como Symfonium y Plexamp necesitan para brillar.

## Qué puedes hacer con Syncify

- **Importa tu catálogo completo** de Qobuz, Tidal, Spotify, Deezer, Apple Music y SoundCloud: favoritos, playlists, compras, historial y apariciones.
- **Descarga en la calidad máxima disponible**, hasta FLAC Hi-Res de 24-bit/192kHz, con verificación de que lo descargado coincide con lo prometido.
- **Letras sincronizadas automáticas**: búsqueda en cascada entre 10 proveedores, guardadas como archivos `.lrc` junto a cada pista.
- **Metadatos de nivel profesional**: artistas múltiples, colaboraciones, compilaciones, códigos de país, BPM y más, extraídos de MusicBrainz, AcoustID y Last.fm.
- **Portadas en alta resolución**, incluidas portadas animadas compatibles con la pantalla de reproducción de Symfonium.
- **Playlists fieles al original**: orden, nombres y contenido preservados, con protección nativa contra duplicados.
- **Deduplicación inteligente** de toda tu biblioteca, incluso entre servicios distintos, sin perder tu pista preferida.
- **Todo automatizable**: programaciones y ejecución desatendida para mantener tu biblioteca al día.

## Requisitos e instalación

Requisitos: Rust 1.75+, Node.js 18+, Python 3.10-3.12, y `ffmpeg` + `fpcalc` (Chromaprint) en el PATH.

```bash
git clone https://github.com/MadManJohnSmith/Syncify.git
cd Syncify

# Frontend
cd ui && npm install && cd ..

# Entorno Python
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
playwright install chromium

# Ejecutar en modo desarrollo
cargo tauri dev
```

---

## Para desarrolladores

- **Arquitectura**: escritorio Tauri v2 con núcleo en Rust multihilo (workers, primitivas atómicas y Tokio Notify) sobre SQLite en modo WAL con 59 migraciones sqlx; UI en Vue 3 + TailwindCSS v4 comunicada por IPC tipado.
- **Crates de dominio**: `syncify-core-domain` (calidad, identidad), `syncify-flac-writer` (escritura Vorbis/FLAC validada), `syncify-lyrics-domain` (contrato compartido de la cascada de letras), `syncify-metadata-domain` y `syncify-tidal-downloader`.
- **Puentes Python**: Playwright (OAuth y captura de sesión), Mutagen (etiquetado), AcoustID/fpcalc (huellas acústicas).
- **Descargas**: pipelines nativos de desencriptado DASH (Qobuz) y cliente Tidal con política estricta de calidad y fallback vía SongLink/Odesli.
- **Pruebas**: suites de integración Rust por subsistema y especificaciones de Vitest en `ui/src/__tests__`.

---

## Aportar al proyecto

Las contribuciones son bienvenidas: bugs, mejoras de UI, nuevos proveedores de letras o metadatos, soporte de más servicios, documentación.

### 1. Prepara tu entorno

```bash
# Haz tu fork y clónalo
git clone https://github.com/TU_USUARIO/Syncify.git
cd Syncify

# Frontend
cd ui && npm install && cd ..

# Entorno Python (puentes de servicios)
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
playwright install chromium
```

### 2. Corre el proyecto en desarrollo

```bash
cargo tauri dev        # abre la app con recarga en caliente de la UI
```

Para probar solo el frontend con su servidor propio: `cd ui && npm run dev`.

### 3. Verifica tus cambios antes de proponerlos

```bash
cargo check                      # compila el backend y los crates
cargo test                       # suites de integración del backend
cd ui && npm run test:run        # tests del frontend
```

El CI del repositorio ejecuta además `cargo clippy` y `cargo fmt`; te recomendamos pasarlos en local (`cargo clippy`, `cargo fmt`) para que tu PR pase a la primera.

### 4. Envía tu contribución

- Crea una rama descriptiva y mantén los commits enfocados, con mensajes estilo convencional (`feat:`, `fix:`, `docs:`, `refactor:`).
- Si añades una funcionalidad visible, incluye cómo probarla; si tocas el pipeline de descargas o metadatos, añade o actualiza tests.
- Abre un Pull Request contra `syncify-graphical` describiendo el qué y el porqué del cambio.

### 5. Reporta bugs y propone ideas

Abre un [Issue](https://github.com/MadManJohnSmith/Syncify/issues) con: versión/OS, pasos para reproducir, resultado esperado vs. obtenido y logs relevantes si los hay (sin credenciales ni tokens personales).

---

## Licencia

Este proyecto se distribuye bajo los términos de la licencia especificada en el repositorio. Consulta el archivo de licencia correspondiente para más detalles.
