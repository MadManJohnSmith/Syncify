#!/usr/bin/env bash
# Inyeccion del runtime Python autonomo en un DEB de Tauri (#244).
#
# Tauri v2 aplana los recursos con el prefijo _up_ (su forma de resolver el
# ../ de ../scripts), asi que los puentes viven en usr/lib/<producto>/_up_/.
# El resolutor de la app (cmd_utils::packaged_python_candidates) busca un
# python/ HERMANO de ese _up_/scripts: get_project_root encuentra scripts/ y
# get_python_executable devuelve <scripts>/../python/bin/python3.
#
# Este script monta ahi el arbol que `dependency_manager.py install-runtime`
# produce en bundle/python (CPython standalone 3.11.9 con requirements ya
# instalados; su forma y su smoke viven en el modulo del proveedor, no aqui).
#
# Tras la inyeccion el manifest queda cojo: dpkg no regenera md5sums al
# reempaquetar, y un DEB con md5sums viejos falla `dpkg --verify` tras
# instalar. Se rehace DEBIAN/md5sums desde el arbol final (sin entradas de
# DEBIAN/) y se reempaqueta con --root-owner-group: sin el, dpkg-deb meteria
# el uid/gid del builder y el paquete dependeria de quien lo construyo.
#
# Uso: package_deb.sh <fichero.deb> <directorio-runtime bundle/python>
#
# Ejecutado por build-linux.yml como llamada fina y probado contra fixtures
# con dpkg-deb falso por PATH (scripts/tests/test_ci_package_deb.py): sin
# red, sin dpkg real.

set -euo pipefail

usage() {
  echo "uso: $0 <fichero.deb> <directorio-runtime bundle/python>" >&2
  exit 2
}

[[ $# -eq 2 ]] || usage
DEB=$1
RUNTIME=$2

if [[ ! -f $DEB ]]; then
  echo "::error::no existe el DEB a inyectar: $DEB"
  exit 1
fi
if [[ ! -d $RUNTIME ]]; then
  echo "::error::no existe el directorio del runtime montado: $RUNTIME"
  exit 1
fi
# La forma del arbol montado es contrato (RUNTIME_INTERPRETER del proveedor):
# sin el interprete no hay paquete que valga y mejor morir aqui con
# diagnostico que publicar un DEB roto.
if [[ ! -e $RUNTIME/bin/python3 ]]; then
  echo "::error::el runtime no trae bin/python3 (layout de install-runtime): $RUNTIME"
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# -R (--raw-extract): arbol desplegable con DEBIAN/ dentro, editable.
dpkg-deb -R "$DEB" "$WORK/pkg"

if [[ ! -d $WORK/pkg/DEBIAN ]]; then
  echo "::error::el DEB desempaquetado no trae DEBIAN/: no es un paquete Debian usable"
  exit 1
fi

# Localizar scripts/ por el marcador: dependency_manager.py es parte
# contractual de los recursos (viaja siempre), asi que su ruta delata donde
# Tauri aplanó scripts/. Nada de hardcodear usr/lib/<producto>: el nombre del
# producto cambia con la version. Se comprueba el MARCADOR y no su dirname:
# con find vacio, dirname "" devuelve "." y la copia se iria fuera del arbol
# del paquete (la guarda heredada del inline solo rechazaba "" y "/").
MARKER="$(find "$WORK/pkg" -path '*_up_/scripts/dependency_manager.py' | head -1)"
if [[ -z $MARKER || ! -f $MARKER ]]; then
  echo "::error::el DEB no contiene _up_/scripts (marcador dependency_manager.py); no se puede inyectar Python"
  exit 1
fi
UP_SCRIPTS="$(dirname "$MARKER")"

# python/ hermano de scripts/: el candidate que el resolutor acepta. Si ya
# existiera (DEB reinyectado), cp -a anidaria el arbol DENTRO (python/python)
# y el paquete saldria roto sin ruido: mejor rechazarlo aqui.
if [[ -e $UP_SCRIPTS/../python ]]; then
  echo "::error::el DEB ya trae _up_/python: no se inyecta dos veces"
  exit 1
fi
cp -a "$RUNTIME" "$UP_SCRIPTS/../python"

# Regenerar md5sums: solo ficheros de datos (DEBIAN/ queda fuera; md5sums
# lleva rutas relativas sin ./, el formato que dpkg espera).
(
  cd "$WORK/pkg"
  find . -type f ! -path './DEBIAN/*' -printf '%P\0' | sort -z \
    | xargs -0 -I{} md5sum "{}" > DEBIAN/md5sums
)

# Reempaquetado reproducible: propietario root:root independiente del builder.
dpkg-deb --root-owner-group -b "$WORK/pkg" "$DEB"

echo "Python inyectado en $DEB ($(find "$UP_SCRIPTS/../python" | wc -l) entradas)"
