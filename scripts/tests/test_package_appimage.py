#!/usr/bin/env python3
"""
Behavior tests for the AppImage packaging script (scripts/ci/package_appimage.sh, #243).

The script owns the whole AppImage chain: extract ONCE, inject GStreamer +
Python + resources, delete-and-guard the host poisoning (libwayland, #212
xdg-open), repack ONCE with a checksum-pinned appimagetool.

Every test crosses the script seam: it runs the script as a subprocess with
fake `gst-inspect-1.0`, `appimagetool` and `curl` executables injected via
PATH, a fake AppImage that answers `--appimage-extract`, and synthetic
source trees (gst plugin libs, standalone Python, bridge scripts). No
network, no real appimagetool, no real GStreamer. Assertions are observable
behavior — exit codes, `::error::` diagnostics, what lands in the repacked
tree, how many times each tool ran — never the script's internal text and
never workflow YAML.
"""

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
PACKAGE_SCRIPT = REPO_ROOT / "scripts" / "ci" / "package_appimage.sh"

# El pin de appimagetool: release fija, nunca `continuous`. La URL es parte
# del contrato del script (un build rolling upstream no puede cambiar el
# empaquetado); el checksum se verifica en el test de corrupcion.
PINNED_URL_PART = "/1.9.1/"

# autoaudiosink y autovideosink son DOS ELEMENTOS de la MISMA libreria: es la
# leccion de los nombres de fichero hardcodeados (libgstautoaudiosink.so no
# existe). El fixture usa un nombre que ninguna lista hardcodeada encontraria.
ELEMENT_LIBS = {
    "autoaudiosink": "libgstautosinkreal.so",
    "autovideosink": "libgstautosinkreal.so",
    "playbin": "libgstplayback.so",
    "decodebin": "libgstplayback.so",
    "audioconvert": "libgstaudioconvert.so",
    "audioresample": "libgstaudioresample.so",
    "videoconvert": "libgstvideoconvert.so",
    "typefindfunctions": "libgstcoreelements.so",
}

# Librerias de sinks de runtime que el script busca por nombre en los
# directorios de librerias del sistema (SYNCIFY_GST_LIB_DIRS en los tests),
# mas un señuelo que no debe acabar en el AppImage.
RUNTIME_SINK_LIBS = (
    "libgstalsa.so",
    "libgstpulseaudio.so",
    "libgstpipewire.so",
    "libgstjack.so",
)
DECOY_LIB = "libgstdecoy.so"

FAKE_GST_INSPECT = r"""#!/usr/bin/env python3
import json, os, sys

state = json.load(open(os.environ["SYNCIFY_PKG_FAKE_STATE"], encoding="utf-8"))
log = os.environ["SYNCIFY_PKG_FAKE_LOG"]
args = sys.argv[1:]

def record(entry):
    with open(log, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry) + "\n")

if not args:
    record({"tool": "gst-inspect", "mode": "list"})
    # Formato real de gst-inspect: "plugin:  elemento:  descripcion" (el
    # elemento NO va al principio de la linea).
    for i, el in enumerate(state["registry"]):
        print(f"plugin{i % 3}:  {el}:  fake element for tests")
    sys.exit(0)

el = args[0]
if el not in state["elements"]:
    sys.stderr.write(f"fake gst-inspect: unexpected element: {el}\n")
    sys.exit(5)

counts_path = os.environ["SYNCIFY_PKG_FAKE_COUNTS"]
counts = json.load(open(counts_path, encoding="utf-8"))
counts[el] = counts.get(el, 0) + 1
query = counts[el]
json.dump(counts, open(counts_path, "w", encoding="utf-8"))

filename = state["elements"][el]["filename"]
swap = state.get("swap", {}).get(el)
if swap and query >= swap.get("after", 2):
    filename = swap["filename"]
record({"tool": "gst-inspect", "mode": "element", "element": el, "query": query})
print("Factory Details:")
print("  Rank                     primary")
print("Plugin Details:")
print(f"  Name                     {el}")
# Formato real: "Filename" separado del valor por espacios y SIN dos puntos.
print(f"  Filename                 {filename}")
sys.exit(0)
"""

