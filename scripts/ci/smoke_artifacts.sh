#!/usr/bin/env bash
# Smokes de los tres artefactos Linux publicables (#245).
#
# Dos fallos pueden salir de aqui sin que ningun paso anterior los vea:
#
#   1. imports: el CPython montado en el tarball, el AppImage y el DEB debe
#      importar los diez modulos canonicos. La lista y el env limpio (la app
#      borra PYTHONHOME/PYTHONPATH al arrancar, main.rs) viven en el
#      proveedor: dependency_manager.py, smoke-imports. Aqui solo se elige el
#      interprete de cada artefacto; la lista no se duplica en bash.
#   2. arranque real: el abort de EGL ("Could not create default EGL display:
#      EGL_BAD_PARAMETER. Aborting...") mata el proceso entero y el smoke de
#      imports no lo ve. Se lanza el AppRun EXTRAIDO bajo xvfb y se exige que
#      la ventana siga viva hasta el corte sin ese abort. El runner no tiene
#      GPU: esto cubre el arranque, NO la combinacion (Iris Xe + Wayland) del
#      fallo reportado.
#
# Se lanza el AppRun y no el binario pelado: usr/bin/syncify-tauri a pelo deja
# el proceso sin el LD_LIBRARY_PATH y los hooks GTK que AppRun exporta, y
# WebKit resuelve sus procesos auxiliares en una ruta relativa al cwd
# ("././/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitNetworkProcess") hasta
# morir con un abort que no es culpa del paquete. Reproducido con el AppImage
# de v0.3.0: directo falla (exit 133), via AppRun vive.
#
# Uso: smoke_artifacts.sh [--dist-dir PATH] [--apprun-timeout SEGUNDOS]
#
# Defaults: dist/linux de la raiz del repo y 45 s. Los artefactos los deja el
# job anterior (assemble_raw.sh, package_deb.sh, package_appimage.sh).
# xvfb-run debe estar instalado (el job provisiona el paquete xvfb): su
# ausencia es un fallo explicito, no un skip silencioso.
#
# Probado contra fixtures por scripts/tests/test_ci_smoke_artifacts.py:
# AppImage, dpkg-deb y xvfb-run falsos por PATH e interpretes que registran su
# invocacion. Sin red y sin toolkit real.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

DIST_DIR="$REPO_ROOT/dist/linux"
APPRUN_TIMEOUT=45

while [ $# -gt 0 ]; do
  case "$1" in
    --dist-dir) DIST_DIR="${2:?--dist-dir necesita una ruta}"; shift 2 ;;
    --apprun-timeout) APPRUN_TIMEOUT="${2:?--apprun-timeout necesita segundos}"; shift 2 ;;
    *) echo "::error::argumento desconocido: $1" >&2; exit 2 ;;
  esac
done

fail() {
  echo "::error::$1"
  exit 1
}

[[ $APPRUN_TIMEOUT =~ ^[0-9]+$ ]] || fail "--apprun-timeout debe ser un entero de segundos: $APPRUN_TIMEOUT"
[[ -d $DIST_DIR ]] || fail "no existe el directorio de artefactos: $DIST_DIR"

PROVIDER="$REPO_ROOT/scripts/dependency_manager.py"
[[ -f $PROVIDER ]] || fail "no existe el proveedor del runtime: $PROVIDER"

# Las mismas tres variables que la app borra al arrancar y que el proveedor
# quita del subproceso: un valor del anfitrion no puede ensuciar el smoke.
PY_ENV=(env -u PYTHONHOME -u PYTHONPATH -u PYTHONDONTWRITEBYTECODE)

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# smoke_imports ETIQUETA INTERPRETE [ASIGNACION...]
# La lista canonica y la limpieza del entorno del interprete son del modulo.
smoke_imports() {
  local label=$1 python=$2
  shift 2
  echo "== Smoke python: $label =="
  "${PY_ENV[@]}" env "$@" python3 "$PROVIDER" smoke-imports --python "$python"
  echo "OK $label"
}

# ── Tarball crudo ────────────────────────────────────────────────────
# Se prueba el .tar.gz final, no el arbol Syncify-Linux-Raw que queda al
# lado: el artefacto que se publica y se descarga es el tarball.
TARBALL="$DIST_DIR/Syncify-Linux-x86_64.tar.gz"
[[ -f $TARBALL ]] || fail "no existe el tarball final: $TARBALL"
mkdir -p "$WORK/tarball"
tar -xzf "$TARBALL" -C "$WORK/tarball" || fail "no se pudo extraer el tarball: $TARBALL"
PY_TARBALL="$WORK/tarball/Syncify-Linux-Raw/python/bin/python3"
[[ -x $PY_TARBALL ]] || fail "el tarball no trae Syncify-Linux-Raw/python/bin/python3"
smoke_imports tarball "$PY_TARBALL"

