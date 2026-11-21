#!/usr/bin/env python3
"""Sonda WK-01: webview mínimo de WebKitGTK que registra frames y píxeles.

Proceso hijo del experimento `tools/wk01-composicion.sh` (#52). No es código de
producto: no se enlaza con la app ni con Tauri. Reproduce la pila real del
webview de Syncify en Linux —GTK 3 + WebKitGTK 4.1, las mismas bibliotecas que
usa el webview de Tauri— y carga una pantalla fija (`wk01/fixture.html`) con un
bloque en movimiento.

Qué mide, por corrida:

* `frames`: ticks de `requestAnimationFrame` del proceso web (producción de
  frames). Un `requestAnimationFrame` vivo NO prueba que algo se pinte.
* `after-paint`: frames pintados por el proceso de UI de GTK sobre la ventana
  (presentación). Es la señal que distingue "el web process anima" de "la
  ventana muestra algo".
* `window`: captura de píxeles de la ventana (X11). Prueba de contenido
  visible: píxeles rojos del bloque y diferencia entre dos capturas.
* `snapshot`: `webkit_web_view_get_snapshot()`, el contenido tal como lo
  renderiza el web process. Si hay snapshot con contenido y la ventana sale
  vacía, el contenido se renderiza pero no se presenta.

Escribe eventos JSON por línea en `--out` y hace `flush` tras cada uno: si el
proceso aborta o lo mata el timeout, la evidencia parcial sobrevive.

Uso (lo normal es invocarlo desde el orquestador):

    /usr/bin/python3 tools/wk01/webkit_probe.py \\
        --fixture tools/wk01/fixture.html --out /ruta/events.jsonl

Requiere PyGObject del sistema (`python3-gobject`/`python-gobject`) con GTK 3 y
WebKit2 4.1; el intérprete por defecto es el del sistema (`/usr/bin/python3`),
no el `.venv` del repo, que no lleva las tipologías de GObject.
"""

import argparse
import json
import os
import signal
import sys
import time
import traceback

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_NO_BINDINGS = 3
EXIT_RUNTIME = 4

DEFAULT_WIDTH = 480
DEFAULT_HEIGHT = 360
DEFAULT_WARMUP = 1.4
DEFAULT_MEASURE = 2.4

RED_MIN = 90
RED_MARGIN = 40
RED_PIXELS_PRESENT = 300


def _log(msg):
    print("wk01-probe: " + msg, file=sys.stderr, flush=True)


def _import_stack():
    try:
        import gi

        gi.require_version("Gtk", "3.0")
        gi.require_version("Gdk", "3.0")
        gi.require_version("WebKit2", "4.1")
        from gi.repository import GLib, Gdk, GdkPixbuf, Gtk, WebKit2
    except (ImportError, ValueError) as exc:  # pragma: no cover - entorno
        _log("faltan tipologías GTK 3 / WebKit2 4.1: %s" % exc)
        _log("usa el python3 del sistema con PyGObject, no el .venv del repo")
        raise SystemExit(EXIT_NO_BINDINGS)
    return GLib, Gdk, GdkPixbuf, Gtk, WebKit2


class Recorder:
    """Escribe eventos JSON por línea, con flush inmediato."""

    def __init__(self, path):
        self.path = path
        self.handle = open(path, "w", encoding="utf-8")
        self.t0 = time.monotonic()

    def emit(self, event, **fields):
        payload = {"event": event, "t_ms": round((time.monotonic() - self.t0) * 1000.0, 1)}
        payload.update(fields)
        self.handle.write(json.dumps(payload, sort_keys=True) + "\n")
        self.handle.flush()

    def close(self):
        self.handle.close()


