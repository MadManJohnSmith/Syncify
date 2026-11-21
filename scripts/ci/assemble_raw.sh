#!/usr/bin/env bash
# Ensamblado del tarball crudo Linux (#244): dist/linux/Syncify-Linux-Raw.
#
# El tarball crudo es la opcion sin instalador: binario + bin/ (ffmpeg,
# ffprobe, fpcalc colocados por el proveedor con triple) + scripts/ + el
# Python autonomo de bundle/python. get_project_root encuentra Syncify-Linux-Raw
# (trae scripts/) y get_python_executable resuelve <raiz>/python/bin/python3.
#
# Las exclusiones de seguridad (TASK-95 / SEC-011) NO viven aqui: se consumen
# de scripts/ci/exclusions.txt, fuente unica compartida con el portable de
# Windows (#247). rsync ignora ahi comentarios y lineas vacias. Desde que la
# fuente es comun, Linux gana *.pyc (SEC-011 detecto que su lista inline no
# lo tenia).
#
# Rutas relativas a la raiz del repo (el build corre ahi, igual que el inline
# que sustituye). Sin flags: el contrato es el arbol del repo.
#
# Uso: assemble_raw.sh
#
# Probado contra raices de repo sinteticas (scripts/tests/test_ci_assemble_raw.py):
# arbol de scripts con basura -> salida limpia, tarball sin basura.

set -euo pipefail

BINARY=target/release/syncify-tauri
EXTERNALS=src-tauri/binaries
TRIPLE=x86_64-unknown-linux-gnu
SCRIPTS_DIR=scripts
RUNTIME=bundle/python
EXCLUSIONS=scripts/ci/exclusions.txt
OUT=dist/linux
NAME=Syncify-Linux-Raw
TARBALL=Syncify-Linux-x86_64.tar.gz

fail() {
  echo "::error::$1"
  exit 1
}

# Validar todas las entradas ANTES de crear el arbol de salida: un fallo
# temprano no deja un Raw a medias que un paso posterior podria empaquetar.
[[ -f $BINARY ]] || fail "no existe el binario compilado: $BINARY (¿corrio tauri build?)"
[[ -d $SCRIPTS_DIR ]] || fail "no existe el directorio de scripts: $SCRIPTS_DIR"
[[ -f $EXCLUSIONS ]] || fail "no existe la fuente unica de exclusiones: $EXCLUSIONS"
[[ -d $RUNTIME ]] || fail "no existe el runtime montado: $RUNTIME (¿corrio dependency_manager.py install-runtime?)"
[[ -e $RUNTIME/bin/python3 ]] || fail "el runtime no trae bin/python3 (layout de install-runtime): $RUNTIME"

for tool in ffmpeg ffprobe fpcalc; do
  [[ -f $EXTERNALS/$tool-$TRIPLE ]] || fail "falta el binario externo $tool-$TRIPLE en $EXTERNALS (¿corrio dependency_manager.py place-external?)"
done

RAW=$OUT/$NAME
rm -rf "$RAW"
mkdir -p "$RAW/bin" "$RAW/scripts"

# Binario crudo junto a bin/: el resolutor busca ffmpeg/ffprobe/fpcalc en
# bin/ hermano del ejecutable cuando no hay bundle externo.
install -m 755 "$BINARY" "$RAW/syncify"

# Los externos viajan con nombre pelado: en el tarball crudo no hay triple
# que resolver, solo ficheros que el usuario puede listar.
for tool in ffmpeg ffprobe fpcalc; do
  install -m 755 "$EXTERNALS/$tool-$TRIPLE" "$RAW/bin/$tool"
done

# scripts/ con la fuente unica de exclusiones: rsync ignora en el fichero
# comentarios y lineas vacias, y cada glob casa contra el nombre en
# cualquier nivel (credentials* corta en services/ tambien).
rsync -a --exclude-from="$EXCLUSIONS" "$SCRIPTS_DIR/" "$RAW/scripts/"

# Python autonomo junto a scripts/: hermano del scripts/ que get_project_root
# usa de marcador. cp -a conserva symlinks (bin/python -> python3).
cp -a "$RUNTIME" "$RAW/python"

(
  cd "$OUT"
  tar -czf "$TARBALL" "$NAME/"
)

echo "Tarball crudo generado en $OUT/$TARBALL"
du -sh "$RAW" "$OUT/$TARBALL"
