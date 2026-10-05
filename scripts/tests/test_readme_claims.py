#!/usr/bin/env python3
"""Front-page claim gates for README.md and README.es.md.

The 73-item audit found both front pages promising things the tree does not
do: credentials living in the OS keyring with no fallback, DASH decryption
credited to Qobuz, two-way favourite sync from the library view, playlists
surviving a migration with their names and order, and an unbounded "import
everything". The repairs are prose, so nothing but a gate stops them rotting
back. Both READMEs have to keep the same guarantees, in their own language.

Hermetic like its sibling gate: no network, no repository writes, only the
tree.
"""

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
READMES = ("README.md", "README.es.md")

# Claims the code does not honour. Whitespace is normalised before matching so
# a line break inside a sentence cannot smuggle one of these back in.
FORBIDDEN_CLAIMS = (
    ("unbounded import promise", r"import everything"),
    ("unbounded import promise", r"importa todo"),
    ("full-catalog promise", r"your full catalog"),
    ("full-catalog promise", r"tu cat[áa]logo completo"),
    ("two-way favourite sync", r"favorites stay in sync both ways"),
    ("two-way favourite sync", r"favoritos se sincronizan en ambos sentidos"),
    ("order-preserving playlist migration", r"playlists move with their order and names intact"),
    ("order-preserving playlist migration", r"playlists viajan con su orden y nombres"),
    ("keychain-only credential storage", r"credentials are stored in your OS keyring"),
    ("keychain-only credential storage", r"credenciales se guardan en el llavero"),
)

# The disclosure each language owes the reader, once the promise above is gone.
REQUIRED_DISCLOSURES = (
    (
        "the library view never writes to the connected accounts",
        r"not by the library view",
        r"no desde la vista de biblioteca",
    ),
    (
        "the encryption key falls back to a file when there is no keyring",
        r"`\.crypto_key`",
        r"`\.crypto_key`",
    ),
    (
        "migration transfers favourites, not playlists",
        r"does not rebuild your playlists",
        r"no reconstruye tus playlists",
    ),
)

MARKDOWN_LINK = re.compile(r"\]\(([^)\s]+)\)")


def _read(relative: str) -> str:
    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def _flatten(text: str) -> str:
    return " ".join(text.split())


class ReadmeClaimTests(unittest.TestCase):
    """Both front pages must make the same promises, in their own language."""

    def setUp(self):
        self.raw = {name: _read(name) for name in READMES}
        self.flat = {name: _flatten(text) for name, text in self.raw.items()}

    def test_overclaims_are_gone(self):
        for label, pattern in FORBIDDEN_CLAIMS:
            for name, text in self.flat.items():
                self.assertIsNone(
                    re.search(pattern, text, re.IGNORECASE),
                    f"{name} still makes the {label} claim: {pattern!r}",
                )

    def test_required_disclosures_are_present(self):
        for label, english, spanish in REQUIRED_DISCLOSURES:
            for name, pattern in (("README.md", english), ("README.es.md", spanish)):
                self.assertRegex(
                    self.flat[name],
                    re.compile(pattern, re.IGNORECASE),
                    f"{name} must state that {label}",
                )

    def test_qobuz_is_never_credited_with_dash_decryption(self):
        # Qobuz serves direct FLAC URLs; the segmented delivery the pipeline
        # assembles belongs to Tidal. Any sentence naming DASH must not name
        # Qobuz, whichever document it sits in.
        for name in READMES + ("docs/README.md", "docs/README.en.md"):
            for sentence in re.split(r"(?<=[.!?])\s+", _flatten(_read(name))):
                if re.search(r"\bDASH\b", sentence, re.IGNORECASE):
                    self.assertNotIn(
                        "Qobuz",
                        sentence,
                        f"{name} attributes DASH to Qobuz: {sentence!r}",
                    )


