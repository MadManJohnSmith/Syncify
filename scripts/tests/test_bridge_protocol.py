#!/usr/bin/env python3
"""Contrato de scripts/services/bridge_protocol.py.

El protocolo es la única superficie que comparten los 15 puentes: si cambia,
cambia el contrato con Rust. Estos tests fijan los cuatro desenlaces:

* retorno normal      -> {"success": true, "data": <resultado>}, exit 0
* envelope explícito  -> se emite verbatim (auth/lyrics históricos), exit 0
* BridgeFailure       -> {"success": false, "error": <mensaje>}, exit 1
* excepción inesperada-> traceback a stderr, sin envelope, exit 1
"""

import io
import json
import os
import sys
import unittest
from contextlib import redirect_stdout, redirect_stderr
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.bridge_protocol import BridgeFailure, FailureEnvelope, run, run_cli  # noqa: E402


def _capture(fn, *args, **kwargs):
    """Run fn, return (exit_code, parsed_envelope_or_None, stderr_text)."""
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        code = fn(*args, **kwargs)
    text = out.getvalue()
    envelope = None
    if text.strip():
        assert len(text.strip().splitlines()) == 1, f"unexpected extra lines: {text!r}"
        envelope = json.loads(text)
    return code, envelope, err.getvalue()


class RunModeTests(unittest.TestCase):
    def test_handler_return_is_wrapped_in_data(self):
        code, envelope, _ = _capture(run, lambda args: {"tracks": 3}, argv=[])
        self.assertEqual(code, 0)
        self.assertEqual(envelope, {"success": True, "data": {"tracks": 3}})

    def test_explicit_envelope_is_emitted_verbatim(self):
        payload = {"success": False, "service": "spotify", "message": "deprecated", "native": True}
        code, envelope, _ = _capture(run, lambda args: payload, argv=[])
        self.assertEqual(code, 0)
        self.assertEqual(envelope, payload)

    def test_bridge_failure_becomes_error_envelope_and_exit_1(self):
        def handler(args):
            raise BridgeFailure("No match found")

        code, envelope, _ = _capture(run, handler, argv=[])
        self.assertEqual(code, 1)
        self.assertEqual(envelope, {"success": False, "error": "No match found"})

    def test_unexpected_exception_writes_traceback_and_no_envelope(self):
        def handler(args):
            raise RuntimeError("boom")

        code, envelope, stderr = _capture(run, handler, argv=[])
        self.assertEqual(code, 1)
        self.assertIsNone(envelope, "a bug must not look like a domain failure")
        self.assertIn("RuntimeError: boom", stderr)

    def test_parse_callable_receives_raw_arguments(self):
        seen = {}

        def parse(raw):
            seen["raw"] = list(raw)
            return {"track": raw[0]}

        code, envelope, _ = _capture(run, lambda args: args["track"], parse=parse, argv=["a", "b"])
        self.assertEqual(code, 0)
        self.assertEqual(seen["raw"], ["a", "b"])
        self.assertEqual(envelope, {"success": True, "data": "a"})

    def test_system_exit_from_parser_propagates(self):
        def parse(raw):
            raise SystemExit(2)

        out = io.StringIO()
        with redirect_stdout(out):
            with self.assertRaises(SystemExit) as ctx:
                run(lambda args: None, parse=parse, argv=[])
        self.assertEqual(ctx.exception.code, 2)
        self.assertEqual(out.getvalue(), "")

    def test_non_ascii_is_not_ascii_escaped(self):
        code, envelope, _ = _capture(run, lambda args: {"title": "Café del Mar · 東京"}, argv=[])
        self.assertEqual(code, 0)
        self.assertEqual(envelope["data"]["title"], "Café del Mar · 東京")

    def test_non_ascii_serialization_is_literal_utf8(self):
        out = io.StringIO()
        with redirect_stdout(out):
            run(lambda args: {"title": "Café"}, argv=[])
        self.assertIn("Café", out.getvalue())
        self.assertNotIn("\\u", out.getvalue())


class FailureEnvelopeTests(unittest.TestCase):
    def test_failure_envelope_is_verbatim_and_exits_one(self):
        payload = {"success": False, "data": {"verification_url": "https://link.tidal.com/X"}}
        code, envelope, _ = _capture(run, lambda args: FailureEnvelope(payload), argv=[])
        self.assertEqual(code, 1)
        self.assertEqual(envelope, payload)

    def test_plain_failure_dict_keeps_historic_exit_zero(self):
        payload = {"success": False, "service": "spotify", "message": "deprecated"}
        code, envelope, _ = _capture(run, lambda args: dict(payload), argv=[])
        self.assertEqual(code, 0)
        self.assertEqual(envelope, payload)


class CliModeTests(unittest.TestCase):
    def test_success_returns_zero_and_no_envelope(self):
        code, envelope, _ = _capture(run_cli, lambda args: None, argv=[])
        self.assertEqual(code, 0)
        self.assertIsNone(envelope)

    def test_bridge_failure_prints_error_to_stderr(self):
        def handler(args):
            raise BridgeFailure("database not found")

        code, envelope, stderr = _capture(run_cli, handler, argv=[])
        self.assertEqual(code, 1)
        self.assertIsNone(envelope)
        self.assertIn("ERROR: database not found", stderr)

    def test_unexpected_exception_returns_one(self):
        def handler(args):
            raise RuntimeError("boom")

        code, envelope, stderr = _capture(run_cli, handler, argv=[])
        self.assertEqual(code, 1)
        self.assertIsNone(envelope)
        self.assertIn("RuntimeError: boom", stderr)


if __name__ == "__main__":
    unittest.main()
