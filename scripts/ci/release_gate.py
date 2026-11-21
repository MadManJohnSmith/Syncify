#!/usr/bin/env python3
"""Puerta de publicación: el único lugar donde se decide si un tag puede existir.

Existe para cerrar un fallo real de proceso: los workflows de build eran
independientes entre sí y sin gating, así que cualquiera podía pulsar
`create_release` y crear el tag/publicar un release sin que existiera CI verde
para ese commit. Esta puerta hace esa comprobación de forma explícita y falla
de forma ruidosa en lugar de dejar que action-gh-release cree el tag.

Los jobs `gate` de build-linux.yml y build-windows.yml invocan este script en
un único paso (`python3 scripts/ci/release_gate.py ...`), y también se puede
ejecutar en local con `gh` autenticado: probar un cambio de la puerta cuesta
0 minutos de runner. Es un paso `run:` normal, no un reusable workflow:
invocar un reusable desde un workflow_dispatch no arranca (el run moría en
startup_failure antes del primer paso; comprobado sustituyendo la puerta por
un reusable mínimo de un solo `run: echo ok` y viendo que el fallo se
repetía).

Tres reglas, evaluadas en este orden (la primera que falla detiene la puerta):

  1. El tag tiene forma vX.Y.Z.
  2. El SHA que se va a publicar tiene una corrida de CI *exitosa*.
     Solo cuentan corridas completadas: una en curso no da por bueno un
     workflow que se lanzó pero todavía no terminó.
  3. El tag no existe todavía, o existe apuntando EXACTAMENTE al mismo SHA.
     Nunca se mueve un tag existente, ni se borra. Para tags anotados se
     consulta la línea peeled (`refs/tags/TAG^{}`): un tag anotado en el
     mismo SHA se permite, en otro SHA se rechaza.

CLI:

    python3 scripts/ci/release_gate.py --tag v0.2.1 --sha <commit> [--repo owner/name]

`--repo` por defecto sale de `GITHUB_REPOSITORY` (definido en cualquier job de
Actions). El token llega por el entorno (`GH_TOKEN`; en Actions el paso lo
fija a `github.token`). Exit 0 aprueba; exit 1 rechaza emitiendo mensajes
`::error::` con el diagnóstico completo: la pista de paths-ignore para
commits solo-de-docs y el comando `gh workflow run` que relanza CI bajo
demanda. Cualquier otro código de salida es un error de uso, no un veredicto.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys

TAG_PATTERN = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+$")
CI_WORKFLOW = "ci.yml"


def emit_error(message: str) -> None:
    """Emit an annotation visible as an error in the Actions run log."""
    print(f"::error::{message}", flush=True)


def _run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True)


def check_tag_format(tag: str) -> bool:
    """Rule 1: the tag must have the vX.Y.Z form."""
    if not TAG_PATTERN.match(tag):
        emit_error(f"El tag '{tag}' no tiene la forma vX.Y.Z. No se publica nada.")
        return False
    print(f"Formato del tag correcto: {tag}", flush=True)
    return True


def _ci_conclusions(repo: str, sha: str) -> list[str] | None:
    """Completed CI run conclusions for the exact SHA, or None if gh itself failed."""
    proc = _run([
        "gh", "run", "list",
        "--repo", repo,
        "--workflow", CI_WORKFLOW,
        "--commit", sha,
        "--status", "completed",
        "--json", "conclusion",
        "--jq", ".[].conclusion",
    ])
    if proc.returncode != 0:
        emit_error(
            f"gh no pudo listar las corridas de CI de {sha} en {repo} "
            f"(salida {proc.returncode}): {proc.stderr.strip()}"
        )
        return None
    return [line.strip() for line in proc.stdout.splitlines() if line.strip()]


def _current_ref(fallback: str) -> str:
    """Branch to suggest when relaunching CI; the SHA itself if there is no clone."""
    proc = _run(["git", "rev-parse", "--abbrev-ref", "HEAD"])
    if proc.returncode == 0:
        ref = proc.stdout.strip()
        if ref and ref != "HEAD":
            return ref
    return fallback


def check_ci_green(repo: str, sha: str) -> bool:
    """Rule 2: the exact SHA has a successful (not merely running) CI run."""
    print(f"Verificando CI para el commit {sha}", flush=True)
    conclusions = _ci_conclusions(repo, sha)
    if conclusions is None:
        return False

    # Se filtran las corridas fallidas/incompletas para no dar por bueno un
    # workflow que se lanzo pero todavia no termino.
    if not conclusions:
        emit_error(f"No hay ninguna corrida de CI completada para {sha}.")
        emit_error("Si el commit solo toca .md o docs/, CI no se dispara por paths-ignore.")
        emit_error("Lanzala bajo demanda antes de publicar:")
        emit_error(f"  gh workflow run {CI_WORKFLOW} --ref {_current_ref(sha)}")
        return False

    if "success" not in conclusions:
        emit_error(f"La CI de {sha} no esta en verde. Se obtuvo:")
        for conclusion in conclusions:
            print(conclusion, flush=True)
        return False

    print(f"CI en verde para {sha}", flush=True)
    return True


def _ls_remote(repo: str, pattern: str) -> list[str] | None:
    """First field of every `git ls-remote` line for the pattern, or None on failure.

    None replica la semántica actual del gate (`|| true` en el shell): un
    fallo del ls-remote no bloquea la publicación, pero se avisa en stderr
    para que no pase en silencio.
    """
    proc = _run(["git", "ls-remote", "--tags", f"https://github.com/{repo}", pattern])
    if proc.returncode != 0:
        print(
            f"aviso: git ls-remote fallo para {pattern} en {repo}: {proc.stderr.strip()}",
            file=sys.stderr,
            flush=True,
        )
        return None
    return [line.split()[0] for line in proc.stdout.splitlines() if line.split()]


def check_tag_not_moved(repo: str, tag: str, sha: str) -> bool:
    """Rule 3: the tag does not exist yet, or points exactly to this SHA."""
    remote_shas = _ls_remote(repo, f"refs/tags/{tag}")
    if not remote_shas:
        print(f"El tag {tag} no existe todavia. Se creara apuntando a {sha}.", flush=True)
        return True

    # peeled: para tags anotados hay una segunda linea refs/tags/TAG^{}
    peeled_shas = _ls_remote(repo, f"refs/tags/{tag}^{{}}") or []
    resolved = peeled_shas[0] if peeled_shas else remote_shas[0]

    if resolved != sha:
        emit_error(f"El tag {tag} ya existe y apunta a {resolved}, no a {sha}.")
        emit_error("No se mueve un tag existente. Publica una version nueva.")
        return False

    print(f"El tag {tag} ya existe y apunta al mismo commit {sha}.", flush=True)
    return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Puerta de publicación: formato vX.Y.Z -> CI verde para el SHA exacto "
            "-> tag no movible. Exit 0 aprueba, exit 1 rechaza con ::error::."
        ),
    )
    parser.add_argument("--tag", required=True, help="Tag candidato (debe ser vX.Y.Z)")
    parser.add_argument("--sha", required=True, help="Commit exacto que se va a publicar")
    parser.add_argument(
        "--repo",
        default=os.environ.get("GITHUB_REPOSITORY"),
        help="owner/name (por defecto, GITHUB_REPOSITORY)",
    )
    args = parser.parse_args(argv)
    if not args.repo:
        parser.error("--repo no indicado y GITHUB_REPOSITORY no esta definido")

    approved = (
        check_tag_format(args.tag)
        and check_ci_green(args.repo, args.sha)
        and check_tag_not_moved(args.repo, args.tag, args.sha)
    )
    if approved:
        print(
            f"Puerta de publicación aprobada: commit {args.sha}, tag {args.tag}.",
            flush=True,
        )
    return 0 if approved else 1


if __name__ == "__main__":
    sys.exit(main())