# ── AppImage ─────────────────────────────────────────────────────────
shopt -s nullglob
APPIMAGES=("$DIST_DIR"/*.AppImage)
shopt -u nullglob
(( ${#APPIMAGES[@]} > 0 )) || fail "no hay ningun AppImage en $DIST_DIR"
APPIMAGE="${APPIMAGES[0]}"
APPIMAGE_ABS="$(cd "$(dirname "$APPIMAGE")" && pwd)/$(basename "$APPIMAGE")"

mkdir -p "$WORK/appimage"
( cd "$WORK/appimage" && "$APPIMAGE_ABS" --appimage-extract >/dev/null ) \
  || fail "el AppImage no se pudo extraer con --appimage-extract: $APPIMAGE_ABS"
AI_ROOT="$WORK/appimage/squashfs-root"
[[ -d $AI_ROOT ]] || fail "el AppImage no produjo squashfs-root al extraer"
PY_APPIMAGE="$AI_ROOT/resources/python/bin/python"
[[ -x $PY_APPIMAGE ]] || fail "el AppImage no trae resources/python/bin/python"

# LD_LIBRARY_PATH replica el entorno que deja AppRun: si una libreria del
# AppDir sombrease el libpython del bundle, este es el sitio donde se ve.
smoke_imports appimage "$PY_APPIMAGE" LD_LIBRARY_PATH="$AI_ROOT/usr/lib"

# ── DEB ──────────────────────────────────────────────────────────────
shopt -s nullglob
DEBS=("$DIST_DIR"/*.deb)
shopt -u nullglob
(( ${#DEBS[@]} > 0 )) || fail "no hay ningun DEB en $DIST_DIR"
DEB="${DEBS[0]}"
command -v dpkg-deb >/dev/null 2>&1 || fail "dpkg-deb no esta en PATH: no se puede extraer el DEB"
mkdir -p "$WORK/deb"
dpkg-deb -x "$DEB" "$WORK/deb" || fail "dpkg-deb no pudo extraer el DEB: $DEB"
# package_deb.sh monta bundle/python en _up_/python: el interprete contractual
# del runtime es bin/python3.
PY_DEB="$(find "$WORK/deb" -path '*_up_/python/bin/python3' -print -quit)"
[[ -n $PY_DEB && -x $PY_DEB ]] || fail "el DEB final no contiene _up_/python/bin/python3"
smoke_imports deb "$PY_DEB"

# ── Guarda EGL/xvfb: arranque real del AppRun extraido ───────────────
command -v xvfb-run >/dev/null 2>&1 || fail "no se encontro xvfb-run (paquete xvfb): la guarda EGL lo necesita"

APPRUN="$AI_ROOT/AppRun"
if [[ ! -x $APPRUN ]]; then
  echo "::error::no se encontro AppRun ejecutable dentro del AppImage"
  ls -R "$AI_ROOT" | head -40 >&2 || true
  exit 1
fi

LOG="$WORK/apprun.log"
set +e
# El mismo env limpio que los smokes: un PYTHONHOME del anfitrion rompe el
# interprete de los procesos hijos antes de que la app llegue a limpiarlo.
# 124 es el codigo de `timeout` cuando el proceso aguanto hasta el corte.
"${PY_ENV[@]}" timeout --signal=INT "$APPRUN_TIMEOUT" xvfb-run -a "$APPRUN" >"$LOG" 2>&1
code=$?
set -e
cat "$LOG"

if grep -q "Could not create default EGL display" "$LOG"; then
  fail "el AppImage aborta al crear el display EGL"
fi
if grep -q "Unable to spawn a new child process" "$LOG"; then
  fail "el AppImage no logra lanzar los procesos auxiliares de WebKit"
fi
if ! grep -q "Syncify starting" "$LOG"; then
  fail "la app no llego al arranque: falta 'Syncify starting' en el log (arriba)"
fi
if [[ $code -ne 124 ]]; then
  fail "la app salio con codigo $code en vez de seguir viva (log arriba)"
fi
echo "Arranque verificado: ventana viva ${APPRUN_TIMEOUT} s sin abort de EGL"
