#!/usr/bin/env bash
# install_native_deps.sh — fuente única de las dependencias nativas de Linux.
#
# Consumidores:
#   - .github/workflows/ci.yml         (job Rust, pata Linux)
#   - .github/workflows/build-linux.yml (AppImage y DEB)
# La caché de /var/cache/apt/archives de ambos se invalida con el hash de ESTE
# fichero: editar la lista refresca los .deb; el resto del workflow no la tira.
#
# La lista es la unión exacta de las dos que vivían en los workflows: los 15
# paquetes del build de release y los 7 del job Rust. Seis de los siete ya
# estaban entre los 15; `flac` solo estaba en CI → 16 únicos.
#
# Los reintentos viajan en cada invocación (Acquire::Retries=5): el incidente
# del 2026-11-07 (corrida 42216381025) murió descargando .deb de un mirror
# lento, y la defensa no debe depender de un apt.conf.d que solo prepara uno
# de los consumidores.
set -euo pipefail

readonly APT_RETRIES=5
readonly NATIVE_PACKAGES=(
  build-essential
  curl
  ffmpeg
  file
  flac
  libasound2-dev
  libayatana-appindicator3-dev
  libchromaprint-tools
  libgtk-3-dev
  librsvg2-dev
  libssl-dev
  libwebkit2gtk-4.1-dev
  python3-pip
  python3-venv
  wget
  xvfb
)

apt_get() {
  sudo apt-get -o "Acquire::Retries=${APT_RETRIES}" "$@"
}

apt_get update
apt_get install -y "${NATIVE_PACKAGES[@]}"
