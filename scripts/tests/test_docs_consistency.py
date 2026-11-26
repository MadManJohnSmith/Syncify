#!/usr/bin/env python3
"""Documentation consistency gates for the repaired findings.

These checks exist so the repairs do not rot back into the defects the audit
found, and they are hermetic: no network, no repository writes, only the tree.

Covered:

* SYNC-AUD-063 — the CI header must not describe a debt the workflow already closed.
* SYNC-AUD-064 — the README must point contributions at the branch CI runs on,
  and never at a branch that has been deleted upstream.
* SYNC-AUD-065 — the README must not send anyone to `workspace/`, which `.gitignore`
  excludes and no checkout or CI ever contains.
* SYNC-AUD-067 — the debt register the code and the plan cite must exist in the tree.
* SYNC-AUD-068 — the parity matrix must not republish a stale suite figure.
* SYNC-AUD-070 — every file path the design docs cite must resolve, so "revalidated"
  is a checkable statement.
* SYNC-AUD-072 — the gate table must name, for every row, whether the figure was
  measured on the revision it claims and which command produces it.
* SYNC-AUD-077 — the conditioned scope (D-01 to D-04) must be declared as such
  instead of being read as pending work.
* #393 — the Rust job's commands are read from its parsed `run:` steps, so a
  `--skip` inside a `|`/`>` block and a Windows prebuild without `--workspace`
  both fail the gate.
* #366 — `docs/HANDOFF.md` is a stable pointer, not a frozen relay; the two
  wiki pages that rule on pull requests name the same batch branch; and a
  `docs/COLA_*.md` whose state is closed lives in `docs/historico/`.
"""

import re
import sys
import unittest
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# Only tokens whose last segment carries a known source extension are treated as
# file references: that skips API endpoints (`/me/library/songs`) and enum-ish
# values (`success/partial_failure/failed`), which are not paths in this tree.
KNOWN_EXTENSIONS = (
    ".rs", ".ts", ".vue", ".py", ".sql", ".json", ".md", ".yml",
    ".yaml", ".sh", ".toml", ".html", ".css", ".lock",
)
BACKTICKED = re.compile(r"`([^`\n]+)`")

# Roots a bare filename such as `models.rs` may live in.
BASENAME_ROOTS = (
    "src-tauri/src",
    "src-tauri",
    "src-tauri/migrations",
    "ui/src",
    "scripts",
    "scripts/services",
    "crates",
)


def _is_file_reference(token: str) -> bool:
    token = token.strip()
    if not token or " " in token or token.startswith(("http", "--", "$", "{{")):
        return False
    if "*" in token or token.startswith("/"):
        return False
    first_segment = token.split("/", 1)[0]
    if "." in first_segment and not first_segment.startswith("."):
        return False  # a host, e.g. `tekstowo.pl/szukaj,...`
    return token.rsplit("/", 1)[-1].endswith(KNOWN_EXTENSIONS)


def _expand_braces(token: str):
    """`src/{a,b}.rs` names several files: every one of them must resolve."""
    if "{" not in token or "}" not in token:
        return [token]
    head, _, rest = token.partition("{")
    options, _, tail = rest.partition("}")
    return [f"{head}{option}{tail}" for option in options.split(",")]


# Paths the documentation is allowed to cite even though they are not in the
# tree, with the reason. The test also asserts each one is *still* absent, so an
# entry that stops being true is reported instead of rotting in the allowlist.
EXPECTED_ABSENT = {
    ".gui_credentials_cache.json": "runtime credential cache, ignored by .gitignore",
    "scripts/.gui_credentials_cache.json": "runtime credential cache, ignored by .gitignore",
    "downloader.rs": "QBDLX module deleted on 2026-08-25; the doc marks it '(eliminado)'",
    "src-tauri/src/downloader.rs": "QBDLX module deleted on 2026-08-25; the doc marks it '(eliminado)'",
    "resources/qbdlx-mod/": "vendor bundle removed together with the module",
    "scripts/services/spotify_auth.py": "Spotify auth moved to the Rust core; cited as removed",
    "useQueue.ts": "composable purged as dead code in 417a0eb; cited as removed",
}


def _resolve(token: str, relative_to: Path = None):
    """Return the first candidate path that exists, or None."""
    direct = [
        token,
        f"src-tauri/{token}",
        f"src-tauri/src/{token}",
        f"src-tauri/migrations/{token}",
        f"src-tauri/resources/{token}",
        f"ui/{token}",
        f"ui/src/{token}",
        f"scripts/{token}",
    ]
    if relative_to is not None:
        direct.insert(0, str((relative_to.parent / token).resolve()))
    for candidate in direct:
        if (REPO_ROOT / candidate).exists():
            return candidate
    if "/" in token:
        for crate_src in sorted(REPO_ROOT.glob("crates/*/src")):
            if (crate_src / token).exists():
                return str(crate_src / token)
        return None
    for root in BASENAME_ROOTS:
        for match in (REPO_ROOT / root).glob("**/" + token):
            return str(match.relative_to(REPO_ROOT))
    return None