def pixbuf_stats(pixbuf):
    """Estadísticas baratas de un pixbuf: color medio, píxeles rojos y variedad."""
    if pixbuf is None:
        return {"available": False}
    width = pixbuf.get_width()
    height = pixbuf.get_height()
    stride = pixbuf.get_rowstride()
    channels = pixbuf.get_n_channels()
    data = pixbuf.get_pixels()

    total = width * height
    sums = [0, 0, 0]
    red = 0
    colored = 0
    sampled = set()
    index = 0
    for y in range(height):
        row = data[y * stride : y * stride + width * channels]
        for x in range(width):
            r = row[x * channels]
            g = row[x * channels + 1] if channels > 1 else r
            b = row[x * channels + 2] if channels > 2 else r
            sums[0] += r
            sums[1] += g
            sums[2] += b
            if r > RED_MIN and r - g > RED_MARGIN and r - b > RED_MARGIN:
                red += 1
            if max(r, g, b) - min(r, g, b) > 20:
                colored += 1
            if index % 37 == 0:
                sampled.add((r, g, b))
            index += 1

    return {
        "available": True,
        "width": width,
        "height": height,
        "mean_rgb": [round(value / total, 1) for value in sums],
        "red_px": red,
        "colored_px": colored,
        "sampled_colors": len(sampled),
        "uniform": len(sampled) <= 2,
    }


def changed_pixels(first, second):
    """Cuenta píxeles distintos entre dos capturas del mismo tamaño."""
    if not first or not second:
        return None
    if (first.get_width(), first.get_height()) != (second.get_width(), second.get_height()):
        return None
    width = first.get_width()
    height = first.get_height()
    stride_a = first.get_rowstride()
    stride_b = second.get_rowstride()
    channels = first.get_n_channels()
    if channels != second.get_n_channels():
        return None
    a = first.get_pixels()
    b = second.get_pixels()
    changed = 0
    for y in range(height):
        row_a = a[y * stride_a : y * stride_a + width * channels]
        row_b = b[y * stride_b : y * stride_b + width * channels]
        for x in range(width):
            base = x * channels
            if row_a[base : base + 3] != row_b[base : base + 3]:
                changed += 1
    return changed


