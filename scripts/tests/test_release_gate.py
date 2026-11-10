#!/usr/bin/env python3
"""
Behavior tests for the publication gate CLI (scripts/ci/release_gate.py, #240).

The gate decides whether a release tag may exist, with three rules evaluated
in this order: tag format vX.Y.Z -> green CI for the exact SHA -> tag is not
movable (peeled `^{}` resolution for annotated tags).

Every test crosses the CLI seam: it runs the script as a subprocess with fake
`gh` and `git` executables injected via PATH, driven by a scenario file. No
network, no GitHub Actions. Assertions are observable behavior — exit codes
and `::error::` diagnostics — never the script's internal text or workflow
YAML.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
GATE_SCRIPT = REPO_ROOT / "scripts" / "ci" / "release_gate.py"

REPO = "MadManJohnSmith/Syncify"
SHA = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
OTHER_SHA = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

# El fake de `gh` no se limita a responder: valida el contrato de la
# invocacion (listar corridas completadas de ci.yml para el SHA y el repo
# pedidos) para que un cambio de contrato en el script falle aqui y no en
# Actions.
FAKE_GH = r"""#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
state = json.load(open(os.environ["RELEASE_GATE_FAKE_STATE"], encoding="utf-8"))
log = os.environ.get("RELEASE_GATE_FAKE_LOG")
if log:
    with open(log, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"tool": "gh", "args": args}) + "\n")

if args[:2] == ["run", "list"]:
    for flag in ("--repo", "--workflow", "--commit", "--status"):
        if flag not in args:
            sys.stderr.write(f"fake gh: missing {flag}\n")
            sys.exit(5)
    if args[args.index("--workflow") + 1] != "ci.yml":
        sys.stderr.write("fake gh: expected ci.yml\n")
        sys.exit(5)
    if args[args.index("--status") + 1] != "completed":
        sys.stderr.write("fake gh: expected completed\n")
        sys.exit(5)
    repo = args[args.index("--repo") + 1]
    expected_repo = state.get("expect_repo")
    if expected_repo and repo != expected_repo:
        sys.stderr.write(f"fake gh: expected repo {expected_repo}, got {repo}\n")
        sys.exit(5)
    sha = args[args.index("--commit") + 1]
    for conclusion in state.get("runs", {}).get(sha, []):
        print(conclusion)
    sys.exit(0)

sys.stderr.write(f"fake gh: unexpected invocation: {args}\n")
sys.exit(5)
"""

# El fake de `git` resuelve refs/tags/<TAG> y refs/tags/<TAG>^{} contra el
# escenario, igual que `git ls-remote --tags` contra el remoto real. El
# rev-parse falla a proposito: el gate corre sin clone (asi lo hace el job
# gate en Actions) y debe caer al SHA como ref sugerida.
FAKE_GIT = r"""#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
state = json.load(open(os.environ["RELEASE_GATE_FAKE_STATE"], encoding="utf-8"))
log = os.environ.get("RELEASE_GATE_FAKE_LOG")
if log:
    with open(log, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"tool": "git", "args": args}) + "\n")

if args[:2] == ["rev-parse", "--abbrev-ref"]:
    sys.exit(1)

if args[:1] == ["ls-remote"]:
    refs = state.get("remote_refs", {})
    pattern = args[-1]
    sha = refs.get(pattern)
    if sha:
        print(f"{sha}\t{pattern}")
    sys.exit(0)

