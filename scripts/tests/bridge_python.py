#!/usr/bin/env python3
"""The interpreter to spawn the Python bridges with, verified rather than assumed.

`sys.executable` is the usual answer for "which Python am I?", but inside a
session hosted by an AppImage it is not one: the environment exports `APPIMAGE`
and CPython then reports the host application as its own executable. Spawning
it launches a GUI process instead of running the bridge, so the CLI contract
tests read Electron startup logs where the bridge's JSON was expected and every
assertion about the payload fails for a reason that has nothing to do with the
bridge.

`test_download_bridge_contract.py` already worked around this inline, because
the same trap catches the Rust side (`get_python_executable`). This module is
that workaround, shared: prefer `sys.executable` when it really is an
interpreter, otherwise fall back to the running interpreter's own prefix and
then to PATH. A host with no `APPIMAGE` never takes the fallback, so the CI
mirror (.github/workflows/ci.yml) resolves exactly as before.
"""

import os
import shutil
import sys
from pathlib import Path


def _is_the_hosting_app(candidate):
    """True when the candidate is the AppImage hosting this session, not a Python."""
    appimage = os.environ.get("APPIMAGE")
    if not appimage:
        return False
    try:
        return os.path.realpath(candidate) == os.path.realpath(appimage)
    except OSError:
        return False


def is_interpreter(candidate):
    """Whether a path can be run as the interpreter, checked and not assumed.

    Three conditions, all of them observable: the path must name an existing
    file, the file must carry the execute bit, and it must not be the AppImage
    this session is hosted by.
    """
    if not candidate:
        return False
    path = Path(candidate)
    if not path.is_file() or not os.access(path, os.X_OK):
        return False
    return not _is_the_hosting_app(path)


def python_executable():
    """The interpreter to run a bridge with, or raise if the host has none.

    Resolution order: `sys.executable` when it passes `is_interpreter`, then
    the interpreter running this very process (`sys.base_prefix`), then PATH.
    """
    if is_interpreter(sys.executable):
        return sys.executable

    prefix_interpreter = Path(sys.base_prefix) / "bin" / "python3"
    for candidate in (prefix_interpreter, shutil.which("python3"), shutil.which("python")):
        if is_interpreter(candidate):
            return str(candidate)

    raise RuntimeError(
        "no Python interpreter available to run the bridges with: sys.executable "
        f"is {sys.executable!r}, and neither {prefix_interpreter} nor PATH yields one"
    )
