<div align="center">

# Syncify

**Toda tu biblioteca musical. Máxima calidad. Bajo tu control.**

Conecta Qobuz, Tidal, Spotify, Deezer, SoundCloud y Apple Music. Importa tu
catálogo completo, descárgalo en la mejor calidad disponible y conserva una
biblioteca local perfectamente organizada, con archivos reales, lista para
Symfonium, Plexamp o cualquier reproductor que quieras.

[English](README.md) · [Español](README.es.md)

**[⬇ Descarga la última versión](https://github.com/MadManJohnSmith/Syncify/releases/latest)**
· [Compila desde fuente](#compilar-desde-fuente) · [Documentación](docs/README.md)

[![Release](https://img.shields.io/github/v/release/MadManJohnSmith/Syncify?style=flat-square&logo=github)](https://github.com/MadManJohnSmith/Syncify/releases/latest)
[![CI](https://github.com/MadManJohnSmith/Syncify/actions/workflows/ci.yml/badge.svg)](https://github.com/MadManJohnSmith/Syncify/actions/workflows/ci.yml)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-Core%20Engine-DEA584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Vue 3](https://img.shields.io/badge/Vue.js-v3-4FC08D?style=flat-square&logo=vue.js&logoColor=white)](https://vuejs.org/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-v4-38B2AC?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![SQLite](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57?style=flat-square&logo=sqlite&logoColor=white)](https://www.sqlite.org/)

**Windows** instalador y portable · **Linux** AppImage, DEB y tarball

</div>

---

## Véalo en acción

![Panel de Syncify — resumen de biblioteca, fuentes por servicio y estadísticas de descarga](docs/screenshots/dashboard.png)

| | |
|---|---|
| ![Library — todo tu catálogo en una tabla](docs/screenshots/library.png) | ![Letras sincronizadas con el reproductor integrado](docs/screenshots/lyrics.png) |
| *Todo tu catálogo, unificado* | *Letras sincronizadas, guardadas como .lrc* |
| ![Downloads — cola con hilos y reintentos](docs/screenshots/downloads.png) | ![Seis servicios conectados en el onboarding](docs/screenshots/services.png) |
| *Un motor de descargas que controlas* | *Un login por servicio, todo queda sincronizado* |

---

## ¿Por qué Syncify?

Los servicios de streaming te alquilan la música. Si te vas — o el servicio
pierde una licencia, o quitan una pista — tu biblioteca se va contigo. Syncify
le da la vuelta: convierte "tu" catálogo en *tu* catálogo.

- **Cambia de servicio sin dejar tu música atrás.** Importa todo (favoritos,
  playlists, compras e historial) y migra a otro servicio, o descárgalo y
  sé dueño de verdad.
- **Archivos reales, no licencias.** Las descargas quedan como FLAC
  correctamente etiquetado (hasta Hi-Res 24-bit/192 kHz) o MP3, verificado
  contra lo que el proveedor prometió.
- **Una biblioteca, todos los servicios.** Tus favoritos se sincronizan en
  ambos sentidos entre los servicios que uses, con deduplicación inteligente
  que reconoce la misma canción venga de donde venga.

## ¿Qué puedes hacer con Syncify?

- **Importa tu catálogo completo** de Qobuz, Tidal, Spotify, Deezer,
  SoundCloud y Apple Music — pistas, álbumes, artistas y playlists.
- **Migra entre servicios**: elige origen, elige destino, revisa las
  coincidencias y transfiere. Tus playlists viajan con su orden y nombres
  intactos.
- **Descarga con cascada de calidad**: el motor siempre apunta al mejor
  nivel ofrecido para cada pista, baja al siguiente cuando no está y
  verifica cada archivo que escribe.
- **Metadatos de nivel profesional automáticos**: artistas múltiples,
  colaboraciones y compilaciones bien resueltas, países, BPM, tonalidad y
  energía — desde MusicBrainz, AcoustID y Last.fm.
- **Letras sincronizadas como `.lrc`**, resueltas con una cascada de 16
  estrategias sobre 10 proveedores.
- **Portadas en alta resolución** (incluidas animadas) guardadas junto a tu
  música, organizadas en carpetas por artista y álbum bajo `~/Music/Syncify`.
- **Manténla sana**: deduplicación y fusión, auditorías de identidad del
  catálogo, chequeos de integridad y un pipeline de reparación con historial
  auditable.
- **Playlists inteligentes** con reglas, construidas sobre tu propia
  biblioteca.

## Cómo funciona

1. **Conecta una cuenta.** La app abre un flujo de login seguro por servicio
   (OAuth/captura de sesión en el navegador) y guarda tus credenciales
   cifradas en tu máquina.
2. **Importa.** Un motor unificado en Rust recorre la API del servicio con
   paginación, resuelve la identidad canónica de cada pista (ISRC, IDs del
   proveedor), la enriquece y la persiste transaccionalmente, con retry,
   en tu biblioteca SQLite local.
3. **Descarga.** Pipelines nativos manejan el formato de entrega de cada
   proveedor (incluido el desencriptado DASH de Qobuz), escriben archivos
   FLAC/MP3 reales con metadatos, portadas y letras embebidas, y verifican
   que lo que aterrizó en disco coincide con lo prometido.
4. **Disfrútala donde quieras.** El resultado es un árbol de carpetas plano
   con archivos sidecar — sin encierro propietario. Apunta Symfonium,
   Plexamp, Roon o cualquier reproductor y funciona.

Bajo el capó: una app de escritorio **Tauri 2** (núcleo Rust + UI en Vue 3)
con IPC tipado, un motor de descarga/reparación multihilo y un esquema local
endurecido a lo largo de **83 migraciones SQL**. Los logins de servicios
corren por puentes Python pequeños y auditados (Playwright, Mutagen, AcoustID).

### Diagramas

Los dos son HTML interactivos y autónomos — ábrelos en un navegador, sigue el
camino y amplía la parte que te interese.

| | |
|---|---|
| **Arquitectura**<br>Cómo encajan una UI en Vue 3, un núcleo Rust, SQLite, el keyring del sistema y los puentes Python — y dónde viven realmente tus credenciales. | **Publicación protegida**<br>Por qué un tag de versión no se puede mover ni borrar antes de que CI esté en verde en ese commit exacto. |
| [EN](docs/diagrams/architecture.en.html) · [ES](docs/diagrams/architecture.html) | [EN](docs/diagrams/release-gated.en.html) · [ES](docs/diagrams/release-gated.html) |

**Arquitectura** — la UI se comunica con el núcleo Rust por IPC tipado; el núcleo
es dueño de la biblioteca SQLite y habla con el keyring del sistema, así que las
credenciales nunca se guardan en la base de datos. En paralelo corren los workers
y los adaptadores de cada servicio, y los puentes Python se encargan del trabajo
de audio y metadatos a través de `cmd_utils`, que resuelve sus rutas según el
empaquetado instalado (AppImage, instalador o código fuente).

**Publicación protegida** — `main` exige un pull request con los tres checks de CI
en verde, y una puerta reutilizable se niega a publicar si el commit exacto no
pasó CI o si el tag ya apuntaba a otro sitio. Los tags de versión `v*` no se
pueden mover ni borrar.

## Servicios soportados

| | Favoritos | Álbumes/Artistas | Playlists | Historial | Destino de migración |
|---|---|---|---|---|---|
| **Qobuz** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Tidal** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Spotify** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Deezer** | ✅ | ✅ | ✅ | — ⁽¹⁾ | ✅ |
| **SoundCloud** | ✅ | ✅ ⁽²⁾ | — ⁽¹⁾ | — ⁽¹⁾ | ✅ |
| **Apple Music** | ✅ ⁽³⁾ | ✅ ⁽³⁾ | ✅ | — | ✅ |

⁽¹⁾ La API pública del proveedor no lo expone — Syncify avisa en vez de fingir.
⁽²⁾ Se derivan del `publisher_metadata` de cada like; la API no tiene endpoint de álbumes favoritos.
⁽³⁾ Apple Music importa por su propio importador fiel al ISRC (storefront
configurable); el camino del motor unificado está documentado en la matriz
de paridad.

La matriz completa y honesta por fase vive en
[`docs/MATRIZ_PARIDAD_IMPORTACION.md`](docs/MATRIZ_PARIDAD_IMPORTACION.md).

## Instalación

Descarga la última build de la página de
[**Releases**](https://github.com/MadManJohnSmith/Syncify/releases):

| Plataforma | Archivos |
|---|---|
| **Windows** | `Syncify_*_x64-setup.exe` (instalador) · `Syncify-Windows-Portable.zip` |
| **Linux** | `*.AppImage` · `*.deb` · `Syncify-Linux-x86_64.tar.gz` (binario crudo + puentes) |

Después:

1. Instala y abre Syncify: `ffmpeg`, `ffprobe` y `fpcalc` (Chromaprint)
   **viajan dentro de cada paquete**, así que no hay nada más que descargar,
   instalar ni configurar.
2. Corre el asistente de primera ejecución: conecta tus cuentas (las
   credenciales se guardan en el llavero de tu sistema — sin archivos de
   configuración), elige tu carpeta de música, listo.

> ¿Compilando desde fuente? Mira
> [`src-tauri/binaries/README.md`](src-tauri/binaries/README.md) para saber
> cómo se proveen los binarios empaquetados en builds locales de producción.

## Preguntas frecuentes

**¿Sobrevive mi biblioteca si desinstalo Syncify?**

Sí. Las descargas son archivos FLAC/MP3 en carpetas normales, con portadas y
letras `.lrc` como sidecars. Ninguna base de datos propietaria las mantiene
juntas: Symfonium, Plexamp, Roon o cualquier reproductor las lee
directamente.

**¿Dónde viven las credenciales de mis servicios?**

En el llavero de tu sistema operativo, cifradas en tu máquina. Cada login
corre por tu navegador; nada se escribe en archivos de configuración planos.

**¿Qué calidad puedo esperar?**

El motor apunta al mejor nivel que cada servicio ofrezca (FLAC hasta Hi-Res
24-bit/192 kHz) y baja de nivel pista por pista. Cada archivo se verifica
contra lo que el proveedor prometió antes de darse por bueno.

**¿Hay versión para macOS?**

Todavía no. Windows (instalador y portable) y Linux (AppImage, DEB, tarball)
son las plataformas soportadas hoy.

## Compilar desde fuente

Requisitos: **Rust** (estable), **Node.js 20+**, **Python 3.11+** y `ffmpeg`,
`flac` y `fpcalc` en tu `PATH`. En Linux además necesitas los paquetes de
desarrollo de WebKit2GTK/GTK (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev`).

```bash
git clone https://github.com/MadManJohnSmith/Syncify.git
cd Syncify

# CLI de Tauri (raíz del repo) + dependencias del frontend
npm install
cd ui && npm ci && cd ..

# Puentes Python (logins de servicios, metadatos, fingerprinting)
python3 -m venv .venv
source .venv/bin/activate
pip install -r scripts/requirements.txt
playwright install chromium

# Modo desarrollo (con recarga en caliente de la UI)
npm run dev

# Build de producción (instalador para tu SO)
npm run build
```

## Documentación

- [`docs/`](docs/README.md) — arquitectura y features
- [`docs/MATRIZ_PARIDAD_IMPORTACION.md`](docs/MATRIZ_PARIDAD_IMPORTACION.md) — matriz viva de paridad de importación
- [`docs/LYRICS_16_PROVIDER_MATRIX.md`](docs/LYRICS_16_PROVIDER_MATRIX.md) — la cascada de letras, auditada
- [`docs/Deuda_Tecnica_y_UX.md`](docs/Deuda_Tecnica_y_UX.md) — deuda técnica abierta, rastreada a la vista

## Contribuir

Las contribuciones son bienvenidas: bugs, mejoras de UI, nuevos proveedores
de letras o metadatos, más servicios, documentación. Haz fork, crea una rama
enfocada y abre un PR contra `syncify-app` (la rama de desarrollo; `main`
recibe merges verificados para releases). Corre los checks antes de enviar —
`cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check` y
`cd ui && npm run test:run` — el CI ejecuta exactamente esos.

¿Encontraste un bug? [Abre un issue](https://github.com/MadManJohnSmith/Syncify/issues)
con tu SO/versión, pasos para reproducir y logs relevantes — nunca
credenciales ni tokens personales.

## Licencia

Este repositorio no concede actualmente una licencia de uso, copia,
modificación o redistribución. Todos los derechos están reservados por sus
titulares. Antes de cualquier distribución pública se definirá y versionará
una licencia explícita.

---

<div align="center">

<sub>Sincroniza · Descarga · Organiza — <a href="README.md">English version</a></sub>

</div>
