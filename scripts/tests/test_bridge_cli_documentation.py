#!/usr/bin/env python3
"""
Every command a bridge advertises in its module docstring must exist (PY-11).

`organizer_bridge.py` documented a `rename` subcommand that `main()` never
registered, so the documented CLI and the real CLI disagreed. This suite keeps
them in sync for every bridge that uses argparse subcommands.
"""

import ast
import re
import subprocess
import unittest
from pathlib import Path

from bridge_python import python_executable

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"

USAGE_PATTERN = re.compile(r"^python\s+(\S+\.py)\s+(\S+)")
CHOICE_GROUP_PATTERN = re.compile(r"\{([a-zA-Z0-9_\-,\s]+)\}")


def documented_commands(source: str):
    """Return the (script, command) pairs advertised in the module docstring."""
    docstring = ast.get_docstring(ast.parse(source)) or ""
    commands = []
    for line in docstring.splitlines():
        match = USAGE_PATTERN.match(line.strip())
        if match:
            commands.append((match.group(1), match.group(2)))
    return commands


def registered_commands(script: Path):
    """Return the argparse subcommands of `script`, or None when it has none."""
    proc = subprocess.run(
        [python_executable(), str(script), "--help"],
        capture_output=True,
        text=True,
        timeout=120,
    )
    output = proc.stdout + proc.stderr
    match = CHOICE_GROUP_PATTERN.search(output)
    if not match:
        return None
    return {choice.strip() for choice in match.group(1).split(",") if choice.strip()}


class BridgeDocumentationTests(unittest.TestCase):
    def test_documented_commands_are_registered(self):
        bridges = sorted(SCRIPTS_DIR.glob("*_bridge.py"))
        self.assertTrue(bridges, "no bridge scripts found to check")

        for bridge in bridges:
            with self.subTest(bridge=bridge.name):
                available = registered_commands(bridge)
                if available is None:
                    # Positional-CLI bridge (no argparse subcommands): nothing to compare.
                    continue
                for script_name, command in documented_commands(bridge.read_text(encoding="utf-8")):
                    self.assertEqual(
                        script_name,
                        bridge.name,
                        f"{bridge.name} documents commands of another script: {script_name}",
                    )
                    self.assertIn(
                        command,
                        available,
                        f"{bridge.name} documents '{script_name} {command}' but main() does not "
                        f"register it (registered: {', '.join(sorted(available))})",
                    )

    def test_organizer_bridge_no_longer_documents_rename(self):
        """PY-11: `rename` never existed in organizer_bridge.main()."""
        source = (SCRIPTS_DIR / "organizer_bridge.py").read_text(encoding="utf-8")

        self.assertNotIn("organizer_bridge.py rename", source)
        self.assertEqual(registered_commands(SCRIPTS_DIR / "organizer_bridge.py"), {"preview", "organize"})


if __name__ == "__main__":
    unittest.main()