def _doc_text(relative: str) -> str:
    return (REPO_ROOT / relative).read_text(encoding="utf-8")


#: Comandos del job Rust que la puerta fija: nombre del paso -> `run` exacto.
#: Un `run: |` o `run: >` llega aquí como el texto que YAML despliega, así que
#: el bloque serializado de batch_50 y cualquier bloque futuro se comparan
#: como el comando que ejecuta el runner, no como una línea que empieza por
#: `run:`.
RUST_REQUIRED_RUNS = {
    "cargo check --workspace --all-targets --locked": (
        "cargo check --workspace --all-targets --locked"
    ),
    "Compilar binarios de test sin ejecutar (base del manifiesto comctl32)": (
        "cargo test --workspace --locked --no-run"
    ),
    "Suite de tests completa": "cargo test --workspace --locked --no-fail-fast",
}

#: `--skip` como token de cargo, no como fragmento de otra bandera o de un texto.
SKIP_TOKEN = re.compile(r"(?:^|\s)--skip(?:=|\s|$)")


def _rust_run_steps(workflow_text: str) -> dict[str, str]:
    """Los `run` del job Rust, ya desplegados por YAML.

    Leer el YAML y no las líneas es lo que ve un `run: |`: sus comandos no
    empiezan por `run:` ni viven en la misma línea que la clave.
    """
    workflow = yaml.safe_load(workflow_text)
    job = (workflow.get("jobs") or {}).get("rust") or {}
    runs = {}
    for index, step in enumerate(job.get("steps") or []):
        if not isinstance(step, dict) or not isinstance(step.get("run"), str):
            continue
        runs[step.get("name") or f"steps[{index}]"] = step["run"]
    return runs


def _skipped_rust_steps(workflow_text: str) -> dict[str, str]:
    """Pasos del job Rust cuyo `run` lleva `--skip`, con el texto que lo lleva."""
    return {
        name: run
        for name, run in _rust_run_steps(workflow_text).items()
        if SKIP_TOKEN.search(run)
    }


def _missing_required_rust_runs(workflow_text: str) -> dict[str, tuple[str, str | None]]:
    """Pasos del job Rust que no ejecutan el comando canónico que la puerta fija."""
    runs = _rust_run_steps(workflow_text)
    return {
        name: (expected, runs.get(name))
        for name, expected in RUST_REQUIRED_RUNS.items()
        if runs.get(name) != expected
    }


class DocumentationReferenceTests(unittest.TestCase):
    """SYNC-AUD-070: the design reference set must stay resolvable."""

    def _documents(self):
        docs = sorted((REPO_ROOT / "docs" / "features").glob("*.md"))
        docs += sorted((REPO_ROOT / "docs" / "adr").glob("*.md"))
        docs += sorted((REPO_ROOT / "docs" / "agents").rglob("*.md"))
        docs += sorted(p for p in (REPO_ROOT / "docs").glob("*.md"))
        # AGENTS.md es el mapa que un agente lee primero: sus referencias
        # envejecen igual que las de docs/ y deben resolverse igual.
        for root_doc in ("AGENTS.md", "GLOSSARY.md"):
            candidate = REPO_ROOT / root_doc
            if candidate.is_file():
                docs.append(candidate)
        return docs

    def test_every_cited_path_in_docs_resolves(self):
        unresolved = {}
        checked = 0
        for doc in self._documents():
            missing = set()
            for token in BACKTICKED.findall(doc.read_text(encoding="utf-8")):
                if not _is_file_reference(token):
                    continue
                for candidate in _expand_braces(token.strip()):
                    checked += 1
                    if candidate in EXPECTED_ABSENT:
                        continue
                    if _resolve(candidate, doc) is None:
                        missing.add(candidate)
            if missing:
                unresolved[doc.relative_to(REPO_ROOT).as_posix()] = sorted(missing)
        self.assertTrue(checked > 0, "no documentation references were checked; the gate is inert")
        self.assertEqual(
            unresolved,
            {},
            "documentation cites paths that are not in the tree",
        )

    def test_every_adr_declares_its_status(self):
        """An ADR without a state is a note, not a decision record."""
        adrs = sorted((REPO_ROOT / "docs" / "adr").glob("[0-9][0-9][0-9][0-9]-*.md"))
        self.assertTrue(adrs, "no ADRs found; the convention is not established")
        for adr in adrs:
            text = adr.read_text(encoding="utf-8")
            self.assertIn(
                "**Estado:**",
                text,
                f"{adr.name} does not declare **Estado:**; see docs/agents/domain.md",
            )

    def test_allowlisted_absences_are_still_absent(self):
        """An allowlist entry that becomes real is stale and must be removed."""
        for path, reason in EXPECTED_ABSENT.items():
            self.assertFalse(
                (REPO_ROOT / path.rstrip("/")).exists(),
                f"{path} is allowlisted as absent ({reason}) but now exists: "
                "drop it from EXPECTED_ABSENT",
            )

    def test_design_docs_declare_their_revalidation_state(self):
        for doc in sorted((REPO_ROOT / "docs" / "features").glob("*.md")):
            text = doc.read_text(encoding="utf-8")
            self.assertIn(
                "**Estado de revisión:**",
                text,
                f"{doc.name} must declare its revalidation state",
            )