sys.stderr.write(f"fake git: unexpected invocation: {args}\n")
sys.exit(5)
"""


class ReleaseGateCLITest(unittest.TestCase):
    """Exit codes and diagnostics of the gate CLI, with injected fakes."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.bin_dir = base / "bin"
        self.bin_dir.mkdir()
        for name, body in (("gh", FAKE_GH), ("git", FAKE_GIT)):
            fake = self.bin_dir / name
            fake.write_text(body, encoding="utf-8")
            fake.chmod(0o755)
        self.state_path = base / "scenario.json"
        self.log_path = base / "invocations.jsonl"
        self.state_path.write_text("{}", encoding="utf-8")
        self._write_state({})

    def tearDown(self):
        self.tmp.cleanup()

    # ── helpers ────────────────────────────────────────────────────────

    def _write_state(self, state: dict) -> None:
        self.state_path.write_text(json.dumps(state), encoding="utf-8")

    def _run_gate(self, *cli_args: str, github_repository: str | None = None) -> subprocess.CompletedProcess:
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in ("GITHUB_REPOSITORY", "GH_TOKEN")
        }
        env["PATH"] = f"{self.bin_dir}{os.pathsep}{env.get('PATH', '')}"
        env["GH_TOKEN"] = "fake-token-for-tests"
        env["RELEASE_GATE_FAKE_STATE"] = str(self.state_path)
        env["RELEASE_GATE_FAKE_LOG"] = str(self.log_path)
        if github_repository is not None:
            env["GITHUB_REPOSITORY"] = github_repository
        return subprocess.run(
            [sys.executable, str(GATE_SCRIPT), *cli_args],
            capture_output=True,
            text=True,
            env=env,
            cwd=self.tmp.name,  # fuera del repo: sin clone, como en Actions
            timeout=60,
        )

    def _invocations(self) -> list[dict]:
        if not self.log_path.exists():
            return []
        return [
            json.loads(line)
            for line in self.log_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]

    def _approved_scenario(self) -> dict:
        """Scenario that passes all three rules: green CI, tag nonexistent."""
        return {"runs": {SHA: ["success"]}, "remote_refs": {}}

    def _assert_rejected(self, proc: subprocess.CompletedProcess) -> None:
        self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
        self.assertIn("::error::", proc.stdout)

    # ── regla 1: formato ───────────────────────────────────────────────

    def test_valid_tag_format_approves(self):
        self._write_state(self._approved_scenario())
        proc = self._run_gate("--tag", "v1.2.3", "--sha", SHA, "--repo", REPO)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertNotIn("::error::", proc.stdout)

    def test_invalid_tag_format_rejects_without_querying_ci(self):
        proc = self._run_gate("--tag", "1.2.3", "--sha", SHA, "--repo", REPO)
        self._assert_rejected(proc)
        self.assertIn("no tiene la forma vX.Y.Z", proc.stdout)
        # El orden de la puerta es formato -> CI -> tag: con formato invalido
        # no se llega a consultar ninguna corrida ni ninguna ref remota.
        self.assertEqual(self._invocations(), [])

    # ── regla 2: CI verde ──────────────────────────────────────────────

    def test_no_completed_runs_rejects_with_paths_ignore_hint_and_relaunch_command(self):
        self._write_state({"runs": {}})
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self._assert_rejected(proc)
        self.assertIn(f"No hay ninguna corrida de CI completada para {SHA}", proc.stdout)
        # Diagnosticos que la copia muerta conservaba y las copias vivas
        # habian perdido: la pista de paths-ignore y el comando de relanzo.
        self.assertIn("paths-ignore", proc.stdout)
        self.assertIn(f"gh workflow run ci.yml --ref {SHA}", proc.stdout)

    def test_failed_ci_runs_reject(self):
        self._write_state({"runs": {SHA: ["failure", "cancelled"]}})
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self._assert_rejected(proc)
        self.assertIn(f"La CI de {SHA} no esta en verde", proc.stdout)
        self.assertIn("failure", proc.stdout)

    # ── exito / regla 3: tag no movible ────────────────────────────────

    def test_success_with_nonexistent_tag_reports_creation(self):
        self._write_state(self._approved_scenario())
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertIn(f"El tag v0.2.1 no existe todavia. Se creara apuntando a {SHA}.", proc.stdout)
        self.assertNotIn("::error::", proc.stdout)

    def test_existing_tag_at_same_sha_allows(self):
        self._write_state({
            "runs": {SHA: ["success"]},
            "remote_refs": {f"refs/tags/v0.2.1": SHA},
        })
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertIn(f"El tag v0.2.1 ya existe y apunta al mismo commit {SHA}.", proc.stdout)

    def test_existing_tag_at_other_sha_rejects(self):
        self._write_state({
            "runs": {SHA: ["success"]},
            "remote_refs": {f"refs/tags/v0.2.1": OTHER_SHA},
        })
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self._assert_rejected(proc)
        self.assertIn(f"El tag v0.2.1 ya existe y apunta a {OTHER_SHA}, no a {SHA}.", proc.stdout)
        self.assertIn("No se mueve un tag existente. Publica una version nueva.", proc.stdout)

    def test_annotated_tag_peeled_at_same_sha_allows(self):
        # Un tag anotado lista dos lineas: el tag object en refs/tags/TAG y
        # el commit en la peeled refs/tags/TAG^{}. La puerta resuelve por la
        # peeled: mismo commit -> se permite.
        self._write_state({
            "runs": {SHA: ["success"]},
            "remote_refs": {
                "refs/tags/v0.2.1": "cccccccccccccccccccccccccccccccccccccccc",
                "refs/tags/v0.2.1^{}": SHA,
            },
        })
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertIn(f"El tag v0.2.1 ya existe y apunta al mismo commit {SHA}.", proc.stdout)

    def test_annotated_tag_peeled_at_other_sha_rejects(self):
        self._write_state({
            "runs": {SHA: ["success"]},
            "remote_refs": {
                "refs/tags/v0.2.1": "cccccccccccccccccccccccccccccccccccccccc",
                "refs/tags/v0.2.1^{}": OTHER_SHA,
            },
        })
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, "--repo", REPO)
        self._assert_rejected(proc)
        self.assertIn(f"El tag v0.2.1 ya existe y apunta a {OTHER_SHA}, no a {SHA}.", proc.stdout)

    # ── interfaz: --repo por defecto desde GITHUB_REPOSITORY ──────────

    def test_repo_defaults_to_github_repository_env(self):
        self._write_state({**self._approved_scenario(), "expect_repo": REPO})
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA, github_repository=REPO)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        # El fake de gh valida el contrato: si el script no hubiera pasado
        # --repo con el valor del entorno, la corrida habria fallado (exit 5).

    def test_missing_repo_is_a_usage_error_not_a_verdict(self):
        proc = self._run_gate("--tag", "v0.2.1", "--sha", SHA)
        self.assertEqual(proc.returncode, 2, proc.stdout + proc.stderr)
        self.assertNotIn("::error::", proc.stdout)


if __name__ == "__main__":
    unittest.main()