FAKE_APPIMAGE = r"""#!/usr/bin/env python3
import json, os, shutil, sys

args = sys.argv[1:]
log = os.environ["SYNCIFY_PKG_FAKE_LOG"]
template = os.environ["SYNCIFY_PKG_FAKE_TEMPLATE"]

if args != ["--appimage-extract"]:
    sys.stderr.write("fake appimage: unexpected invocation\n")
    sys.exit(5)

with open(log, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({"tool": "appimage-extract", "cwd": os.getcwd()}) + "\n")

shutil.copytree(template, os.path.join(os.getcwd(), "squashfs-root"))
sys.exit(0)
"""

FAKE_APPIMAGETOOL = r"""#!/usr/bin/env python3
import json, os, shutil, sys

args = sys.argv[1:]
log = os.environ["SYNCIFY_PKG_FAKE_LOG"]
with open(log, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({"tool": "appimagetool", "args": args, "cwd": os.getcwd()}) + "\n")

# Contrato del repack: --no-appstream, el AppDir como "." y salida .new
# absoluta fuera del AppDir. El AppDir empaquetado debe traer AppRun
# ejecutable: el repack conserva AppRun y hooks.
if len(args) != 3 or args[0] != "--no-appstream" or args[1] != ".":
    sys.stderr.write("fake appimagetool: expected [--no-appstream, ., OUTPUT]\n")
    sys.exit(5)
out = args[2]
if not os.path.isabs(out) or not out.endswith(".new"):
    sys.stderr.write("fake appimagetool: output must be an absolute .new path\n")
    sys.exit(5)
apprun = os.path.join(os.getcwd(), "AppRun")
if not (os.path.isfile(apprun) and os.access(apprun, os.X_OK)):
    sys.stderr.write("fake appimagetool: AppDir has no executable AppRun\n")
    sys.exit(4)
# La salida es un fichero regular, como el AppImage real de appimagetool
# (mv no sobreescribe un fichero con un directorio), y el arbol completo que
# se ha reempaquetado queda junto a ella en OUT.contents para que el test
# inspeccione lo empaquetado de verdad.
shutil.copytree(os.getcwd(), out + ".contents")
with open(out, "wb") as fh:
    fh.write(b"appimage-bytes")
sys.exit(0)
"""

FAKE_CURL = r"""#!/usr/bin/env python3
import json, os, shutil, sys

args = sys.argv[1:]
log = os.environ["SYNCIFY_PKG_FAKE_LOG"]
serve = os.environ.get("SYNCIFY_PKG_FAKE_CURL_SERVE")

if "-o" not in args:
    sys.stderr.write("fake curl: expected -o DEST URL\n")
    sys.exit(5)
dest = args[args.index("-o") + 1]
url = args[-1]
with open(log, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({"tool": "curl", "url": url, "dest": dest}) + "\n")

if not serve:
    sys.exit(7)
shutil.copyfile(serve, dest)
sys.exit(0)
"""


