#!/usr/bin/env bash
# Doctor de entorno: comprueba los prerrequisitos locales de las puertas del
# repo y avisa cuales quedaran solo para CI. Informativo: siempre sale 0.
#
# Motivo (retro 2026-11-11): webkit2gtk ausente costo un ciclo de CI de
# 33 min por un warning que el clippy local habria cazado, y el mismo
# diagnostico (venv, cargo, node_modules, OpenSSL) se repitio tres veces en
# hilos distintos de la misma semana.
set -u

fail=0
ok()   { printf '  ok     %s\n' "$1"; }
warn() { printf '  FALTA  %s\n' "$1"; fail=1; }

# --- Puertas Rust -----------------------------------------------------------
if command -v cargo >/dev/null 2>&1; then
  ok "cargo en PATH ($(cargo --version 2>/dev/null))"
else
  warn "cargo no esta en PATH: fmt, check, clippy y tests corren solo en CI"
fi

if pkg-config --exists webkit2gtk-4.1 2>/dev/null && pkg-config --exists javascriptcoregtk-4.1 2>/dev/null; then
  ok "webkit2gtk-4.1 y javascriptcoregtk-4.1: check/clippy locales posibles"
else
  warn "webkit2gtk-4.1 o javascriptcoregtk-4.1: cargo check/clippy locales no compilan la app (instala los paquetes -devel de tu distro)"
fi

openssl_version=$(pkg-config --modversion openssl 2>/dev/null || true)
if [ -z "$openssl_version" ]; then
  warn "openssl no responde a pkg-config: openssl-sys no compilara"
elif [ "${openssl_version%%.*}" = "4" ]; then
  warn "pkg-config resuelve openssl $openssl_version, que openssl-sys rechaza: exporta PKG_CONFIG_PATH=/usr/lib/openssl-3/pkgconfig antes de pushear"
else
  ok "openssl $openssl_version via pkg-config"
fi

# --- Suite Python -----------------------------------------------------------
if [ -d .venv ]; then
  ok ".venv presente (suite: .venv/bin/python -m unittest discover -s scripts/tests)"
else
  warn "falta .venv: la suite Python pierde modulos con el python3 del sistema (405 de 428, 4 import errors)"
fi

# --- UI y shell Tauri -------------------------------------------------------
if [ -d node_modules ]; then
  ok "node_modules de la raiz (CLI tauri disponible)"
else
  warn "falta node_modules en la raiz: tauri dev falla con \"env: 'tauri': No such file or directory\" (npm ci)"
fi
if [ -d ui/node_modules ]; then
  ok "ui/node_modules presente"
else
  warn "falta ui/node_modules: las puertas de la UI no arrancan (npm --prefix ui ci)"
fi

printf '\n'
if [ "$fail" -eq 0 ]; then
  echo "Entorno completo: todas las puertas pueden correr en local."
else
  echo "Entorno incompleto: lo marcado FALTA corre solo en CI. Instala lo que uses antes de pushear; el resto lo cubre CI."
fi
exit 0
