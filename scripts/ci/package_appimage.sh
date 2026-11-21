#!/usr/bin/env bash
# Empaquetado del AppImage de Syncify (#243).
#
# Un solo ciclo extract -> inyectar -> repack por corrida, consumido como
# llamada fina por .github/workflows/build-linux.yml (y ejecutable en local
# contra fixtures: los tests de scripts/tests/test_package_appimage.py
# inyectan gst-inspect-1.0, appimagetool y curl falsos por PATH; sin red).
#
# El workflow anterior extraia y reempaquetaba DOS veces (GStreamer por un
# lado, Python por otro) y descargaba appimagetool `continuous` dos veces sin
# pin ni checksum. Aqui:
#
#   1. extract UNA vez del AppImage que produce `tauri build`.
#   2. Inyeccion de GStreamer: librerias localizadas por la capacidad que
#      reporta gst-inspect, no por nombre de fichero.
#   3. Inyeccion del runtime Python autonomo + scripts de los puentes.
#   4. Borrado con guarda de lo que envenena al anfitrion (libwayland vieja,
#      xdg-open viejo de #212).
#   5. Repack UNA vez conservando AppRun y hooks, con appimagetool 1.9.1
#      pineado a una release fija con SHA-256.
#
# Uso:
#   package_appimage.sh [--appimage PATH] [--python-dir PATH] [--scripts-dir PATH]
#
# Sin --appimage se descubre el AppImage de `tauri build` bajo
# target/release/bundle/appimage. Los defaults relativos resuelven contra la
# raiz del repo (dos niveles arriba de este script), no contra el cwd.
#
# Entorno (seams de los tests contra fixtures; en CI nadie los exporta):
#   SYNCIFY_GST_LIB_DIRS             directorios donde buscar las librerias de
#                                    los sinks de runtime (alsa/pulse/pipewire/
#                                    jack). Default: "/usr/lib /lib".
#   SYNCIFY_APPIMAGETOOL_FROM_PATH   si es 1 y hay un appimagetool en el PATH,
#                                    se usa ese en vez de descargar el pineado.

set -euo pipefail

# appimagetool pineado a una release fija con checksum: es maquinaria de
# construccion, no binario publicado, asi que NO entra en la tabla del
# proveedor (scripts/dependency_manager.py). El `continuous` que usaba el
# workflow es una build rolling sin hash, descargada dos veces por corrida:
# un push upstream podia cambiar el empaquetado entre un release y otro.
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
APPIMAGETOOL_SHA256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"

die() {
  echo "::error::$*" >&2
  exit 1
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

APPIMAGE=""
PYTHON_DIR="$REPO_ROOT/bundle/python"
SCRIPTS_DIR="$REPO_ROOT/scripts"

while [ $# -gt 0 ]; do
  case "$1" in
    --appimage) APPIMAGE="${2:?--appimage necesita una ruta}"; shift 2 ;;
    --python-dir) PYTHON_DIR="${2:?--python-dir necesita una ruta}"; shift 2 ;;
    --scripts-dir) SCRIPTS_DIR="${2:?--scripts-dir necesita una ruta}"; shift 2 ;;
    *) die "argumento desconocido: $1" ;;
  esac
done

if [ -z "$APPIMAGE" ]; then
  shopt -s nullglob
  candidates=("$REPO_ROOT/target/release/bundle/appimage/"*.AppImage)
  shopt -u nullglob
  if [ "${#candidates[@]}" -eq 0 ]; then
    die "no hay ningun AppImage en target/release/bundle/appimage; ejecuta antes 'tauri build'"
  fi
  APPIMAGE="${candidates[0]}"
fi
if [ ! -f "$APPIMAGE" ]; then
  die "el AppImage no existe: $APPIMAGE"
fi
APPIMAGE_ABS="$(cd "$(dirname "$APPIMAGE")" && pwd)/$(basename "$APPIMAGE")"

