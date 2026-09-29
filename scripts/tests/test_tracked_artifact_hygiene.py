#!/usr/bin/env python3
"""Local sweep artifacts must not travel in the tree (SYNC-AUD-074).

The audit found 22 files of machine output committed by mistake: 13 `*.txt`
under `src-tauri/` and 9 under `ui/`, none of them ignored, none referenced
anywhere, ~240 KB that travelled in every checkout and every release. Their
content was also stale and actively misleading: `ui/build_output.txt` still
showed a `vue-tsc` error and `ui/tauri_dev_output.txt` a `tauri-cli` panic,
which reads as "the UI build is broken" to anyone who opens them.

The repair removes them from the tree and declares the families in
`.gitignore`. This module is the gate that keeps them from coming back. It is
hermetic: no network, no writes, and the git-backed assertions degrade to
`skipTest` outside a checkout instead of failing.
"""

import shutil
import subprocess
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# The 22 files that were tracked. Kept by name so a regression reports the
# exact path that came back instead of a count.
REMOVED_SWEEP_ARTIFACTS = (
    "src-tauri/check_err.txt",
    "src-tauri/current_tests.txt",
    "src-tauri/db_evidence.txt",
    "src-tauri/db_verification_output.txt",
    "src-tauri/full_test_results.txt",
    "src-tauri/run_out.txt",
    "src-tauri/test_err.txt",
    "src-tauri/test_list.txt",
    "src-tauri/test_out.txt",
    "src-tauri/test_out2.txt",
    "src-tauri/test_out_isolated.txt",
    "src-tauri/test_output.txt",
    "src-tauri/tests.txt",
    "ui/build_logs.txt",
    "ui/build_logs2.txt",
    "ui/build_logs3.txt",
    "ui/build_logs4.txt",
    "ui/build_logs5.txt",
    "ui/build_output.txt",
    "ui/build_output_2.txt",
    "ui/build_output_3.txt",
    "ui/tauri_dev_output.txt",
)

# The `*.txt` files the project legitimately versions.
ALLOWED_TRACKED_TXT = ("requirements.txt", "scripts/requirements.txt")

# Directories that are build output or dependencies, never repository content.
IGNORED_TREES = ("target", "node_modules", "dist", ".audit_tmp", "coverage")


def _git(*args):
    """Run a read-only git command, or return None outside a checkout."""
    if shutil.which("git") is None:
        return None
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout


class SweepArtifactHygieneTests(unittest.TestCase):
    """The 22 sweep files stay out of the tree and stay ignored."""

    def test_no_removed_sweep_artifact_exists_anymore(self):
        for relative in REMOVED_SWEEP_ARTIFACTS:
            with self.subTest(path=relative):
                self.assertFalse(
                    (REPO_ROOT / relative).exists(),
                    f"local sweep output came back: {relative}",
                )

    def test_source_trees_carry_no_txt_artefacts(self):
        """`src-tauri/` and `ui/` hold no `.txt` at all; the two that remain in
        the repository are the declared requirements files."""
        leftovers = []
        for root in ("src-tauri", "ui"):
            for path in (REPO_ROOT / root).rglob("*.txt"):
                if any(part in IGNORED_TREES for part in path.parts):
                    continue
                leftovers.append(str(path.relative_to(REPO_ROOT)))
        self.assertEqual(
            leftovers, [], f"machine output is back in the source trees: {leftovers}"
        )

    def test_gitignore_declares_the_sweep_families(self):
        """The families are declared, so a regenerated file is ignored."""
        gitignore = (REPO_ROOT / ".gitignore").read_text(encoding="utf-8")
        for pattern in (
            "src-tauri/test_out*.txt",
            "src-tauri/test_output.txt",
            "src-tauri/test_err.txt",
            "src-tauri/db_verification_output.txt",
            "ui/build_output*.txt",
            "ui/build_log*.txt",
            "ui/tauri_dev_output.txt",
        ):
            with self.subTest(pattern=pattern):
                self.assertIn(
                    pattern,
                    gitignore,
                    f".gitignore no longer declares {pattern!r}; regenerated "
                    f"sweep output would be committed again",
                )

    def test_every_removed_artifact_is_ignored_by_git(self):
        """Ask git itself, so a wrong pattern in `.gitignore` is caught."""
        if _git("rev-parse", "--is-inside-work-tree") is None:
            self.skipTest("not a git checkout; the ignore gate needs git")
        for relative in REMOVED_SWEEP_ARTIFACTS:
            with self.subTest(path=relative):
                completed = subprocess.run(
                    ["git", "check-ignore", "--no-index", "-q", "--", relative],
                    cwd=REPO_ROOT,
                    capture_output=True,
                    timeout=60,
                    check=False,
                )
                self.assertEqual(
                    completed.returncode,
                    0,
                    f"{relative} is not ignored any more",
                )

    def test_only_the_declared_requirements_files_are_tracked_as_txt(self):
        tracked = _git("ls-files", "*.txt")
        if tracked is None:
            self.skipTest("not a git checkout; the tracked-txt gate needs git")
        self.assertEqual(
            sorted(line for line in tracked.splitlines() if line),
            sorted(ALLOWED_TRACKED_TXT),
            "a new tracked .txt appeared outside the allowlist",
        )


if __name__ == "__main__":
    unittest.main()