class DebtRegisterTests(unittest.TestCase):
    """SYNC-AUD-067: the register the code and the plan cite must exist."""

    def test_debt_register_is_in_the_tree(self):
        register = REPO_ROOT / "docs" / "Deuda_Tecnica_y_UX.md"
        self.assertTrue(register.is_file(), "docs/Deuda_Tecnica_y_UX.md is missing")

    def test_citations_of_the_register_resolve(self):
        for relative in ("docs/PLAN_UNIFICACION_IMPORTACION.md", "src-tauri/src/commands/service.rs"):
            text = _doc_text(relative)
            for token in BACKTICKED.findall(text):
                if "Deuda_Tecnica" in token:
                    self.assertIsNotNone(
                        _resolve(token.strip()),
                        f"{relative} cites {token!r}, which does not resolve",
                    )


class CiNarrativeTests(unittest.TestCase):
    """SYNC-AUD-063: the CI header must not contradict the steps it describes."""

    def setUp(self):
        self.ci = _doc_text(".github/workflows/ci.yml")

    def test_header_does_not_claim_batch_50_is_excluded(self):
        header = "\n".join(self.ci.splitlines()[:6])
        for stale_claim in ("Nota Deuda A12", "se cuelgan", "se re-ejecutan serializados"):
            self.assertNotIn(
                stale_claim,
                header,
                f"the CI header still carries the pre-b9559da claim {stale_claim!r}",
            )

    def _mutated_workflow(self, old: str, new: str) -> str:
        """El workflow real con un fragmento sustituido, para probar la puerta."""
        mutated = self.ci.replace(old, new, 1)
        self.assertNotEqual(mutated, self.ci, f"el fixture no encontró {old!r}")
        return mutated

    def test_no_rust_step_passes_skip_to_the_suite(self):
        """#393: el escaneo cubre todos los `run` parseados, no solo `run: cargo test`.

        El paso serializado de batch_50 ya es un bloque `|`; un `--skip` dentro
        de él no empieza por `run:` y antes pasaba inadvertido.
        """
        self.assertEqual(
            _skipped_rust_steps(self.ci),
            {},
            "ningún paso del job Rust puede saltarse tests con --skip",
        )

    def test_skip_gate_catches_a_skip_inside_a_multiline_run_block(self):
        """El fixture reescribe la suite como bloque `|`/`>` y la puerta lo ve."""
        for indicator in ("|", ">"):
            with self.subTest(indicator=indicator):
                mutated = self._mutated_workflow(
                    "      run: cargo test --workspace --locked --no-fail-fast\n",
                    f"      run: {indicator}\n"
                    "        cargo test --workspace --locked --no-fail-fast"
                    " -- --skip batch_50_audit_test\n",
                )
                skipped = _skipped_rust_steps(mutated)
                self.assertIn(
                    "Suite de tests completa",
                    skipped,
                    f"la puerta debe ver el --skip de un bloque run: {indicator}",
                )

    def test_skip_gate_catches_a_skip_inside_the_serialized_block(self):
        """El bloque ya multilínea de batch_50 tampoco puede llevar `--skip`."""
        mutated = self._mutated_workflow(
            "cargo test --locked --test batch_50_audit_test -- --test-threads=1",
            "cargo test --locked --test batch_50_audit_test -- --test-threads=1"
            " --skip some_other_test",
        )
        self.assertIn(
            "batch_50_audit_test serializado (--test-threads=1)",
            _skipped_rust_steps(mutated),
        )

    def test_rust_check_suite_and_windows_prebuild_use_the_canonical_command(self):
        """#251/#393: check, suite y prebuild de Windows cubren el workspace entero.

        El job corre desde `src-tauri`, un miembro del workspace virtual de la
        raíz: sin `--workspace` solo se compilaba ese paquete. El prebuild es el
        paso que prepara los `.exe` para el manifiesto comctl32; perder la
        bandera ahí deja los tests de crates/ sin precompilar.
        """
        self.assertEqual(
            _missing_required_rust_runs(self.ci),
            {},
            "el job Rust perdió un comando canónico (#251/#393)",
        )

    def test_prebuild_gate_catches_a_missing_workspace(self):
        """Quitar `--workspace` del prebuild Windows debe romper la puerta."""
        mutated = self._mutated_workflow(
            "run: cargo test --workspace --locked --no-run",
            "run: cargo test --locked --no-run",
        )
        violations = _missing_required_rust_runs(mutated)
        self.assertIn(
            "Compilar binarios de test sin ejecutar (base del manifiesto comctl32)",
            violations,
            "la puerta debe detectar que el prebuild dejó de cubrir el workspace",
        )
        self.assertNotIn(
            "Suite de tests completa",
            violations,
            "quitar --workspace del prebuild no debe culpar a la suite",
        )

    def test_serialized_batch_50_step_keeps_its_own_command(self):
        """El paso serializado no hereda `--workspace`: conserva comando y timeout propios."""
        batch = self.ci.split("batch_50_audit_test serializado", 1)[1].split("- name:", 1)[0]
        self.assertIn("cargo test --locked --test batch_50_audit_test -- --test-threads=1", batch)
        self.assertNotIn(
            "--workspace",
            batch,
            "el paso serializado de batch_50 conserva su comando propio",
        )

    def test_serialized_batch_50_step_has_its_own_timeout(self):
        self.assertIn("batch_50_audit_test serializado", self.ci)
        window = self.ci.split("batch_50_audit_test serializado", 1)[1].splitlines()[:4]
        self.assertTrue(
            any("timeout-minutes:" in line for line in window),
            "the isolated batch_50 step must keep its own time bound",
        )

    def test_rust_cache_caches_the_workspace_target(self):
        """#364: the virtual workspace builds into `./target`, not `src-tauri/target`.

        Measured on run 44947072166 (2026-11-21): `workspaces: src-tauri` made
        the action cache `src-tauri/target`, a directory this tree never
        creates, so each 149 MB entry held only `~/.cargo/registry` and every
        run compiled the dependencies from scratch. The manifest at the
        repository root is the virtual workspace, so the target to cache is
        `./target` (also the action's documented default).
        """
        step = self._rust_cache_step()
        workspaces = (step.get("with") or {}).get("workspaces")
        if workspaces is None or not str(workspaces).strip():
            return  # the documented default is already `. -> target`
        cached = []
        for entry in str(workspaces).splitlines():
            if not entry.strip():
                continue
            workspace, _, target = entry.partition("->")
            cached.append((Path(workspace.strip()).as_posix(), target.strip() or "target"))
        self.assertIn(
            (".", "target"),
            cached,
            "rust-cache must cache the workspace target at ./target: the "
            f"repository-root Cargo.toml is the virtual workspace, not "
            f"src-tauri; ci.yml declares {cached}",
        )

    def _rust_cache_step(self):
        workflow = yaml.safe_load(self.ci)
        for job in (workflow.get("jobs") or {}).values():
            for step in job.get("steps") or []:
                uses = step.get("uses") if isinstance(step, dict) else None
                if isinstance(uses, str) and uses.strip().startswith("Swatinem/rust-cache@"):
                    return step
        self.fail("ci.yml no usa Swatinem/rust-cache: la cache de cargo desaparecio")