class PackageAppImageScriptTest(unittest.TestCase):
    """Ciclo unico, inyecciones, guardas y pin de appimagetool, contra fixtures."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)

        # bin/ trae los fakes de gst-inspect y curl; tool_dir/ trae el de
        # appimagetool, para poder sacar del PATH la copia fake y probar la
        # ruta de descarga pineada.
        self.bin_dir = base / "bin"
        self.bin_dir.mkdir()
        self.tool_dir = base / "toolbin"
        self.tool_dir.mkdir()
        self._install_fake(self.bin_dir / "gst-inspect-1.0", FAKE_GST_INSPECT)
        self._install_fake(self.bin_dir / "curl", FAKE_CURL)
        self._install_fake(self.tool_dir / "appimagetool", FAKE_APPIMAGETOOL)

        self.log_path = base / "invocations.jsonl"
        self.counts_path = base / "gst-query-counts.json"
        self.counts_path.write_text("{}", encoding="utf-8")
        self.state_path = base / "scenario.json"
        self.curl_serve = base / "served-bytes.bin"
        self.curl_serve.write_bytes(b"corrupted-bytes-not-an-appimage")

        # Arbol sintetico que el fake AppImage materializa al extraer.
        self.template = base / "appdir-template"
        self._make_appdir_template()

        # Librerias de GStreamer sinteticas (makeSynthetic system dirs).
        self.gst_libs = base / "gstsys" / "usr" / "lib"
        self._make_gst_fixture()

        # Runtime Python autonomo y arbol de scripts de los puentes.
        self.python_dir = base / "runtime" / "bundle" / "python"
        self._make_python_fixture()
        self.scripts_dir = base / "scripts-src"
        self._make_scripts_fixture()

        # El AppImage de entrada: el fichero que produce `tauri build`
        # (para los tests, un ejecutable que responde a --appimage-extract).
        self.appimage = base / "syncify_0.1.0_amd64.AppImage"
        self._install_fake(self.appimage, FAKE_APPIMAGE)

        self._write_state({})

    def tearDown(self):
        self.tmp.cleanup()

    # ── fixtures ────────────────────────────────────────────────────────

    def _install_fake(self, path: Path, body: str) -> None:
        path.write_text(body, encoding="utf-8")
        path.chmod(0o755)

    def _make_appdir_template(
        self, xdg_open_as_dir: bool = False, wayland_as_dir: bool = False
    ) -> None:
        t = self.template
        if t.exists():
            shutil.rmtree(t)
        (t / "usr/bin").mkdir(parents=True)
        (t / "usr/lib").mkdir(parents=True)
        (t / "usr/share/syncify").mkdir(parents=True)
        self._install_fake(t / "AppRun", '#!/bin/sh\nexec ./usr/bin/syncify-tauri "$@"\n')
        self._install_fake(t / "usr/bin/syncify-tauri", "#!/bin/sh\ntrue\n")
        xdg = t / "usr/bin/xdg-open"
        if xdg_open_as_dir:
            xdg.mkdir()
        else:
            self._install_fake(xdg, "#!/bin/sh\nexit 0\n")
        wayland_client = t / "usr/lib/libwayland-client.so.0"
        if wayland_as_dir:
            wayland_client.mkdir()
        else:
            wayland_client.write_text("wayland-1.20", encoding="utf-8")
        (t / "usr/lib/libwayland-cursor.so.0").write_text("wayland-1.20", encoding="utf-8")
        # Una libreria inocente que linuxdeploy arrastro y debe sobrevivir.
        (t / "usr/lib/libwebkit2gtk-4.1.so.0").write_text("webkit", encoding="utf-8")
        # Recurso con permisos restrictivos, como los que Tauri empaqueta.
        (t / "usr/share/syncify/seed.dat").write_text("seed", encoding="utf-8")
        (t / "usr/share/syncify/seed.dat").chmod(0o600)
        (t / "usr/share/syncify").chmod(0o700)
        (t / ".DirIcon").write_text("icon", encoding="utf-8")
        (t / "syncify.desktop").write_text("[Desktop Entry]\n", encoding="utf-8")

    def _make_gst_fixture(self) -> None:
        plug = self.gst_libs / "gstreamer-1.0"
        plug.mkdir(parents=True)
        for name in (*ELEMENT_LIBS.values(), *RUNTIME_SINK_LIBS, DECOY_LIB):
            (plug / name).write_text("gst", encoding="utf-8")

    def _make_python_fixture(self) -> None:
        (self.python_dir / "bin").mkdir(parents=True)
        exe = self.python_dir / "bin" / "python3"
        exe.write_text("#!/bin/sh\ntrue\n", encoding="utf-8")
        exe.chmod(0o755)
        (self.python_dir / "lib").mkdir()
        (self.python_dir / "lib" / "libpython3.11.so.1.0").write_text("py", encoding="utf-8")

    def _make_scripts_fixture(self) -> None:
        s = self.scripts_dir
        s.mkdir()
        for name in (
            "auth_bridge.py",
            "conversion_bridge.py",
            "download_bridge.py",
            "dependency_manager.py",
        ):
            (s / name).write_text(f"# {name}\n", encoding="utf-8")
        (s / "requirements.txt").write_text("requests\n", encoding="utf-8")
        services = s / "services"
        (services / "__pycache__").mkdir(parents=True)
        (services / "__init__.py").write_text("", encoding="utf-8")
        (services / "qobuz_service.py").write_text("# qobuz\n", encoding="utf-8")
        (services / "__pycache__" / "junk.cpython-311.pyc").write_text("pyc", encoding="utf-8")

    # ── harness ─────────────────────────────────────────────────────────

    def _write_state(self, updates: dict) -> None:
        state = {
            "registry": list(ELEMENT_LIBS),
            "elements": {
                el: {"filename": str(self.gst_libs / "gstreamer-1.0" / lib)}
                for el, lib in ELEMENT_LIBS.items()
            },
        }
        state.update(updates)
        self.state_path.write_text(json.dumps(state), encoding="utf-8")

    def _run(self, *extra_args: str, path_tool: str = "present",
             cwd: str | None = None) -> subprocess.CompletedProcess:
        """path_tool: 'present' (fake en PATH + opt-in), 'opt-in' (fake en PATH
        sin bandera) o 'absent' (sin fake en PATH ni bandera)."""
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith("SYNCIFY_")
        }
        path = f"{self.bin_dir}{os.pathsep}{env.get('PATH', '')}"
        if path_tool in ("present", "opt-in"):
            path = f"{self.tool_dir}{os.pathsep}{path}"
        if path_tool == "present":
            # Seam explicito del script: sin la bandera, el PATH no participa
            # y el script descargaria el pineado.
            env["SYNCIFY_APPIMAGETOOL_FROM_PATH"] = "1"
        env["PATH"] = path
        env["SYNCIFY_PKG_FAKE_STATE"] = str(self.state_path)
        env["SYNCIFY_PKG_FAKE_LOG"] = str(self.log_path)
        env["SYNCIFY_PKG_FAKE_COUNTS"] = str(self.counts_path)
        env["SYNCIFY_PKG_FAKE_TEMPLATE"] = str(self.template)
        env["SYNCIFY_PKG_FAKE_CURL_SERVE"] = str(self.curl_serve)
        env["SYNCIFY_GST_LIB_DIRS"] = str(self.gst_libs)
        return subprocess.run(
            [
                "bash", str(PACKAGE_SCRIPT),
                "--appimage", str(self.appimage),
                "--python-dir", str(self.python_dir),
                "--scripts-dir", str(self.scripts_dir),
                *extra_args,
            ],
            capture_output=True,
            text=True,
            env=env,
            cwd=cwd or self.tmp.name,
            timeout=120,
        )

    def _run_repo_defaults(self, repo: Path) -> subprocess.CompletedProcess:
        """La forma exacta de la llamada del workflow: sin argumentos."""
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith("SYNCIFY_")
        }
        env["PATH"] = f"{self.tool_dir}{os.pathsep}{self.bin_dir}{os.pathsep}{env.get('PATH', '')}"
        env["SYNCIFY_APPIMAGETOOL_FROM_PATH"] = "1"
        for key, value in (
            ("SYNCIFY_PKG_FAKE_STATE", str(self.state_path)),
            ("SYNCIFY_PKG_FAKE_LOG", str(self.log_path)),
            ("SYNCIFY_PKG_FAKE_COUNTS", str(self.counts_path)),
            ("SYNCIFY_PKG_FAKE_TEMPLATE", str(self.template)),
            ("SYNCIFY_PKG_FAKE_CURL_SERVE", str(self.curl_serve)),
        ):
            env[key] = value
        return subprocess.run(
            ["bash", "scripts/ci/package_appimage.sh"],
            capture_output=True,
            text=True,
            env=env,
            cwd=repo,
            timeout=120,
        )

    def _invocations(self, tool: str) -> list[dict]:
        if not self.log_path.exists():
            return []
        return [
            json.loads(line)
            for line in self.log_path.read_text(encoding="utf-8").splitlines()
            if line.strip() and json.loads(line)["tool"] == tool
        ]

    def _packed(self, appimage: Path | None = None) -> Path:
        """El arbol que el fake appimagetool reempaquetó (OUT.contents)."""
        return Path(str(appimage or self.appimage) + ".new.contents")

    def _assert_rejected_before_repack(
        self, proc: subprocess.CompletedProcess, needle: str | None = None
    ) -> None:
        output = proc.stdout + proc.stderr
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        if needle:
            self.assertIn(needle, output)
        self.assertEqual(
            self._invocations("appimagetool"), [],
            "un AppDir roto no se llega a reempaquetar",
        )

    # ── ciclo unico y repack en sitio ───────────────────────────────────

    def test_single_cycle_repacks_in_place_with_no_download(self):
        proc = self._run()
        output = proc.stdout + proc.stderr
        self.assertEqual(proc.returncode, 0, output)
        # UN extract, UN repack, cero descargas (el appimagetool viene del
        # PATH; los dos ciclos y las dos descargas del workflow anterior
        # murieron aqui).
        self.assertEqual(len(self._invocations("appimage-extract")), 1)
        tools = self._invocations("appimagetool")
        self.assertEqual(len(tools), 1)
        self.assertEqual(self._invocations("curl"), [])
        # Repack en sitio: la ruta original vuelve a ser el artefacto final y
        # el .new intermedio no queda tirado.
        self.assertTrue(self.appimage.is_file(), output)
        self.assertFalse(Path(str(self.appimage) + ".new").exists(), output)
        # El AppRun (y con el, hooks y arbol) llego al paquete reempaquetado.
        self.assertTrue((self._packed() / "AppRun").is_file(), output)
        # El AppDir reempaquetado se limpio junto con el WORK del script.
        self.assertFalse(Path(tools[0]["cwd"]).exists())

    def test_workflow_call_shape_discovers_artifact_with_defaults(self):
        # La llamada del workflow es `bash scripts/ci/package_appimage.sh`
        # desde la raiz del repo: defaults de bundle/python y scripts/, y
        # descubrimiento del AppImage bajo target/release/bundle/appimage.
        repo = Path(self.tmp.name) / "fakerepo"
        (repo / "scripts/ci").mkdir(parents=True)
        shutil.copyfile(PACKAGE_SCRIPT, repo / "scripts/ci/package_appimage.sh")
        bundle = repo / "target/release/bundle/appimage"
        bundle.mkdir(parents=True)
        shutil.copy(self.appimage, bundle / self.appimage.name)  # conserva el +x
        shutil.copytree(self.python_dir, repo / "bundle/python")
        # Los puentes viven en <repo>/scripts, junto al ci/ real.
        for name in (
            "auth_bridge.py",
            "download_bridge.py",
            "dependency_manager.py",
            "requirements.txt",
        ):
            shutil.copyfile(self.scripts_dir / name, repo / "scripts" / name)
        shutil.copytree(self.scripts_dir / "services", repo / "scripts/services")

        proc = self._run_repo_defaults(repo)
        output = proc.stdout + proc.stderr
        self.assertEqual(proc.returncode, 0, output)
        self.assertEqual(len(self._invocations("appimage-extract")), 1)
        self.assertEqual(len(self._invocations("appimagetool")), 1)
        # Descubierto y reempaquetado en sitio.
        self.assertTrue((bundle / self.appimage.name).is_file(), output)
        self.assertTrue((self._packed(bundle / self.appimage.name) / "AppRun").is_file())

    # ── inyeccion de GStreamer ──────────────────────────────────────────

    def test_gst_libs_located_by_capacity_not_filename(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        plug = self._packed() / "usr/lib/gstreamer-1.0"
        # La libreria que gst-inspect REPORTA para los dos sinks (una sola,
        # nombre que ninguna lista hardcodeada encontraria) llego al AppImage.
        self.assertTrue((plug / "libgstautosinkreal.so").is_file())
        self.assertTrue((plug / "libgstplayback.so").is_file())
        self.assertTrue((plug / "libgstaudioconvert.so").is_file())
        self.assertTrue((plug / "libgstaudioresample.so").is_file())
        self.assertTrue((plug / "libgstvideoconvert.so").is_file())
        self.assertTrue((plug / "libgstcoreelements.so").is_file())
        # Los nombres que una version anterior asumio y no existen jamas.
        self.assertFalse((plug / "libgstautoaudiosink.so").exists())
        self.assertFalse((plug / "libgstautovideosink.so").exists())
        self.assertFalse((plug / "libgstautosink.so").exists())
        # El señuelo del arbol de sistema no se copia entero.
        self.assertFalse((plug / DECOY_LIB).exists())

    def test_runtime_sink_libs_comes_from_lib_dirs(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        plug = self._packed() / "usr/lib/gstreamer-1.0"
        for name in RUNTIME_SINK_LIBS:
            self.assertTrue((plug / name).is_file(), name)

    # ── inyeccion de Python y recursos ──────────────────────────────────

    def test_python_runtime_and_scripts_are_injected(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        resources = self._packed() / "resources"
        # candidate 0 de get_python_executable: resources/python/bin/python.
        self.assertTrue((resources / "python/bin/python3").is_file())
        self.assertTrue((resources / "python/lib/libpython3.11.so.1.0").is_file())
        # marcador de get_project_root: resources/scripts.
        for name in (
            "auth_bridge.py",
            "download_bridge.py",
            "dependency_manager.py",
            "requirements.txt",
        ):
            self.assertTrue((resources / "scripts" / name).is_file(), name)
        self.assertTrue((resources / "scripts/services/qobuz_service.py").is_file())
        self.assertFalse((resources / "scripts/services/__pycache__").exists())

    # ── borrado con guarda del envenenamiento al anfitrion ──────────────

    def test_wayland_and_xdg_open_poison_is_removed(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        packed = self._packed()
        self.assertFalse(
            list((packed / "usr/lib").glob("libwayland-*.so*")),
            "las libwayland empaquetadas envenenan el EGL del anfitrion",
        )
        self.assertFalse(
            (packed / "usr/bin/xdg-open").exists(),
            "el xdg-open empaquetado ensombreceria al del anfitrion (#212)",
        )
        # Lo inocente sobrevive.
        self.assertTrue((packed / "usr/lib/libwebkit2gtk-4.1.so.0").is_file())
        self.assertTrue((packed / "usr/bin/syncify-tauri").is_file())

    def test_restrictive_permissions_are_widened_before_repack(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        seed = self._packed() / "usr/share/syncify/seed.dat"
        self.assertEqual(seed.stat().st_mode & 0o444, 0o444)
        directory = self._packed() / "usr/share/syncify"
        self.assertEqual(directory.stat().st_mode & 0o555, 0o555)

    # ── guardas de fallo explicito ──────────────────────────────────────

    def test_missing_required_element_fails_before_repack(self):
        self._write_state({
            "registry": [el for el in ELEMENT_LIBS if el != "audioconvert"],
        })
        proc = self._run()
        self._assert_rejected_before_repack(proc, "audioconvert")

    def test_nonexistent_filename_report_fails_before_repack(self):
        self._write_state({
            "elements": {
                **{el: {"filename": str(self.gst_libs / "gstreamer-1.0" / lib)}
                   for el, lib in ELEMENT_LIBS.items()},
                "playbin": {"filename": str(self.tmp.name + "/ghost/libgstplaybin.so")},
            },
        })
        proc = self._run()
        self._assert_rejected_before_repack(proc, "playbin")

    def test_sink_lib_not_landed_fails_before_repack(self):
        # El sink se copia con la libreria de la primera consulta, pero la
        # verificacion final (por capacidad) pregunta otra vez: si lo que
        # reporta gst-inspect no esta en el AppDir, no se publica.
        ghost = Path(self.tmp.name) / "ghost" / "libgstghost.so"
        self._write_state({"swap": {"autoaudiosink": {"after": 2, "filename": str(ghost)}}})
        proc = self._run()
        self._assert_rejected_before_repack(proc, "autoaudiosink")

    def test_missing_appimage_fails(self):
        proc = self._run("--appimage", str(Path(self.tmp.name) / "nope.AppImage"))
        self._assert_rejected_before_repack(proc, "nope.AppImage")

    def test_missing_python_dir_fails(self):
        proc = self._run("--python-dir", str(Path(self.tmp.name) / "no-bundle"))
        self._assert_rejected_before_repack(proc, "install-runtime")

    def test_missing_scripts_dir_fails(self):
        proc = self._run("--scripts-dir", str(Path(self.tmp.name) / "no-scripts"))
        self._assert_rejected_before_repack(proc, "arbol de scripts")

    def test_xdg_open_directory_aborts_before_repack(self):
        # Corrupcion del AppDir: el borrado no puede quitarlo y la guarda lo
        # convierte en fallo explicito; no se publica con #212 dentro.
        self._make_appdir_template(xdg_open_as_dir=True)
        proc = self._run()
        self._assert_rejected_before_repack(proc, "#212")

    def test_wayland_directory_aborts_before_repack(self):
        # Misma guarda para el envenenamiento EGL de las libwayland.
        self._make_appdir_template(wayland_as_dir=True)
        proc = self._run()
        self._assert_rejected_before_repack(proc, "libwayland")

    # ── pin de appimagetool ─────────────────────────────────────────────

    def test_checksum_mismatch_aborts_without_executing(self):
        # Sin appimagetool en el PATH, el script descarga el pineado; los
        # bytes servidos no cuadran con el checksum: no se ejecuta nada y no
        # se toca el AppImage original.
        proc = self._run(path_tool="absent")
        output = proc.stdout + proc.stderr
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("SHA-256", output)
        downloads = self._invocations("curl")
        self.assertEqual(len(downloads), 1, output)
        self.assertIn(PINNED_URL_PART, downloads[0]["url"])
        self.assertNotIn("continuous", downloads[0]["url"])
        self.assertEqual(self._invocations("appimagetool"), [])
        self.assertTrue(self.appimage.is_file(), "el AppImage original queda intacto")

    def test_download_failure_of_pinned_tool_fails_explicitly(self):
        # Sin appimagetool en el PATH y sin contenido que servir: la descarga
        # del pineado muere con diagnostico, nunca con un binario cualquiera
        # del sistema.
        self.curl_serve = Path(self.tmp.name) / "does-not-exist"
        proc = self._run(path_tool="absent")
        output = proc.stdout + proc.stderr
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("descarga", output)
        downloads = self._invocations("curl")
        self.assertEqual(len(downloads), 1, output)
        self.assertIn(PINNED_URL_PART, downloads[0]["url"])
        self.assertNotIn("continuous", downloads[0]["url"])
        self.assertEqual(self._invocations("appimagetool"), [])

    def test_path_tool_is_ignored_without_explicit_opt_in(self):
        # Hay un appimagetool en el PATH, pero sin la bandera el script no
        # puede usarlo: intenta la descarga pineada y con bytes corruptos
        # aborta. Nadie puede colar un binario distinto en CI por el PATH.
        proc = self._run(path_tool="opt-in")
        output = proc.stdout + proc.stderr
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertEqual(len(self._invocations("curl")), 1, output)
        self.assertEqual(self._invocations("appimagetool"), [])


if __name__ == "__main__":
    unittest.main()
