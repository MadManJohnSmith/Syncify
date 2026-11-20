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

# --- Target de Rust ---------------------------------------------------------
# Un target por checkout fuera de tmpfs (retro 2026-11-19, R3): /tmp es un
# tmpfs de 12 GB que se llena y un target compartido mezcla fingerprints de
# otros worktrees. Es informativo: el default de los scripts ya lo respeta.
resolved_target="${CARGO_TARGET_DIR:-$HOME/.cache/syncify-target-$(basename "$PWD")}"
probe="$resolved_target"
while [ ! -e "$probe" ] && [ "$probe" != "/" ]; do
  probe="$(dirname "$probe")"
done
target_fstype="$(stat -f -c %T "$probe" 2>/dev/null || true)"
if [ "$target_fstype" = "tmpfs" ]; then
  warn "CARGO_TARGET_DIR=$resolved_target cae en tmpfs: un build grande puede morir con ENOSPC (usa \$HOME/.cache/syncify-target-\$(basename \"\$PWD\"))"
else
  ok "target de Rust en $resolved_target ($target_fstype)"
fi

shared_lane=""
while IFS= read -r worktree_path; do
  [ -n "$worktree_path" ] || continue
  [ "$worktree_path" = "$(pwd -P)" ] && continue
  other_default="$HOME/.cache/syncify-target-$(basename "$worktree_path")"
  if [ "$other_default" = "$resolved_target" ]; then
    shared_lane="$worktree_path"
  fi
done < <(git worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p')
if [ -n "$shared_lane" ]; then
  warn "otro worktree ($shared_lane) resuelve al mismo target: exporta un CARGO_TARGET_DIR distinto por carril"
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