class ReadmeLinkTests(unittest.TestCase):
    """A link or an anchor a reader follows and finds broken is the oldest lie."""

    def _anchors(self, text: str):
        headings = re.findall(r"(?m)^#{1,6}\s+(.*)$", text)
        return {self._slug(heading) for heading in headings}

    @staticmethod
    def _slug(heading: str) -> str:
        cleaned = re.sub(r"[^\w\s-]", "", heading.strip().lower())
        return re.sub(r"\s+", "-", cleaned)

    def _documents(self):
        return READMES + ("CONTRIBUTING.md", "docs/README.md", "docs/README.en.md")

    def test_relative_links_resolve(self):
        broken = []
        for name in self._documents():
            base = (REPO_ROOT / name).parent
            for target in MARKDOWN_LINK.findall(_read(name)):
                if target.startswith(("http://", "https://", "mailto:")):
                    continue
                path = target.split("#", 1)[0]
                # A relative link resolves against the document that holds it.
                if path and not (base / path).exists():
                    broken.append(f"{name} -> {target}")
        self.assertEqual([], broken, f"front-page links that do not resolve: {broken}")

    def test_anchors_resolve(self):
        broken = []
        for name in self._documents():
            text = _read(name)
            anchors = self._anchors(text)
            for target in MARKDOWN_LINK.findall(text):
                if not target.startswith("#"):
                    continue
                anchor = target[1:].lower()
                if anchor not in anchors:
                    broken.append(f"{name} -> {target}")
        self.assertEqual([], broken, f"front-page anchors with no heading: {broken}")

    def test_english_index_is_reachable_from_both_front_pages(self):
        # The English front page used to link the Spanish-only docs index.
        self.assertIn("(docs/README.en.md)", _read("README.md"))
        self.assertIn("(docs/README.en.md)", _read("README.es.md"))
        self.assertIn("(README.md)", _read("docs/README.en.md"))
        self.assertIn("(README.en.md)", _read("docs/README.md"))


class ContributingCheckTests(unittest.TestCase):
    """CONTRIBUTING.md quotes commands; a command nobody runs is a trap."""

    SHELL_COMMAND = re.compile(r"^(?:cargo|npm|npx|python3?|cd)\b")

    def _documented_commands(self):
        commands = ["cargo test --locked --test batch_50_audit_test -- --test-threads=1"]
        for line in _read("CONTRIBUTING.md").splitlines():
            if not line.startswith("|"):
                continue
            for token in re.findall(r"`([^`]+)`", line):
                if not self.SHELL_COMMAND.match(token):
                    continue
                for step in token.split("&&"):
                    step = step.strip()
                    # `cd ui` is the job's working-directory in the workflow,
                    # not part of the command that job runs.
                    if step and not step.startswith("cd "):
                        commands.append(step)
        return commands

    def test_every_documented_command_is_one_the_workflow_runs(self):
        ci = _read(".github/workflows/ci.yml")
        commands = self._documented_commands()
        self.assertGreaterEqual(len(commands), 6, "the check table lost its rows")
        for command in commands:
            self.assertIn(command, ci, f"CONTRIBUTING.md documents a command CI never runs: {command!r}")

    def test_the_paths_ci_ignores_are_disclosed(self):
        contributing = _read("CONTRIBUTING.md")
        for ignored in ("**.md", "docs/**"):
            self.assertIn(ignored, _read(".github/workflows/ci.yml"), f"the workflow no longer ignores {ignored}")
            self.assertIn(ignored, contributing, f"CONTRIBUTING.md must disclose that CI ignores {ignored}")

    def test_the_documentation_gates_are_named_where_they_run(self):
        contributing = _read("CONTRIBUTING.md")
        for gate in ("scripts/tests/test_docs_consistency.py", "scripts/tests/test_readme_claims.py"):
            self.assertIn(gate, contributing, f"CONTRIBUTING.md must name the gate {gate}")
            self.assertTrue((REPO_ROOT / gate).exists(), f"{gate} is cited but missing from the tree")