class ReadmeTests(unittest.TestCase):
    """SYNC-AUD-064 and SYNC-AUD-065: the README must describe this tree."""

    # Branches the README must not send contributors to. `git ls-remote --heads
    # origin` on 2026-10-05 returned only main, syncify-graphical and the fix/*
    # topics: `syncify-app` is gone upstream, so the PR target it advertised
    # could not be opened at all, and `syncify-graphical` still exists but
    # stopped receiving work on 2026-09-10.
    NON_TARGET_BRANCHES = ("syncify-app", "syncify-graphical")

    def setUp(self):
        self.readme = _doc_text("README.md")
        self.readme_es = _doc_text("README.es.md")

    def _ci_push_branches(self):
        """Branches `.github/workflows/ci.yml` runs its jobs on."""
        ci = _doc_text(".github/workflows/ci.yml")
        window = ci.split("push:", 1)[1].split("branches:", 1)[1]
        window = window.split("paths-ignore:", 1)[0]
        return set(re.findall(r"^\s*-\s*(\S+)\s*$", window, re.MULTILINE))

    def test_pr_target_is_the_live_integration_branch(self):
        # Frozen on purpose: a CI checkout is shallow, so this gate cannot
        # resolve refs without network. It stays honest because the branch is
        # cross-checked against the one the CI workflow in the tree triggers on.
        live = self._ci_push_branches()
        self.assertTrue(live, "the push trigger no longer names any branch")
        for branch in live:
            for name, readme in (("README.md", self.readme), ("README.es.md", self.readme_es)):
                self.assertIn(
                    f"`{branch}`",
                    readme,
                    f"{name} must point pull requests at `{branch}`, the branch CI runs on",
                )
                for removed in self.NON_TARGET_BRANCHES:
                    self.assertNotIn(
                        f"`{removed}`",
                        readme,
                        f"{name} must not send contributors to `{removed}`: not an integration target",
                    )

    def test_no_pointer_to_the_gitignored_workspace_directory(self):
        for line in self.readme.splitlines():
            if "workspace/" in line:
                self.assertIn(
                    "no existe",
                    line,
                    f"README points at a gitignored path: {line!r}",
                )


