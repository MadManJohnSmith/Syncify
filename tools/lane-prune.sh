#!/usr/bin/env bash
# lane-prune.sh — lista o elimina worktrees de carril ya fusionados.
#
#   tools/lane-prune.sh            dry-run: qué se podaría y con qué target
#   tools/lane-prune.sh --apply    elimina los worktrees y sus targets
#
# Solo entran worktrees cuyo HEAD ya es ancestro de origin/dev, que no son el
# checkout principal ni el directorio actual y que están limpios (se toleran
# los symlinks de entorno `.venv`, `node_modules` y `ui/node_modules`). La poda
# se niega con un `cargo` vivo: sus binarios y el lock del target no son
# basura. Los worktrees con cambios sin commitear se conservan y se listan.
#
# No borra ramas. Haz `git fetch origin dev` antes si el ref local está viejo.
set -euo pipefail

usage() {
    sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

apply=0
for arg in "$@"; do
    case "$arg" in
        --apply) apply=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Uso: tools/lane-prune.sh [--apply]" >&2; exit 2 ;;
    esac
done

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

git rev-parse --verify --quiet origin/dev >/dev/null \
    || { echo "No existe origin/dev: ejecuta git fetch origin dev." >&2; exit 1; }

if pgrep -x cargo >/dev/null 2>&1; then
    echo "Hay un cargo vivo: no se poda nada." >&2
    pgrep -a cargo >&2 || true
    exit 1
fi

main_worktree="$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -n 1)"
main_worktree="$(cd "$main_worktree" && pwd -P)"
current="$(pwd -P)"

prune_paths=()
prune_targets=()
dirty_paths=()
unmerged=0

target_for() {
    local path="$1" branch="$2" base issue candidate
    base="$HOME/.cache/syncify-target-$(basename "$path")"
    if [ -d "$base" ]; then
        printf '%s\n' "$base"
        return 0
    fi
    branch="${branch#refs/heads/}"
    issue="$(printf '%s' "$branch" | grep -oE '[0-9]{1,4}' | head -n 1 || true)"
    if [ -n "$issue" ]; then
        for candidate in "$HOME/.cache/syncify-target-$issue" "$HOME/.cache/syncify-target-$issue"*; do
            if [ -d "$candidate" ]; then
                printf '%s\n' "$candidate"
                return 0
            fi
        done
    fi
    return 0
}

has_unexpected_changes() {
    local path="$1" line
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        case "$line" in
            "?? .venv"|"?? node_modules"|"?? ui/node_modules") ;;
            *) return 0 ;;
        esac
    done < <(git -C "$path" status --porcelain 2>/dev/null || true)
    return 1
}

while IFS=$'\t' read -r path head branch; do
    [ -n "$path" ] || continue
    path="$(cd "$path" && pwd -P)"
    if [ "$path" = "$main_worktree" ] || [ "$path" = "$current" ]; then
        continue
    fi
    if ! git merge-base --is-ancestor "$head" origin/dev 2>/dev/null; then
        unmerged=$((unmerged + 1))
        continue
    fi
    if has_unexpected_changes "$path"; then
        dirty_paths+=("$path")
        continue
    fi
    prune_paths+=("$path")
    prune_targets+=("$(target_for "$path" "$branch")")
done < <(
    git worktree list --porcelain | awk '
        /^worktree / { path = substr($0, 10) }
        /^HEAD /     { head = substr($0, 6) }
        /^branch /   { branch = substr($0, 8) }
        /^$/         { if (path != "") print path "\t" head "\t" branch; path = ""; head = ""; branch = "" }
        END          { if (path != "") print path "\t" head "\t" branch }
    '
)

echo "Fusionados en origin/dev y limpios: ${#prune_paths[@]}"
for index in "${!prune_paths[@]}"; do
    target="${prune_targets[$index]:-}"
    printf '  %s\n' "${prune_paths[$index]}"
    if [ -n "$target" ]; then
        printf '    target: %s\n' "$target"
    else
        printf '    target: sin mapear\n'
    fi
done

if [ "${#dirty_paths[@]}" -gt 0 ]; then
    echo
    echo "Conservados con cambios sin commitear: ${#dirty_paths[@]}"
    printf '  %s\n' "${dirty_paths[@]}"
fi

echo
echo "Sin fusionar (se conservan): $unmerged"

if [ "$apply" -eq 0 ]; then
    echo "Dry-run: vuelve a ejecutar con --apply para eliminar los fusionados."
    exit 0
fi

if [ "${#prune_paths[@]}" -eq 0 ]; then
    echo "No hay nada que podar."
    exit 0
fi

if pgrep -x cargo >/dev/null 2>&1; then
    echo "Apareció un cargo vivo: no se poda nada." >&2
    pgrep -a cargo >&2 || true
    exit 1
fi

for index in "${!prune_paths[@]}"; do
    path="${prune_paths[$index]}"
    target="${prune_targets[$index]:-}"
    [ "$path" != "$main_worktree" ] || continue
    if has_unexpected_changes "$path"; then
        echo "Conservado (aparecieron cambios): $path" >&2
        continue
    fi
    echo "==> git worktree remove $path"
    git worktree remove --force "$path"
    if [ -n "$target" ] && [ -d "$target" ]; then
        case "$target" in
            "$HOME"/.cache/syncify-target*)
                echo "==> rm -rf $target"
                rm -rf -- "$target"
                ;;
            *)
                echo "AVISO: no borro un target fuera de ~/.cache/syncify-target*: $target" >&2
                ;;
        esac
    fi
done

git worktree prune
echo "Poda terminada."
