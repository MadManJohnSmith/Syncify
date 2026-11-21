#!/usr/bin/env python3
"""Protocolo único de los puentes Python de Syncify.

Todos los puentes hablan el mismo contrato con el núcleo Rust:

    {"success": true,  "data": <resultado>}      # éxito, exit 0
    {"success": false, "error": "<mensaje>"}     # fallo de dominio, exit 1

Un handler puede además devolver un diccionario que ya traiga la clave
``success``: se emite tal cual, sin envolver, con exit 0. Eso conserva los
payloads históricos de auth y lyrics, que mezclan campos extra en el nivel
superior del envelope.

Dos modos de ejecución:

* ``run(handler)``     — modo IPC: envelope JSON por stdout, UTF-8.
* ``run_cli(handler)`` — modo humano: salida legible, errores a stderr.

Las excepciones inesperadas NO producen envelope: el traceback va a stderr y
el proceso sale con 1, igual que un script sin captura de errores. Así Rust
sigue viendo «stdout vacío + exit != 0» para los bugs de los puentes, mientras
que ``BridgeFailure`` sí se convierte en el envelope de fallo.
"""

from __future__ import annotations

import json
import sys
import traceback
from typing import Any, Callable, List, Optional

__all__ = ["BridgeFailure", "FailureEnvelope", "run", "run_cli"]


class BridgeFailure(Exception):
    """Fallo de dominio esperado por el puente.

    ``run`` lo convierte en ``{"success": false, "error": str(exc)}`` con
    exit 1; ``run_cli`` lo imprime como ``ERROR: ...`` en stderr.
    """


class FailureEnvelope(dict):
    """Envelope de fallo con payload propio, emitido verbatim con exit 1.

    Algunos handlers necesitan devolver datos junto al fallo (por ejemplo, la
    URL de verificación del device-code flow de Tidal). Un dict normal con
    ``success`` se emite verbatim con exit 0 —el comportamiento histórico de
    auth—, así que el fallo declara su código de salida con este subclase.
    """


def _force_utf8_stdio() -> None:
    """Fuerza stdio UTF-8 en los modos CLI del bridge.

    En Windows el stdout/stderr capturado es cp1252 por defecto y los
    informes de mantenimiento imprimen banners Unicode (══, ✓): morian con
    UnicodeEncodeError antes de terminar (la ejecucion de scripts desde los
    tests de CI y el runtime embebido de Windows la sufren igual). En Linux
    es un no-op.
    """
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[union-attr]
        except (AttributeError, ValueError):
            # StringIO en tests o stream ya cerrado: se escribe igual.
            pass


def _write_envelope(payload: dict) -> None:
    """Escribe un envelope JSON UTF-8 por stdout, sin escapar no-ASCII."""
    try:
        sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        # StringIO o stdout ya cerrado: el payload se escribe igual.
        pass
    sys.stdout.write(json.dumps(payload, ensure_ascii=False, default=str) + "\n")
    sys.stdout.flush()


def _invoke(
    handler: Callable[[Any], Any],
    parse: Optional[Callable[[List[str]], Any]],
    argv: Optional[List[str]],
) -> Any:
    raw = list(sys.argv[1:] if argv is None else argv)
    args = parse(raw) if parse is not None else raw
    return handler(args)


def run(
    handler: Callable[[Any], Any],
    parse: Optional[Callable[[List[str]], Any]] = None,
    argv: Optional[List[str]] = None,
) -> int:
    """Ejecuta un handler en modo IPC y devuelve el código de salida.

    ``parse`` recibe la lista de argumentos (sin ``argv[0]``) y devuelve lo que
    recibe el handler; por defecto la lista llega tal cual. ``argv`` permite
    inyectar argumentos en tests.
    """
    _force_utf8_stdio()
    try:
        result = _invoke(handler, parse, argv)
    except BridgeFailure as exc:
        _write_envelope({"success": False, "error": str(exc)})
        return 1
    except SystemExit:
        # argparse ya explicó el problema por stderr con su propio código.
        raise
    except Exception:  # noqa: BLE001 - bug del puente: traceback, sin envelope
        traceback.print_exc(file=sys.stderr)
        return 1

    if isinstance(result, dict) and "success" in result:
        _write_envelope(result)
        return 1 if isinstance(result, FailureEnvelope) else 0
    _write_envelope({"success": True, "data": result})
    return 0


def run_cli(
    handler: Callable[[Any], Any],
    parse: Optional[Callable[[List[str]], Any]] = None,
    argv: Optional[List[str]] = None,
) -> int:
    """Ejecuta un handler en modo humano (mantenimiento) y devuelve el código.

    El handler imprime su propio informe legible; este modo solo estandariza el
    parseo de argumentos, el tratamiento de ``BridgeFailure`` y los códigos de
    salida.
    """
    _force_utf8_stdio()
    try:
        _invoke(handler, parse, argv)
        return 0
    except BridgeFailure as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    except SystemExit:
        raise
    except Exception:  # noqa: BLE001
        traceback.print_exc(file=sys.stderr)
        return 1