if [ ! -d "$PYTHON_DIR" ]; then
  die "falta el runtime Python autonomo: $PYTHON_DIR (ejecuta antes 'dependency_manager.py install-runtime')"
fi
if [ ! -d "$SCRIPTS_DIR" ]; then
  die "falta el arbol de scripts de los puentes: $SCRIPTS_DIR"
fi

# ── 1. Extract UNA vez ─────────────────────────────────────────────────────
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
(
  cd "$WORK"
  "$APPIMAGE_ABS" --appimage-extract >/dev/null
)
ROOT="$WORK/squashfs-root"
if [ ! -d "$ROOT" ]; then
  die "el AppImage no extrajo squashfs-root"
fi
if [ ! -x "$ROOT/AppRun" ]; then
  die "el AppDir extraido no tiene AppRun ejecutable"
fi

# ── 2. Inyeccion de GStreamer ──────────────────────────────────────────────
# WebKitGTK delega la reproduccion HTML5 a GStreamer. El AppImage empaqueta
# las librerias (linuxdeploy las arrastra por dependencia de webkit2gtk) pero
# NINGUN plugin: sin /usr/lib/gstreamer-1.0 no existe autoaudiosink y la
# reproduccion falla con
# "GStreamer element autoaudiosink not found. Please install it".
# Se añaden los plugins minimos de audio y video al AppImage.
# autoaudiosink/autovideosink viven en gst-plugins-good, que NO viene
# instalado en los runners: sin el, no hay nada que copiar (la primera
# iteracion empaqueto 13 plugins y ninguno de sink).
GST_LIB_DIRS="${SYNCIFY_GST_LIB_DIRS:-/usr/lib /lib}"
PLUGDIR="$ROOT/usr/lib/gstreamer-1.0"
mkdir -p "$PLUGDIR"

gst-inspect-1.0 > "$WORK/gst-elements.txt"
: > "$WORK/libs.txt"

# Elementos imprescindibles para reproducir audio y video. Esta lista si es
# una decision de producto: es lo que la app necesita.
#
# La comprobacion la hace GStreamer, no una lista de nombres de fichero. Una
# version anterior buscaba un .so por sinks, nombre que no existe:
# autoaudiosink y autovideosink son dos elementos dentro de una misma
# libreria, asi que buscar por nombre decia que faltaban cuando no faltaban.
#
# gst-inspect lista como "plugin:  elemento: descripcion", asi que el
# elemento NO va al principio de la linea. Con "^$el:" no coincidia nunca y
# el paso rechazaba un AppImage con los sinks correctos.
REQUIRED="autoaudiosink autovideosink playbin decodebin audioconvert
           audioresample videoconvert typefindfunctions"
for el in $REQUIRED; do
  if ! grep -qE "(^|[[:space:]])$el:" "$WORK/gst-elements.txt"; then
    die "elemento obligatorio ausente del registro de gst-inspect: $el"
  fi
  # La ruta real de la libreria que aporta ese elemento. Cada libreria se
  # copia segun la ruta que dice gst-inspect, no segun un nombre supuesto:
  # este paso fallo tres veces seguidas por hardcodear nombres de fichero
  # que no existen (libgstautoaudiosink.so, libgstautovideosink.so y
  # libgstautosink.so). autoaudiosink y autovideosink son ELEMENTOS, y
  # gst-inspect dice que libreria aporta cada uno.
  #
  # gst-inspect separa "Filename" del valor con espacios y SIN dos puntos,
  # asi que un sed que busque "Filename:" no extrae nada. Lo que funciona es
  # tomar, en esa linea, el primer token que empieza por "/".
  lib=$(gst-inspect-1.0 "$el" 2>/dev/null | awk '/^[[:space:]]*Filename/ {for(i=1;i<=NF;i++) if($i ~ /^\//) {print $i; exit}}')
  if [ -z "$lib" ] || [ ! -f "$lib" ]; then
    die "gst-inspect no dio una ruta existente para $el (dio: ${lib:-nada})"
  fi
  echo "$lib" >> "$WORK/libs.txt"