class ContributingGuideTests(unittest.TestCase):
    """#393: el comando de suite es el mismo en CI, AGENTS.md y las dos secciones."""

    SUITE = RUST_REQUIRED_RUNS["Suite de tests completa"]

    def test_both_languages_publish_the_canonical_suite_command(self):
        english, _, spanish = _doc_text("CONTRIBUTING.md").partition("## Español")
        self.assertTrue(english and spanish, "CONTRIBUTING.md perdió una de sus dos secciones")
        self.assertIn(
            self.SUITE,
            _doc_text("AGENTS.md"),
            "AGENTS.md y CONTRIBUTING.md deben citar el mismo comando de suite",
        )
        for language, section in (("EN", english), ("ES", spanish)):
            with self.subTest(language=language):
                self.assertIn(
                    self.SUITE,
                    section,
                    f"la sección {language} debe citar el comando canónico de suite",
                )
                self.assertNotIn(
                    "`cargo test --workspace --locked`",
                    section,
                    f"la sección {language} conserva la forma corta sin --no-fail-fast",
                )


class ParityMatrixGateTests(unittest.TestCase):
    """SYNC-AUD-068: the matrix must not republish a superseded suite figure."""

    def test_stale_gate_figures_are_gone(self):
        matrix = _doc_text("docs/MATRIZ_PARIDAD_IMPORTACION.md")
        self.assertNotIn("1019+ passed", matrix)
        self.assertNotIn("vitest 283/283", matrix)

    def test_every_gate_row_names_its_command(self):
        matrix = _doc_text("docs/MATRIZ_PARIDAD_IMPORTACION.md")
        section = matrix.split("## Gates que avalan esta matriz", 1)[1]
        rows = [line for line in section.splitlines() if line.startswith("|")]
        self.assertGreaterEqual(len(rows), 4, "the gates table is missing or truncated")
        for row in rows[2:]:
            self.assertIn("`", row, f"gate row without a reproducible command: {row!r}")

    def test_every_gate_row_states_whether_it_was_measured(self):
        """SYNC-AUD-072: a figure must be attributable to a revision and to a run.

        A gate that was not measured has to say so in its own row instead of
        leaving the reader to assume the neighbouring figures cover it.
        """
        matrix = _doc_text("docs/MATRIZ_PARIDAD_IMPORTACION.md")
        section = matrix.split("## Gates que avalan esta matriz", 1)[1]
        header, *rows = [line for line in section.splitlines() if line.startswith("|")]
        self.assertIn("Revisión", header, "the gates table must carry a revision column")
        for row in rows[1:]:
            if set(row) <= set("|-: "):
                continue
            cells = [cell.strip().strip("`") for cell in row.strip("|").split("|")]
            self.assertEqual(
                len(cells), 4, f"gate row does not match the four-column table: {row!r}"
            )
            revision, outcome = cells[2], cells[3]
            if revision == "—":
                self.assertIn(
                    "sin medir",
                    outcome,
                    f"a row with no revision must declare itself unmeasured: {row!r}",
                )
            else:
                self.assertRegex(
                    revision, r"^[0-9a-f]{7,40}$", f"unattributable revision: {row!r}"
                )

    def test_gate_rows_publish_a_command_cargo_accepts(self):
        """cargo 1.98 rejects `cargo --manifest-path <path> test`."""
        matrix = _doc_text("docs/MATRIZ_PARIDAD_IMPORTACION.md")
        section = matrix.split("## Gates que avalan esta matriz", 1)[1]
        rows = [line for line in section.splitlines() if line.startswith("|")]
        for row in rows[2:]:
            if set(row) <= set("|-: "):
                continue
            command = row.strip("|").split("|")[1]
            self.assertNotIn(
                "cargo --manifest-path",
                command,
                f"gate row publishes a command form cargo rejects: {row!r}",
            )