def main(argv):
    parser = argparse.ArgumentParser(description="Sonda de composición WebKitGTK (WK-01)")
    parser.add_argument("--fixture", required=True, help="HTML de la pantalla de prueba")
    parser.add_argument("--out", required=True, help="fichero JSONL de eventos")
    parser.add_argument("--png-dir", default=None, help="directorio para capturas PNG")
    parser.add_argument("--width", type=int, default=DEFAULT_WIDTH)
    parser.add_argument("--height", type=int, default=DEFAULT_HEIGHT)
    parser.add_argument("--warmup", type=float, default=DEFAULT_WARMUP)
    parser.add_argument("--measure", type=float, default=DEFAULT_MEASURE)
    args = parser.parse_args(argv)

    GLib, Gdk, GdkPixbuf, Gtk, WebKit2 = _import_stack()

    recorder = Recorder(args.out)
    state = {
        "ready": False,
        "hidden": None,
        "dpr": None,
        "frames_n": 0,
        "frames_at": None,
        "intervals": [],
        "paints": 0,
        "load_events": [],
        "pixbuf_t1": None,
        "pixbuf_t2": None,
        "stats_t1": None,
        "stats_t2": None,
        "snapshot": None,
        "finished": False,
    }

    def stop(code=EXIT_OK):
        if state["finished"]:
            return
        state["finished"] = True
        recorder.emit("exit", code=code)
        recorder.close()
        Gtk.main_quit()

    def on_signal(signum, _frame):
        recorder.emit("signal", signal=signum, name=signal.Signals(signum).name)
        stop(EXIT_OK)

    for sig in (signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, on_signal)

    Gtk.init([])

    recorder.emit(
        "start",
        argv=argv,
        pid=os.getpid(),
        gdk_backend=os.environ.get("GDK_BACKEND"),
        webkit_disable_dmabuf=os.environ.get("WEBKIT_DISABLE_DMABUF_RENDERER"),
        webkit_disable_compositing=os.environ.get("WEBKIT_DISABLE_COMPOSITING_MODE"),
        webkit_dmabuf_disable_gbm=os.environ.get("WEBKIT_DMABUF_RENDERER_DISABLE_GBM"),
        xdg_data_home=os.environ.get("XDG_DATA_HOME"),
        xdg_config_home=os.environ.get("XDG_CONFIG_HOME"),
        xdg_cache_home=os.environ.get("XDG_CACHE_HOME"),
        display=os.environ.get("DISPLAY"),
        wayland_display=os.environ.get("WAYLAND_DISPLAY"),
    )

    manager = WebKit2.UserContentManager()
    manager.register_script_message_handler("wk01")

    def on_message(_manager, js_result):
        try:
            payload = json.loads(js_result.get_js_value().to_string())
        except (ValueError, AttributeError):
            return
        if payload.get("ready"):
            state["ready"] = True
            state["hidden"] = payload.get("hidden")
            state["dpr"] = payload.get("dpr")
            recorder.emit(
                "ready",
                hidden=state["hidden"],
                dpr=state["dpr"],
                scale_factor=window.get_scale_factor() if window.get_window() else None,
            )
            return
        if "n" in payload:
            state["frames_n"] = payload["n"]
            state["frames_at"] = time.monotonic()
            state["intervals"] = payload.get("intervals") or []
            state["hidden"] = payload.get("hidden", state["hidden"])

    manager.connect("script-message-received::wk01", on_message)

    window = Gtk.Window(title="WK-01 composición WebKitGTK")
    window.set_default_size(args.width, args.height)
    # Sin robar foco ni aparecer en la barra de tareas: el experimento puede
    # correr mientras el dueño usa la máquina.
    window.set_focus_on_map(False)
    window.set_skip_taskbar_hint(True)
    window.set_skip_pager_hint(True)

    view = WebKit2.WebView.new_with_user_content_manager(manager)
    settings = view.get_settings()
    window.add(view)
    window.show_all()

    gdk_window = window.get_window()
    if gdk_window is None:  # pragma: no cover - sin display
        recorder.emit("error", stage="window", message="la ventana no tiene GdkWindow")
        stop(EXIT_RUNTIME)
        return EXIT_RUNTIME

    clock = gdk_window.get_frame_clock()

    def on_after_paint(_clock):
        state["paints"] += 1

    clock.connect("after-paint", on_after_paint)
    recorder.emit(
        "geometry",
        width=gdk_window.get_width(),
        height=gdk_window.get_height(),
        scale_factor=window.get_scale_factor(),
        viewable=gdk_window.is_viewable(),
        visible=gdk_window.is_visible(),
        hardware_acceleration_policy=str(
            settings.get_hardware_acceleration_policy().value_nick
        ),
        user_agent=settings.get_user_agent(),
    )

    def on_load_changed(_view, event):
        state["load_events"].append(event.value_nick)

    view.connect("load-changed", on_load_changed)
    view.load_uri(GLib.filename_to_uri(os.path.abspath(args.fixture), None))

    def save_png(pixbuf, tag):
        if pixbuf is None or not args.png_dir:
            return None
        path = os.path.join(args.png_dir, "%s-window.png" % tag)
        pixbuf.savev(path, "png", [], [])
        return path

    def capture_window(tag):
        pixbuf = Gdk.pixbuf_get_from_window(
            gdk_window, 0, 0, gdk_window.get_width(), gdk_window.get_height()
        )
        stats = pixbuf_stats(pixbuf)
        stats["png"] = save_png(pixbuf, tag)
        stats["viewable"] = gdk_window.is_viewable()
        recorder.emit("window", tag=tag, **stats)
        return pixbuf, stats

    def capture_snapshot():
        def done(view_obj, result):
            try:
                surface = view_obj.get_snapshot_finish(result)
            except Exception as exc:  # noqa: BLE001 - la API lanza GError
                recorder.emit("snapshot", available=False, error=str(exc))
                return
            path = None
            if args.png_dir:
                path = os.path.join(args.png_dir, "snapshot.png")
                surface.write_to_png(path)
            stats = {"available": True, "png": path}
            try:
                stats.update(pixbuf_stats(GdkPixbuf.Pixbuf.new_from_file(path)))
            except Exception as exc:  # noqa: BLE001
                stats["error"] = str(exc)
            recorder.emit("snapshot", **stats)

        view.get_snapshot(
            WebKit2.SnapshotRegion.VISIBLE, WebKit2.SnapshotOptions.NONE, None, done
        )

    def after_warmup():
        state["pixbuf_t1"], state["stats_t1"] = capture_window("t1")
        state["paints_t1"] = state["paints"]
        state["frames_t1"] = state["frames_n"]
        state["frames_t1_at"] = time.monotonic()
        GLib.timeout_add(int(args.measure * 1000), after_measure)
        return False

    def after_measure():
        state["pixbuf_t2"], state["stats_t2"] = capture_window("t2")
        state["paints_t2"] = state["paints"]
        state["frames_t2"] = state["frames_n"]
        state["frames_t2_at"] = time.monotonic()
        capture_snapshot()

        changed = changed_pixels(state["pixbuf_t1"], state["pixbuf_t2"])
        intervals = sorted(i for i in state["intervals"] if i > 0)
        median_interval = intervals[len(intervals) // 2] if intervals else None
        elapsed = None
        if state["frames_t1_at"] and state["frames_t2_at"]:
            elapsed = state["frames_t2_at"] - state["frames_t1_at"]
        observed_fps = None
        if elapsed and elapsed > 0:
            observed_fps = (state["frames_t2"] - state["frames_t1"]) / elapsed
        paints_delta = state["paints_t2"] - state["paints_t1"]

        recorder.emit(
            "measure",
            window_changed_px=changed,
            window_px_total=(state["stats_t2"] or {}).get("width", 0)
            * (state["stats_t2"] or {}).get("height", 0),
            frames_t1=state["frames_t1"],
            frames_t2=state["frames_t2"],
            frames_delta=state["frames_t2"] - state["frames_t1"],
            frames_observed_fps=None if observed_fps is None else round(observed_fps, 1),
            intervals_median_ms=median_interval,
            fps_from_intervals=None
            if not median_interval
            else round(1000.0 / median_interval, 1),
            paints_t1=state["paints_t1"],
            paints_t2=state["paints_t2"],
            paints_delta=paints_delta,
            paints_fps=None if not elapsed else round(paints_delta / elapsed, 1),
            elapsed_s=None if not elapsed else round(elapsed, 2),
            hidden=state["hidden"],
            ready=state["ready"],
            load_events=state["load_events"],
            window_content_visible=bool(
                ((state["stats_t2"] or {}).get("red_px") or 0) >= RED_PIXELS_PRESENT
            ),
        )
        GLib.timeout_add(900, lambda: (stop(EXIT_OK), False)[1])
        return False

    GLib.timeout_add(int(args.warmup * 1000), after_warmup)

    # Presupuesto duro: aunque el webview se cuelgue, la sonda sale sola antes
    # de que el `timeout` del orquestador la mate.
    hard_deadline_ms = int((args.warmup + args.measure + 6.0) * 1000)
    GLib.timeout_add(
        hard_deadline_ms,
        lambda: (recorder.emit("error", stage="deadline", message="sonda sin salida"), stop(EXIT_RUNTIME), False)[2],
    )

    try:
        Gtk.main()
    except Exception:  # noqa: BLE001 - queremos la traza en el JSON
        recorder.emit("error", stage="main", traceback=traceback.format_exc())
        return EXIT_RUNTIME
    finally:
        try:
            recorder.close()
        except Exception:  # noqa: BLE001
            pass
    return EXIT_OK


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except SystemExit:
        raise
    except Exception:  # noqa: BLE001 - el orquestador lee el código de salida
        traceback.print_exc()
        sys.exit(EXIT_RUNTIME)
