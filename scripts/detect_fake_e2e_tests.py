#!/usr/bin/env python3
"""Detector for fake, tautological, and ungrounded E2E tests in Syncify.

TASK-125 Acceptance Gate:
1. Verifies that all core E2E integration test suites in `src-tauri/tests/`
   reference and exercise authentic production code from `syncify_tauri_lib`,
   `syncify_core_domain`, `syncify_flac_writer`, or `syncify_desktop` rather than
   fabricating detached mocks and local SQL simulations.
2. Verifies that Rust, Python, and TypeScript test suites do not contain tautological
   assertions (e.g., `assert!(true)`, `assert step is False or True`, `in (False, True)`,
   or empty test blocks without assertions).

Returns exit code 0 if all tests are legitimate, or exit code 1 with a diagnostic violation report.
"""

import re
import sys
from pathlib import Path
from typing import List, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
SRC_TAURI_TESTS = REPO_ROOT / "src-tauri" / "tests"
SCRIPTS_TESTS = REPO_ROOT / "scripts" / "tests"
UI_TESTS = REPO_ROOT / "ui" / "src" / "__tests__"

# Production crates / modules that real Syncify Rust tests must reference
PRODUCTION_RUST_MODULES = [
    "syncify_tauri_lib",
    "syncify_core_domain",
    "syncify_flac_writer",
    "syncify_tidal_downloader",
    "syncify_desktop",
]

# E2E integration suites covered under TASK-125
TARGET_E2E_SUITES = [
    "backup_restore_e2e_test.rs",
    "dashboard_health_e2e_test.rs",
    "downloads_flow_e2e_test.rs",
    "integrity_audit_e2e_test.rs",
    "queue_concurrency_and_reconciliation_test.rs",
    "notifications_e2e_test.rs",
    "qobuz_parity_e2e_test.rs",
    "reauth_and_recovery_e2e_test.rs",
]

# Patterns representing tautological / fake assertions in Rust
RUST_TAUTOLOGY_PATTERNS = [
    (re.compile(r"assert!\s*\(\s*true\s*[\),]", re.IGNORECASE), "Literal assert!(true) tautology"),
    (re.compile(r"assert_eq!\s*\(\s*true\s*,\s*true\s*\)", re.IGNORECASE), "Literal assert_eq!(true, true) tautology"),
    (re.compile(r"assert_eq!\s*\(\s*1\s*,\s*1\s*\)", re.IGNORECASE), "Literal assert_eq!(1, 1) tautology"),
]

# Patterns representing tautological / fake assertions in Python
PYTHON_TAUTOLOGY_PATTERNS = [
    (re.compile(r"assert\s+.*\s+(is\s+False\s+or\s+True|is\s+True\s+or\s+False)", re.IGNORECASE), "Tautology: 'is False or True'"),
    (re.compile(r"in\s*\(\s*False\s*,\s*True\s*\)", re.IGNORECASE), "Tautology: membership in (False, True) covers all booleans"),
    (re.compile(r"self\.assertTrue\s*\(\s*True\s*\)"), "Literal self.assertTrue(True)"),
    (re.compile(r"self\.assertFalse\s*\(\s*False\s*\)"), "Literal self.assertFalse(False)"),
]


class Violation:
    def __init__(self, file_path: Path, line_num: int, message: str, rule: str):
        self.file_path = file_path
        self.line_num = line_num
        self.message = message
        self.rule = rule

    def __str__(self) -> str:
        rel = self.file_path.relative_to(REPO_ROOT) if self.file_path.is_relative_to(REPO_ROOT) else self.file_path
        return f"[{self.rule}] {rel}:{self.line_num} -> {self.message}"