done

# Los sinks entre los que el automatico elige en runtime segun la maquina del
# usuario: ALSA, PulseAudio, PipeWire y JACK.
for lib in $(find $GST_LIB_DIRS -name 'libgstalsa.so' \
                -o -name 'libgstpulseaudio.so' \
                -o -name 'libgstpipewire.so' \
                -o -name 'libgstjack.so' 2>/dev/null | sort -u); do
  echo "$lib" >> "$WORK/libs.txt"
done

sort -u "$WORK/libs.txt" | while read -r lib; do
  cp -a "$lib" "$PLUGDIR/"
done

echo "Plugins de GStreamer copiados: $(ls -1 "$PLUGDIR" | wc -l)"
ls -1 "$PLUGDIR"

# Sin sink de audio el AppImage arranca pero no suena. Publicar un paquete
# roto es peor que fallar la build. Se comprueba por capacidad:
# PREGUNTANDO cual es la libreria de cada sink, no comparando con un nombre
# supuesto.
for el in autoaudiosink autovideosink; do
  lib=$(gst-inspect-1.0 "$el" 2>/dev/null | awk '/^[[:space:]]*Filename/ {for(i=1;i<=NF;i++) if($i ~ /^\//) {print $i; exit}}')
  if [ ! -f "$PLUGDIR/$(basename "$lib")" ]; then
    die "la libreria de $el no llego al AppImage: ${lib:-desconocida}"
  fi
done

