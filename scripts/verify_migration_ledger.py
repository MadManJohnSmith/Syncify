#!/usr/bin/env python3
"""Verify the sqlx migrations ledger (_sqlx_migrations) against the migrations tree.

Read-only tool: the database is always opened with URI ``mode=ro`` (and
``PRAGMA query_only`` is set for good measure), so this script can never write
to the database. Safe to run against a live app database.

Checks performed for every row of ``_sqlx_migrations``:
  - MISMATCH            applied version whose stored checksum differs from the
                        sha384 of its corresponding ``.sql`` file (CRITICAL).
  - APPLIED_FILE_MISSING applied version with no matching migration file in the
                        migrations directory (CRITICAL).
  - FAILED              ledger row registered with success != 1 (CRITICAL).
  - PENDING             migration file present in the directory but not yet
                        applied (INFORMATIONAL). A PENDING file with a version
                        number lower than the max applied version is reported
                        as a PENDING_OUT_OF_ORDER warning.

Exit codes:
  0  no MISMATCH / APPLIED_FILE_MISSING / FAILED rows.
  1  any of the above found, or the ledger/database could not be verified.

Usage:
  python3 scripts/verify_migration_ledger.py [--db PATH] [--migrations-dir PATH] [--json]

Defaults:
  --db              ~/.local/share/com.syncify.app/syncify.db
  --migrations-dir  <repo-root>/src-tauri/migrations, with the repo root
                    auto-detected by walking up from this script's location.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sqlite3
import sys
from pathlib import Path

MIGRATIONS_SUBTREE = Path("src-tauri") / "migrations"
LEDGER_TABLE = "_sqlx_migrations"

# Statuses that make the script exit non-zero.
CRITICAL_STATUSES = {"MISMATCH", "APPLIED_FILE_MISSING", "FAILED"}


def default_migrations_dir() -> Path:
    """Locate <repo-root>/src-tauri/migrations by walking up from this file."""
    here = Path(__file__).resolve().parent
    for candidate in (here, *here.parents):
        tree = candidate / MIGRATIONS_SUBTREE
        if tree.is_dir():
            return tree
    # Fall back to cwd-based detection.
    for candidate in (Path.cwd(), *Path.cwd().parents):
        tree = candidate / MIGRATIONS_SUBTREE
        if tree.is_dir():
            return tree
    return here.parent / MIGRATIONS_SUBTREE


def default_db_path() -> Path:
    return Path.home() / ".local" / "share" / "com.syncify.app" / "syncify.db"


def connect_readonly(db_path: Path) -> sqlite3.Connection:
    """Open the database strictly read-only via URI mode=ro."""
    uri = db_path.expanduser().resolve().as_uri() + "?mode=ro"
    conn = sqlite3.connect(uri, uri=True)
    conn.execute("PRAGMA query_only = 1")
    return conn


def sha384_file(path: Path) -> str:
    digest = hashlib.sha384()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def parse_version_prefix(stem: str) -> int | None:
    """sqlx names: <version>_<description>.sql -> leading integer version."""
    digits = stem.split("_", 1)[0]
    return int(digits) if digits.isdigit() else None


def scan_migrations_dir(migrations_dir: Path) -> tuple[dict[int, list[dict]], list[str]]:
    """Index migration files by version.

    Supports both sqlx layouts:
      - simple: <version>_<description>.sql
      - directory: <version>_<description>/up.sql
    Returns (files_by_version, warnings).
    """
    files: dict[int, list[dict]] = {}
    warnings: list[str] = []

    def record(version: int, path: Path, description: str) -> None:
        entry = {
            "path": str(path),
            "description": description,
            "checksum": sha384_file(path),
        }
        files.setdefault(version, []).append(entry)

    for entry in sorted(migrations_dir.iterdir()):
        if entry.is_file() and entry.suffix.lower() == ".sql":
            version = parse_version_prefix(entry.stem)
            if version is None:
                warnings.append(f"ignorable file without numeric version prefix: {entry.name}")
                continue
            description = entry.stem.split("_", 1)[1] if "_" in entry.stem else entry.stem
            record(version, entry, description)
        elif entry.is_dir() and (entry / "up.sql").is_file():
            version = parse_version_prefix(entry.name)
            if version is None:
                warnings.append(f"ignorable directory without numeric version prefix: {entry.name}")
                continue
            description = entry.name.split("_", 1)[1] if "_" in entry.name else entry.name
            record(version, entry / "up.sql", description)

    for version, entries in files.items():
        if len(entries) > 1:
            warnings.append(
                f"duplicate migration files for version {version}: "
                + ", ".join(Path(e["path"]).name for e in entries)
            )
    return files, warnings


def checksum_to_hex(stored: object) -> str | None:
    """Normalize the checksum column (BLOB or TEXT) to uppercase hex."""
    if isinstance(stored, (bytes, bytearray, memoryview)):
        return bytes(stored).hex().upper()
    if isinstance(stored, str):
        return stored.removeprefix("X'").removesuffix("'").upper()
    return None


def verify(db_path: Path, migrations_dir: Path) -> dict:
    result: dict = {
        "db": str(db_path),
        "migrations_dir": str(migrations_dir),
        "ok": False,
        "applied_count": 0,
        "max_applied_version": None,
        "rows": [],
        "pending": [],
        "warnings": [],
    }

    if not db_path.is_file():
        result["warnings"].append(f"database file not found: {db_path}")
        result["error"] = "database-not-found"
        return result
    if not migrations_dir.is_dir():
        result["warnings"].append(f"migrations directory not found: {migrations_dir}")
        result["error"] = "migrations-dir-not-found"
        return result

    files_by_version, dir_warnings = scan_migrations_dir(migrations_dir)
    result["warnings"].extend(dir_warnings)

    conn = connect_readonly(db_path)
    try:
        try:
            rows = conn.execute(
                f"SELECT version, description, checksum, success FROM {LEDGER_TABLE} ORDER BY version"
            ).fetchall()
        except sqlite3.Error as exc:
            result["warnings"].append(f"cannot read ledger table {LEDGER_TABLE}: {exc}")
            result["error"] = "ledger-unreadable"
            return result
    finally:
        conn.close()

    result["applied_count"] = len(rows)
    if rows:
        result["max_applied_version"] = max(int(r[0]) for r in rows)
    applied_versions: set[int] = set()

    for version, description, checksum, success in rows:
        version = int(version)
        applied_versions.add(version)
        row: dict = {
            "version": version,
            "description": description,
            "status": "OK",
            "success": bool(success),
            "file": None,
            "expected_checksum": None,
            "stored_checksum": checksum_to_hex(checksum),
        }

        if not success:
            row["status"] = "FAILED"

        candidates = files_by_version.get(version, [])
        if not candidates:
            row["status"] = "APPLIED_FILE_MISSING"
        else:
            entry = candidates[0]
            row["file"] = entry["path"]
            row["expected_checksum"] = entry["checksum"]
            if row["stored_checksum"] is None or row["stored_checksum"] != entry["checksum"]:
                if row["status"] == "OK":
                    row["status"] = "MISMATCH"

        result["rows"].append(row)

    max_applied = result["max_applied_version"]
    for version in sorted(files_by_version):
        if version in applied_versions:
            continue
        entry = files_by_version[version][0]
        pending_row = {
            "version": version,
            "description": entry["description"],
            "file": entry["path"],
            "out_of_order": max_applied is not None and version < max_applied,
        }
        result["pending"].append(pending_row)

    critical = [r for r in result["rows"] if r["status"] in CRITICAL_STATUSES]
    out_of_order = [p for p in result["pending"] if p["out_of_order"]]
    result["ok"] = not critical
    if out_of_order:
        result["warnings"].append(
            "pending migrations with version lower than max applied: "
            + ", ".join(str(p["version"]) for p in out_of_order)
        )
    return result


def print_human(result: dict) -> None:
    print(f"Database        : {result['db']}")
    print(f"Migrations dir  : {result['migrations_dir']}")
    print(f"Ledger rows     : {result['applied_count']} "
          f"(max applied version: {result['max_applied_version']})")
    print("-" * 78)

    status_order = {"MISMATCH": 0, "APPLIED_FILE_MISSING": 0, "FAILED": 0, "OK": 1}
    rows = sorted(result["rows"], key=lambda r: (status_order.get(r["status"], 1), r["version"]))
    for row in rows:
        marker = "  !! " if row["status"] in CRITICAL_STATUSES else "     "
        line = f"{marker}v{row['version']:<6} {row['status']:<22} {row['description']}"
        print(line)
        if row["status"] == "MISMATCH":
            print(f"        file    : {row['file']}")
            print(f"        expected: {row['expected_checksum']}")
            print(f"        stored  : {row['stored_checksum']}")
        elif row["status"] == "APPLIED_FILE_MISSING":
            print(f"        no matching migration file found in {result['migrations_dir']}")

    if result["pending"]:
        print("-" * 78)
        print(f"PENDING (not yet applied, informational): {len(result['pending'])}")
        for p in result["pending"]:
            flag = " [OUT-OF-ORDER: lower than max applied version]" if p["out_of_order"] else ""
            print(f"     v{p['version']:<6} {p['description']}{flag}")

    if result["warnings"]:
        print("-" * 78)
        for warning in result["warnings"]:
            print(f"WARNING: {warning}")

    print("-" * 78)
    if result.get("error"):
        print(f"RESULT: VERIFICATION ERROR ({result['error']})")
    elif result["ok"]:
        print("RESULT: OK - ledger matches migrations tree")
    else:
        print("RESULT: FAILED - checksum mismatches or missing migration files detected")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Read-only verification of the sqlx migrations ledger against migration files."
    )
    parser.add_argument("--db", type=Path, default=default_db_path(),
                        help="path to the SQLite database (default: %(default)s)")
    parser.add_argument("--migrations-dir", type=Path, default=None,
                        help="migrations directory (default: repo-root/src-tauri/migrations, auto-detected)")
    parser.add_argument("--json", action="store_true",
                        help="machine-readable JSON output")
    args = parser.parse_args(argv)

    migrations_dir = args.migrations_dir or default_migrations_dir()
    result = verify(args.db, migrations_dir)

    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print_human(result)

    return 0 if result.get("ok") else 1


if __name__ == "__main__":
    sys.exit(main())