def check_rust_e2e_tests() -> Tuple[List[Violation], int, int]:
    violations: List[Violation] = []
    total_rust_tests = 0
    e2e_tests_count = 0

    if not SRC_TAURI_TESTS.exists():
        return violations, 0, 0

    for test_file in sorted(SRC_TAURI_TESTS.glob("*.rs")):
        total_rust_tests += 1
        content = test_file.read_text(encoding="utf-8", errors="replace")
        lines = content.splitlines()

        if test_file.name in TARGET_E2E_SUITES:
            e2e_tests_count += 1
            # Verify reference to real production crates/modules
            has_production_reference = any(
                mod in content for mod in PRODUCTION_RUST_MODULES
            )

            if not has_production_reference:
                violations.append(
                    Violation(
                        test_file,
                        1,
                        f"E2E test suite does not reference any production module ({', '.join(PRODUCTION_RUST_MODULES)})",
                        "FAKE_E2E_NO_PRODUCTION_CODE",
                    )
                )

        # Check for tautologies in all Rust test files
        for idx, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                continue
            for pattern, msg in RUST_TAUTOLOGY_PATTERNS:
                if pattern.search(line):
                    violations.append(Violation(test_file, idx, msg, "TAUTOLOGICAL_ASSERTION"))

    return violations, total_rust_tests, e2e_tests_count


def check_python_tests() -> Tuple[List[Violation], int]:
    violations: List[Violation] = []
    total_py_tests = 0

    if not SCRIPTS_TESTS.exists():
        return violations, 0

    for py_file in sorted(SCRIPTS_TESTS.glob("*.py")):
        total_py_tests += 1
        content = py_file.read_text(encoding="utf-8", errors="replace")
        lines = content.splitlines()

        for idx, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith("#"):
                continue
            for pattern, msg in PYTHON_TAUTOLOGY_PATTERNS:
                if pattern.search(line):
                    violations.append(Violation(py_file, idx, msg, "TAUTOLOGICAL_ASSERTION"))

    return violations, total_py_tests


def check_typescript_tests() -> Tuple[List[Violation], int]:
    violations: List[Violation] = []
    total_ts_tests = 0

    if not UI_TESTS.exists():
        return violations, 0

    test_files = list(UI_TESTS.rglob("*.spec.ts")) + list(UI_TESTS.rglob("*.test.ts"))
    it_pattern = re.compile(r"(?:it|test)\s*\(\s*['\"`]([^'\"`]+)['\"`]\s*,\s*(?:async\s*)?\(\s*\)\s*=>\s*\{")

    for ts_file in sorted(test_files):
        total_ts_tests += 1
        content = ts_file.read_text(encoding="utf-8", errors="replace")
        lines = content.splitlines()

        # Find tests that contain no expect / assert
        for match in it_pattern.finditer(content):
            test_name = match.group(1)
            start_pos = match.end()
            # find matching closing brace
            brace_depth = 1
            curr_pos = start_pos
            while curr_pos < len(content) and brace_depth > 0:
                ch = content[curr_pos]
                if ch == '{':
                    brace_depth += 1
                elif ch == '}':
                    brace_depth -= 1
                curr_pos += 1
            test_body = content[start_pos:curr_pos - 1]

            # Check if body has any assertion
            has_assertion = "expect(" in test_body or "assert(" in test_body
            if not has_assertion:
                # Find line number
                line_no = content[:match.start()].count('\n') + 1
                violations.append(
                    Violation(
                        ts_file,
                        line_no,
                        f"Test '{test_name}' has no assertions or expect() calls",
                        "EMPTY_TEST_WITHOUT_ASSERTIONS",
                    )
                )

    return violations, total_ts_tests


def main():
    print("=" * 70)
    print("SYNCIFY: E2E TEST INTEGRITY & ANTI-TAUTOLOGY DETECTOR")
    print("=" * 70)

    rust_violations, total_rust, total_e2e = check_rust_e2e_tests()
    py_violations, total_py = check_python_tests()
    ts_violations, total_ts = check_typescript_tests()

    all_violations = rust_violations + py_violations + ts_violations

    print(f"Rust test files scanned:         {total_rust} ({total_e2e} verified E2E suites)")
    print(f"Python test files scanned:       {total_py}")
    print(f"TypeScript test files scanned:   {total_ts}")
    print(f"Total violations found:          {len(all_violations)}")
    print("-" * 70)

    if all_violations:
        print("VIOLATIONS REPORT:")
        for v in all_violations:
            print(f"  FAILED: {v}")
        print("-" * 70)
        print("RESULT: FAILED (Violations must be remediated before committing)")
        sys.exit(1)
    else:
        print("RESULT: PASSED (All E2E tests and assertion contracts are authentic and valid)")
        sys.exit(0)


if __name__ == "__main__":
    main()