# ── 3. Inyeccion del runtime Python y los scripts ─────────────────────────
# get_project_root sube desde usr/bin y encuentra <AppDir>/resources/
# (scripts + python); get_python_executable devuelve entonces
# resources/python/bin/python. Espejo del python-embed de Windows.
mkdir -p "$ROOT/resources/scripts"
cp -a "$PYTHON_DIR" "$ROOT/resources/python"
cp "$SCRIPTS_DIR"/*_bridge.py "$SCRIPTS_DIR/dependency_manager.py" \
  "$SCRIPTS_DIR/requirements.txt" "$ROOT/resources/scripts/"
cp -a "$SCRIPTS_DIR/services" "$ROOT/resources/scripts/services"
rm -rf "$ROOT/resources/scripts/services/__pycache__"

# ── 4. Borrado con guarda del envenenamiento al anfitrion ─────────────────
# Las libwayland-* que linuxdeploy arrastra por dependencia de webkit2gtk
# (copias de Ubuntu 22.04, wayland 1.20) envenenan el EGL del anfitrion:
# dentro del proceso ensombrecen a las del sistema via LD_LIBRARY_PATH y la
# Mesa reciente necesita simbolos que 1.20 no exporta (p.ej.
# wl_fixes_interface, wayland >= 1.22). El dlopen del vendor EGL
# (libEGL_mesa) falla, glvnd responde EGL_BAD_PARAMETER y el
# WebKitWebProcess muere con "Could not create default EGL display:
# EGL_BAD_PARAMETER. Aborting...", dejando la ventana en blanco. Reproducido
# en CachyOS (Mesa 26.2.4): con las copias empaquetadas aborta siempre
# (WEBKIT_DISABLE_COMPOSITING_MODE ni LIBGL_ALWAYS_SOFTWARE lo evitan); sin
# ellas el WebProcess vive. Cualquier anfitrion capaz de correr GTK3 tiene
# libwayland en el sistema y es retrocompatible con el binario de webkit. El
# smoke de arranque del final no puede ver esto: la Mesa del runner si es
# compatible con las copias 1.20.
#
# El borrado es tolerante y la guarda es el fallo explicito: si el fichero
# sigue ahi (por el motivo que sea), la build muere, no se publica.
rm -f "$ROOT"/usr/lib/libwayland-*.so* 2>/dev/null || true
if ls "$ROOT"/usr/lib/libwayland-*.so* >/dev/null 2>&1; then
  die "las libwayland empaquetadas siguen en el AppDir; envenenarian el EGL del anfitrion"
fi

# #212: linuxdeploy arrastra su propio usr/bin/xdg-open (xdg-utils 1.1.3,
# base Ubuntu 22.04) y el AppRun antepone el bin/ del AppDir al PATH, de modo
# que ese xdg-open viejo ensombrece al del anfitrion para cualquier hijo de
# la app. En Plasma 6 (KDE_SESSION_VERSION=6) el open_kde() de 1.1.x no tiene
# rama y sale 0 sin abrir nada: el login Tidal cree que abrio la pestana y
# sondea 300 s, reveal/open y los enlaces externos se rompen igual. Sin copia
# empaquetada, el PATH resuelve el xdg-open (y gio, kde-open) del sistema,
# que si conoce el escritorio del usuario. Misma medicina que las libwayland
# de arriba: la copia vieja del paquete no mejora al anfitrion, lo envenena.
# Borrado tolerante y guarda explicita, igual que las libwayland.
rm -f "$ROOT/usr/bin/xdg-open" 2>/dev/null || true

# En la linea de la asercion de sinks de arriba: si el fichero sigue en el
# AppDir que se va a empaquetar, el AppImage saldria con el bug de #212
# dentro y nadie lo veria. La build falla, no se publica.
if [ -e "$ROOT/usr/bin/xdg-open" ]; then
  die "el xdg-open empaquetado sigue en el AppDir; ensombreceria al del anfitrion (#212)"
fi

# ── 5. Repack UNA vez ──────────────────────────────────────────────────────
# appimagetool resuelto UNA sola vez por corrida: se descarga el pineado y se
# verifica su SHA-256 ANTES de ejecutarlo. La unica alternativa es el seam
# explicito de los tests (SYNCIFY_APPIMAGETOOL_FROM_PATH=1 mas un fake en el
# PATH): en CI nadie exporta la bandera, asi que el binario usado es SIEMPRE
# el pineado, sin importar lo que traiga la imagen del runner.
ensure_appimagetool() {
  if [ "${SYNCIFY_APPIMAGETOOL_FROM_PATH:-0}" = "1" ] && command -v appimagetool >/dev/null 2>&1; then
    command -v appimagetool
    return
  fi
  local dest="$WORK/tools/appimagetool"
  mkdir -p "$(dirname "$dest")"
  if ! curl -sL --fail -o "$dest" "$APPIMAGETOOL_URL"; then
    rm -f "$dest"
    die "la descarga de appimagetool fallo: $APPIMAGETOOL_URL"
  fi
  if ! echo "$APPIMAGETOOL_SHA256  $dest" | sha256sum -c - >/dev/null 2>&1; then
    rm -f "$dest"
    die "el SHA-256 del appimagetool descargado no coincide con el pin ($APPIMAGETOOL_SHA256); no se ejecuta"
  fi
  chmod +x "$dest"
  printf '%s' "$dest"
}

# appimagetool (el `continuous` de antes) rechaza directorios con modo 0700;
# los recursos que Tauri empaqueta conservan permisos restrictivos.
chmod -R a+rX "$ROOT"

APPIMAGETOOL_BIN="$(ensure_appimagetool)"

# Reempaquetar con appimagetool (conserva el AppRun y los hooks).
(
  cd "$ROOT"
  "$APPIMAGETOOL_BIN" --no-appstream . "$APPIMAGE_ABS.new"
)
mv "$APPIMAGE_ABS.new" "$APPIMAGE_ABS"

echo "AppImage reempaquetado: $APPIMAGE_ABS"
ls -la "$(dirname "$APPIMAGE_ABS")"