class PlanStateTests(unittest.TestCase):
    """SYNC-AUD-069: the plan must state what actually executed."""

    def test_plan_status_is_not_a_stale_proposal(self):
        plan = _doc_text("docs/PLAN_UNIFICACION_IMPORTACION.md")
        self.assertNotIn("Estado: propuesta aprobada (pendiente de ejecución)", plan)
        self.assertIn("EN EJECUCIÓN", plan)


class AcceptedQobuzAppIdRiskTests(unittest.TestCase):
    """SYNC-AUD-071, revisado 2026-10-04: el bundle app_id/secret de Qobuz es
    PÚBLICO (lo usan los clientes open-source) y DEBE vivir incrustado tanto en
    los puentes Python como en el núcleo Rust. Lo que no debe regresar: que la
    decisión siga escrita en docs/Deuda_Tecnica_y_UX.md y que ambos lados
    compartan el MISMO valor público."""

    def _committed_public_app_id(self) -> str:
        """Read the accepted public app id from its single allowed home.

        The value is never spelled out here: `qobuz_credentials_leak_test` scans
        `scripts/` for it, and this gate must not become a second copy.
        """
        match = re.search(r'APP_ID\s*=\s*"(\d+)"', _doc_text("scripts/services/qobuz_service.py"))
        self.assertIsNotNone(match, "the public app id constant disappeared; update this gate")
        return match.group(1)

    def test_register_documents_the_accepted_exposure(self):
        register = _doc_text("docs/Deuda_Tecnica_y_UX.md")
        self.assertIn(
            self._committed_public_app_id(),
            register,
            "the accepted exposure must name the value it accepts",
        )
        self.assertIn("aceptada", register.lower(), "the entry must be labelled as accepted")

    def test_python_bridge_points_at_the_register(self):
        service = _doc_text("scripts/services/qobuz_service.py")
        self.assertIn(
            "Deuda_Tecnica_y_UX.md",
            service,
            "the constant must say that the exposure is a documented accepted risk",
        )

    def test_rust_core_embeds_the_same_public_app_id(self):
        core = _doc_text("src-tauri/src/services/qobuz.rs")
        self.assertIn(
            self._committed_public_app_id(),
            core,
            "the Rust core embeds the same public app_id as the Python bridges",
        )


class ConditionedScopeDeclarationTests(unittest.TestCase):
    """SYNC-AUD-077: the conditioned scope must be declared, not left implicit.

    The open entries are registered with their closing condition and none of them
    depends on work the team can do on its own: they are scope conditioned by an
    external input, not defects. A closure that lists them as pending work
    promises something the project cannot deliver, so the declaration is gated.

    The set of entries is read from the register's own "Deuda abierta" section
    instead of being spelled out here: closing an entry (D-03, in this revision)
    must not require editing this gate, and an entry added to the register must
    not be able to skip the declaration.
    """

    #: A row of the conditioned-scope table: its first column is the debt id.
    _CONDITIONED_ROW = re.compile(r"^\|\s*(D-\d+)\s*\|", re.MULTILINE)

    def setUp(self):
        self.register = _doc_text("docs/Deuda_Tecnica_y_UX.md")
        self.matrix = _doc_text("docs/MATRIZ_PARIDAD_IMPORTACION.md")

    def test_register_declares_the_conditioned_scope(self):
        self.assertIn(
            "Alcance condicionado",
            self.register,
            "the debt register must carry the conditioned-scope declaration",
        )
        self.assertIn(
            "no trabajo pendiente",
            self.register,
            "the declaration must say these entries are not pending work",
        )
        self.assertIn(
            "NO declara como pendiente",
            self.register,
            "the section heading must state what the register does not declare",
        )

    def _open_entries(self):
        """The debt ids the register currently lists as open."""
        open_section = self.register.split("## Deuda abierta", 1)[1].split(
            "## Alcance condicionado", 1
        )[0]
        return sorted(set(re.findall(r"^### (D-\d+)\b", open_section, re.MULTILINE)))

    def _conditioned_section(self):
        return self.register.split("Alcance condicionado", 1)[1].split("## Cerrado", 1)[0]

    def _conditioned_rows(self):
        """The debt ids the conditioned-scope table actually declares.

        Read from the table rows rather than from the whole section: the prose
        above the table enumerates the same ids, so searching the raw section
        would keep passing after a row is dropped from the table.
        """
        return sorted(set(self._CONDITIONED_ROW.findall(self._conditioned_section())))

    def test_every_conditioned_entry_is_named_with_its_condition(self):
        entries = self._open_entries()
        self.assertNotEqual(
            entries, [], "the register no longer declares open debt; this gate needs updating"
        )
        section = self._conditioned_section()
        rows = self._conditioned_rows()
        self.assertEqual(
            rows, entries, "the conditioned-scope table must declare exactly the open entries"
        )
        for entry in entries:
            with self.subTest(entry=entry):
                self.assertIn(
                    entry,
                    section,
                    f"{entry} is open debt but is missing from the conditioned-scope table",
                )
        self.assertIn(
            "Credenciales del propietario",
            section,
            "the table must name the external input that conditions the scope",
        )

    def test_register_does_not_claim_the_product_is_complete(self):
        self.assertIn(
            "que la funcionalidad del producto",
            self.register,
            "the declaration must state explicitly what it does not claim",
        )
        self.assertIn(
            "esté completa",
            self.register,
            "the completeness caveat must survive the declaration",
        )

    def test_matrix_marks_the_incomplete_rows_as_conditioned_scope(self):
        self.assertIn(
            "alcance condicionado",
            self.matrix,
            "the matrix must label its SoundCloud/Apple Music rows as conditioned scope",
        )
        self.assertIn(
            "Deuda_Tecnica_y_UX.md",
            self.matrix,
            "the matrix must point at the register entry that holds the condition",
        )


