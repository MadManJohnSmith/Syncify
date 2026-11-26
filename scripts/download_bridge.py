#!/usr/bin/env python3
"""download_bridge.py — retirado (D14, #126).

La descarga es nativa en Rust (ADR-0009): Qobuz y Tidal tienen adaptador propio
y Deezer/SoundCloud se sirven por el gateway de proveedores. Este CLI y las
implementaciones `download_track` de `scripts/services/` se retiraron para no
mantener un segundo motor de descarga que Rust nunca invocó.

El fichero sobrevive como tombstone porque dos suites Rust siguen exigiendo su
presencia y, una de ellas, su contrato histórico de override por entorno:

- `src-tauri/tests/python_module_hygiene_test.rs`
  (`test_legitimate_production_bridges_exist`).
- `src-tauri/tests/qobuz_credentials_leak_test.rs`
  (`test_qobuz_bridges_keep_env_override_contract`, que busca las cadenas
  `QOBUZ_APP_ID` y `QOBUZ_APP_SECRET`).

Retirarlo del árbol exige actualizar `src-tauri/**` en el mismo cambio; esta
rama es solo Python/docs. Mientras exista, cualquier invocación responde con el
error de retirada y exit 1, sin importar servicios ni descargar nada.

Variables históricas de override (las sigue leyendo el cliente nativo):
`QOBUZ_APP_ID`, `QOBUZ_APP_SECRET`, `TIDAL_ACCESS_TOKEN`, `DEEZER_ARL` y
`SOUNDCLOUD_AUTH_TOKEN`.
"""

import json
import sys

RETIRED_ERROR = (
    "download_bridge.py está retirado (D14, #126): la descarga es nativa en "
    "Rust (ADR-0009). Usa la cola de descargas de la app."
)


def main() -> int:
    print(json.dumps({"success": False, "error": RETIRED_ERROR}, ensure_ascii=False))
    return 1


if __name__ == "__main__":
    sys.exit(main())
