#!/usr/bin/env python3
"""Documentation consistency gates for the repaired findings.

These checks exist so the repairs do not rot back into the defects the audit
found, and they are hermetic: no network, no repository writes, only the tree.

Covered:

* SYNC-AUD-063 — the CI header must not describe a debt the workflow already closed.
* SYNC-AUD-064 — the README must point contributions at the live development branch.
* SYNC-AUD-065 — the README must not send anyone to `workspace/`, which `.gitignore`
  excludes and no checkout or CI ever contains.
* SYNC-AUD-067 — the debt register the code and the plan cite must exist in the tree.
* SYNC-AUD-068 — the parity matrix must not republish a stale suite figure.
* SYNC-AUD-070 — every file path the design docs cite must resolve, so "revalidated"
  is a checkable statement.
"""

import re
import sys
import unittest
from pathlib import Path

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
    "legacy/syncify-cli/src/download/lyrics.rs": "archived prototype kept outside the tree (SYNC-AUD-065)",
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


class DocumentationReferenceTests(unittest.TestCase):
    """SYNC-AUD-070: the design reference set must stay resolvable."""

    def _documents(self):
        docs = sorted((REPO_ROOT / "docs" / "features").glob("*.md"))
        docs += sorted(p for p in (REPO_ROOT / "docs").glob("*.md"))
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

    def test_general_suite_step_does_not_skip_batch_50(self):
        for line in self.ci.splitlines():
            if line.strip().startswith("run: cargo test --locked"):
                self.assertNotIn("--skip", line, "the general suite must not skip batch_50 anymore")

    def test_serialized_batch_50_step_has_its_own_timeout(self):
        self.assertIn("batch_50_audit_test serializado", self.ci)
        window = self.ci.split("batch_50_audit_test serializado", 1)[1].splitlines()[:4]
        self.assertTrue(
            any("timeout-minutes:" in line for line in window),
            "the isolated batch_50 step must keep its own time bound",
        )


class ReadmeTests(unittest.TestCase):
    """SYNC-AUD-064 and SYNC-AUD-065: the README must describe this tree."""

    def setUp(self):
        self.readme = _doc_text("README.md")

    def test_pr_target_is_the_development_branch(self):
        self.assertIn("Pull Request contra `syncify-app`", self.readme)
        self.assertNotIn("Pull Request contra `syncify-graphical`", self.readme)

    def test_no_pointer_to_the_gitignored_workspace_directory(self):
        for line in self.readme.splitlines():
            if "workspace/" in line:
                self.assertIn(
                    "no existe",
                    line,
                    f"README points at a gitignored path: {line!r}",
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


class PlanStateTests(unittest.TestCase):
    """SYNC-AUD-069: the plan must state what actually executed."""

    def test_plan_status_is_not_a_stale_proposal(self):
        plan = _doc_text("docs/PLAN_UNIFICACION_IMPORTACION.md")
        self.assertNotIn("Estado: propuesta aprobada (pendiente de ejecución)", plan)
        self.assertIn("EN EJECUCIÓN", plan)


class AcceptedQobuzAppIdRiskTests(unittest.TestCase):
    """SYNC-AUD-071: the Qobuz `app_id` exposure is accepted, so it stays written down.

    The decision is not a code fix: the public client id stays in the Python
    bridges on purpose while the paired secret never enters the tree. What must
    never regress is that the decision is visible and that the Rust core keeps
    its own placeholder.
    """

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

    def test_rust_core_keeps_the_app_id_out_of_source(self):
        core = _doc_text("src-tauri/src/services/qobuz.rs")
        self.assertNotIn(
            self._committed_public_app_id(),
            core,
            "the Rust core keeps a development placeholder for the app id",
        )


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False, verbosity=2).result.wasSuccessful() else 1)