#: El relevo vivo del repositorio. Desde 2026-11-22 el relevo fechado vive en
#: `docs/historico/`; lo que queda aquí solo puede ser un puntero estable.
LIVE_HANDOFF = "docs/HANDOFF.md"

#: Marcadores de un relevo congelado: issues del tracker y listas de trabajo.
FROZEN_HANDOFF_ISSUE = re.compile(r"#\d+")
FROZEN_HANDOFF_PHRASES = ("Pendiente", "Siguiente paso")


def _frozen_handoff_markers(text: str) -> list[str]:
    """Lo que un puntero estable no puede contener: estado que envejece solo.

    Las referencias `#NN` y las secciones «Pendiente»/«Siguiente paso» son
    ciertas un día y falsas al siguiente; por eso viven en un registro fechado
    de `docs/historico/`, no en el fichero que se lee como relevo vivo.
    """
    return FROZEN_HANDOFF_ISSUE.findall(text) + [
        phrase for phrase in FROZEN_HANDOFF_PHRASES if phrase in text
    ]


class LiveHandoffGateTests(unittest.TestCase):
    """#366: el relevo vivo es un puntero, no un relevo congelado."""

    def test_live_handoff_is_a_pointer_and_not_a_frozen_relay(self):
        markers = _frozen_handoff_markers(_doc_text(LIVE_HANDOFF))
        self.assertEqual(
            markers,
            [],
            f"{LIVE_HANDOFF} volvió a guardar estado (issues o pendientes): el "
            "relevo fechado va a docs/historico/ y aquí solo queda el puntero",
        )

    def test_gate_catches_the_frozen_2026_11_09_relay(self):
        """El fixture es el relevo real congelado, no una copia inventada."""
        frozen = _doc_text("docs/historico/HANDOFF-2026-11-09.md")
        self.assertIn("## Pendiente", frozen)
        self.assertIn("## Siguiente paso concreto", frozen)
        self.assertTrue(
            FROZEN_HANDOFF_ISSUE.search(frozen),
            "el relevo congelado perdió sus referencias #NN: el fixture dejó de probar la puerta",
        )
        self.assertGreaterEqual(len(_frozen_handoff_markers(frozen)), 3)


#: Las dos páginas que fijan cómo viaja un PR de carril.
LANE_PR_PAGES = (
    "docs/agents/wiki/protocolo-pr.md",
    "docs/agents/wiki/modo-ejecucion.md",
)
#: La base que cada página declara para un PR, en cualquier mención.
PR_BASE_CLAIM = re.compile(r"PRs? con base `([^`]+)`")
#: Una rama de lote, escrita con números o con marcador de posición.
BATCH_BRANCH = re.compile(r"^wave/ola-(?:\d+|[Nn])-batch-(?:\d+|<[nNmM]>|[NnMm])$")
#: Toda rama de lote es la misma familia: el número de ola y de lote cambia.
BATCH_BRANCH_FAMILY = "wave/ola-N-batch-N"
LANE_PAGE_MENTION = re.compile(r"carril", re.IGNORECASE)


def _pr_base_targets(text: str) -> list[str]:
    """Las ramas que la página declara como base de un PR."""
    return [target.strip() for target in PR_BASE_CLAIM.findall(text)]


def _batch_branch_families(text: str) -> set[str]:
    """Las ramas de lote que la página nombra en las reglas de carril.

    Se leen solo las líneas que hablan de carriles y solo los tokens con forma
    de rama de lote: una cita histórica de `main` o de `dev` en otra frase no
    cuenta como la rama de integración de los carriles.
    """
    families = set()
    for line in text.splitlines():
        if not LANE_PAGE_MENTION.search(line):
            continue
        families.update(
            BATCH_BRANCH_FAMILY
            for token in BACKTICKED.findall(line)
            if BATCH_BRANCH.match(token.strip())
        )
    return families


def _lane_integration_violations(pages: dict[str, str]) -> list[str]:
    """Lo que hace que las páginas no digan lo mismo sobre la base del PR.

    Dos reglas: ninguna puede seguir mandando un PR de carril a `dev` (el flujo
    por lotes lo retiró) y ambas deben nombrar la misma familia de rama de lote.
    """
    problems = []
    families = {}
    for page, text in pages.items():
        for target in _pr_base_targets(text):
            if target == "dev":
                problems.append(f"{page}: declara «PR con base `dev`», retirada por el flujo por lotes")
        families[page] = _batch_branch_families(text)
        if not families[page]:
            problems.append(f"{page}: no nombra la rama del lote para los PRs de carril")
    if len({frozenset(branches) for branches in families.values()}) > 1:
        problems.append(f"las páginas no nombran la misma rama de integración: {families}")
    return problems


class LaneIntegrationBranchTests(unittest.TestCase):
    """#366: las dos páginas mandan los PRs de carril a la misma rama."""

    def _pages(self) -> dict[str, str]:
        return {page: _doc_text(page) for page in LANE_PR_PAGES}

    def test_both_pages_name_the_same_batch_branch(self):
        self.assertEqual(
            _lane_integration_violations(self._pages()),
            [],
            "protocolo-pr.md y modo-ejecucion.md deben mandar los PRs de carril "
            "a la misma rama de lote, y no a `dev`",
        )

    def test_gate_catches_the_dev_base_claim(self):
        problems = _lane_integration_violations(
            {
                "a": "PR de carril → rama del lote `wave/ola-0-batch-1`",
                "b": "- PR con base `dev`; squash-merge",
            }
        )
        self.assertTrue(
            any("`dev`" in problem for problem in problems),
            f"la puerta debe ver la base `dev`: {problems}",
        )

    def test_gate_catches_a_page_that_stops_naming_the_batch_branch(self):
        problems = _lane_integration_violations(
            {
                "a": "PR de carril → rama del lote `wave/ola-0-batch-1`",
                "b": "Los PRs de carril van contra la rama que toque",
            }
        )
        self.assertTrue(
            any("no nombra la rama del lote" in problem for problem in problems),
            f"la puerta debe ver la página sin rama de lote: {problems}",
        )


#: El estado de una cola es su primera línea `**Estado:**`.
COLA_STATE_LINE = re.compile(r"^\*\*Estado:\*\*(.*)$", re.MULTILINE)
CLOSED_COLA_STATE = re.compile(r"\b(?:cerrad|completad)[ao]s?\b", re.IGNORECASE)


def _declares_closed_state(text: str) -> bool:
    """Una cola cerrada solo se declara en su línea de estado.

    Buscar la palabra en todo el documento marcaría una cola abierta que cita
    un ítem suyo ya cerrado; el estado global es la primera línea `**Estado:**`.
    """
    match = COLA_STATE_LINE.search(text)
    return match is not None and CLOSED_COLA_STATE.search(match.group(1)) is not None


class ClosedQueueLocationTests(unittest.TestCase):
    """#366: una cola cerrada se archiva en `docs/historico/`, no se queda viva."""

    def test_no_closed_queue_stays_under_docs(self):
        live = sorted((REPO_ROOT / "docs").glob("COLA_*.md"))
        self.assertTrue(live, "no hay colas vivas bajo docs/: la puerta quedaría inerte")
        offenders = [
            path.name
            for path in live
            if _declares_closed_state(path.read_text(encoding="utf-8"))
        ]
        self.assertEqual(
            offenders,
            [],
            "estas colas se declaran cerradas bajo docs/: muévelas a docs/historico/",
        )

    def test_gate_reads_the_state_line_and_not_a_closed_item(self):
        open_queue = (
            "# Cola\n\n**Estado:** DLX-02 sigue abierto; DLX-04 se cerró el 2026-11-21.\n"
        )
        closed_queue = "# Cola\n\n**Estado:** cerrada el 2026-11-20; sin trabajo pendiente.\n"
        self.assertFalse(_declares_closed_state(open_queue))
        self.assertTrue(_declares_closed_state(closed_queue))


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False, verbosity=2).result.wasSuccessful() else 1)
