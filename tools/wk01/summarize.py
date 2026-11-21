#!/usr/bin/env python3
"""Agrega y compara las corridas de WK-01 (`tools/wk01-composicion.sh`).

Lee los `events.jsonl` y `run.json` que deja el orquestador, calcula la señal de
presentación por ruta y backend, y escribe `results.json` + `resumen.md` en el
directorio de la medición. No toca nada fuera de ese directorio.

La comparación que importa es dentro del MISMO backend: la ruta compatible es
el control. Si la captura de ventana funciona en el backend (la compatible
enseña píxeles), una ventana vacía en la acelerada es una ventana en blanco, no
un fallo de captura. En Wayland la captura de ventana devuelve negro, así que
allí la señal es `after-paint` (frames pintados por GTK) contra la referencia
compatible; el veredicto se marca como «sin prueba de píxel».
"""

import argparse
import json
import os
import statistics
import sys

RED_PIXELS_PRESENT = 300
PAINT_RATIO_PRESENT = 0.5
ERROR_PATTERNS = ("Failed to create GBM buffer", "EGL", "Aborting", "Invalid argument")


def read_events(path):
    events = []
    if not os.path.exists(path):
        return events
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except ValueError:
                events.append({"event": "unparseable", "raw": line[:200]})
    return events


def first(events, name):
    for event in events:
        if event.get("event") == name:
            return event
    return {}


def count_errors(log_path):
    hits = {}
    if not os.path.exists(log_path):
        return hits
    with open(log_path, "r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            for pattern in ERROR_PATTERNS:
                if pattern in line:
                    hits[pattern] = hits.get(pattern, 0) + 1
    return hits


def run_snapshot_ok(snapshot):
    """El contenido se renderiza si el snapshot tiene el bloque de la fixture."""
    return (snapshot.get("red_px") or 0) >= RED_PIXELS_PRESENT


def load_run(run_dir):
    events = read_events(os.path.join(run_dir, "events.jsonl"))
    meta_path = os.path.join(run_dir, "run.json")
    meta = {}
    if os.path.exists(meta_path):
        with open(meta_path, "r", encoding="utf-8") as handle:
            meta = json.load(handle)
    geometry = first(events, "geometry")
    measure = first(events, "measure")
    window_t2 = {}
    for event in events:
        if event.get("event") == "window" and event.get("tag") == "t2":
            window_t2 = event
    snapshot = first(events, "snapshot")
    exit_event = first(events, "exit")

    exit_code = meta.get("exit_code")
    signal_number = None
    if isinstance(exit_code, int) and exit_code > 128:
        signal_number = exit_code - 128
    timed_out = bool(meta.get("timed_out")) or exit_code in (124, 137)

    return {
        "dir": run_dir,
        "backend": meta.get("backend"),
        "route": meta.get("route"),
        "rep": meta.get("rep"),
        "exit_code": exit_code,
        "signal": signal_number,
        "timed_out": timed_out,
        "probe_exit": exit_event.get("code"),
        "ready": measure.get("ready"),
        "hidden": measure.get("hidden"),
        "hardware_acceleration_policy": geometry.get("hardware_acceleration_policy"),
        "viewable": window_t2.get("viewable", geometry.get("viewable")),
        "window_available": window_t2.get("available", False),
        "window_red_px": window_t2.get("red_px"),
        "window_colored_px": window_t2.get("colored_px"),
        "window_uniform": window_t2.get("uniform"),
        "window_changed_px": measure.get("window_changed_px"),
        "snapshot_red_px": snapshot.get("red_px"),
        "content_rendered": bool(run_snapshot_ok(snapshot)),
        "frames_delta": measure.get("frames_delta"),
        "frames_fps": measure.get("fps_from_intervals"),
        "frames_observed_fps": measure.get("frames_observed_fps"),
        "paints_fps": measure.get("paints_fps"),
        "paints_delta": measure.get("paints_delta"),
        "paints_t2": measure.get("paints_t2"),
        "elapsed_s": measure.get("elapsed_s"),
        "errors": count_errors(os.path.join(run_dir, "cmd.log")),
        "loadavg_before": (meta.get("loadavg_before") or [None])[0],
    }


def merge_errors(items):
    merged = {}
    for item in items:
        for key, value in item["errors"].items():
            merged[key] = merged.get(key, 0) + value
    return merged


def median(values):
    values = [value for value in values if isinstance(value, (int, float))]
    if not values:
        return None
    return round(statistics.median(values), 1)


def aggregate(runs):
    """Agrupa por backend/ruta y decide el veredicto con la referencia local."""
    groups = {}
    for run in runs:
        groups.setdefault((run["backend"], run["route"]), []).append(run)

    # ¿La captura de ventana sirve en este backend? La ruta compatible es el
    # control: si ella no enseña píxeles, no hay prueba de píxel disponible.
    pixel_supported = {}
    for (backend, route), items in groups.items():
        if route == "compatible":
            pixel_supported[backend] = any(
                (item["window_red_px"] or 0) >= RED_PIXELS_PRESENT for item in items
            )
    for backend in {key[0] for key in groups}:
        pixel_supported.setdefault(backend, False)

    rows = []
    for (backend, route), items in sorted(groups.items()):
        reference_fps = median(
            [item["paints_fps"] for item in groups.get((backend, "compatible"), [])]
        )
        presents = []
        for item in items:
            if pixel_supported.get(backend):
                ok = (item["window_red_px"] or 0) >= RED_PIXELS_PRESENT and (
                    item["window_changed_px"] or 0
                ) > 0
            else:
                fps = item["paints_fps"]
                ok = bool(
                    item["content_rendered"]
                    and fps is not None
                    and reference_fps
                    and fps >= PAINT_RATIO_PRESENT * reference_fps
                )
            presents.append(bool(ok) and not item["timed_out"])

        failed = [item for item in items if item["exit_code"] not in (0, None)]
        if any(item["timed_out"] for item in items):
            verdict = "sin veredicto (timeout)"
        elif all(presents):
            verdict = "compone" if pixel_supported.get(backend) else "compone (sin prueba de píxel)"
        elif any(presents):
            verdict = "inestable"
        elif failed:
            verdict = "no compone (falla de proceso: %s)" % ",".join(
                str(item["exit_code"]) for item in failed
            )
        else:
            verdict = "no compone"

        rows.append(
            {
                "backend": backend,
                "route": route,
                "runs": len(items),
                "exit_codes": [item["exit_code"] for item in items],
                "signals": [item["signal"] for item in items],
                "timed_out": any(item["timed_out"] for item in items),
                "accel_policy": sorted(
                    {str(item["hardware_acceleration_policy"]) for item in items}
                ),
                "frames_fps": median([item["frames_fps"] for item in items]),
                "paints_fps": median([item["paints_fps"] for item in items]),
                "window_red_px": median([item["window_red_px"] or 0 for item in items]),
                "window_changed_px": median([item["window_changed_px"] or 0 for item in items]),
                "snapshot_red_px": median([item["snapshot_red_px"] or 0 for item in items]),
                "content_rendered": all(item["content_rendered"] for item in items),
                "window_pixels_measurable": pixel_supported.get(backend, False),
                "errors": merge_errors(items),
                "verdict": verdict,
            }
        )
    return rows


def format_table(rows):
    lines = [
        "| backend | ruta | n | salida | frames/s | paints/s | ventana rojo | ventana cambia | snapshot rojo | política | veredicto |",
        "|---|---|---:|---|---:|---:|---:|---:|---:|---|---|",
    ]
    for row in rows:
        lines.append(
            "| {backend} | {route} | {runs} | {codes} | {frames} | {paints} | {red} | {changed} | {snap} | {policy} | **{verdict}** |".format(
                backend=row["backend"],
                route=row["route"],
                runs=row["runs"],
                codes=",".join(str(code) for code in row["exit_codes"]),
                frames="—" if row["frames_fps"] is None else row["frames_fps"],
                paints="—" if row["paints_fps"] is None else row["paints_fps"],
                red=row["window_red_px"],
                changed=row["window_changed_px"],
                snap=row["snapshot_red_px"],
                policy=",".join(row["accel_policy"]),
                verdict=row["verdict"],
            )
        )
    return "\n".join(lines)


def format_detail(runs, out_dir):
    lines = [
        "| directorio | pista | salida | timeout | ready | hidden | frames/s | paints/s | ventana rojo | snapshot rojo | errores |",
        "|---|---|---|---|---|---|---:|---:|---:|---:|---|",
    ]
    for run in runs:
        errors = ", ".join("%s×%d" % (key, value) for key, value in sorted(run["errors"].items()))
        lines.append(
            "| {name} | {route} | {code} | {timeout} | {ready} | {hidden} | {frames} | {paints} | {red} | {snap} | {errors} |".format(
                name=os.path.relpath(run["dir"], out_dir),
                route=run["route"],
                code=run["exit_code"],
                timeout="sí" if run["timed_out"] else "no",
                ready=run["ready"],
                hidden=run["hidden"],
                frames=run["frames_fps"],
                paints=run["paints_fps"],
                red=run["window_red_px"],
                snap=run["snapshot_red_px"],
                errors=errors or "—",
            )
        )
    return "\n".join(lines)


def main(argv):
    parser = argparse.ArgumentParser(description="Agrega las corridas de WK-01")
    parser.add_argument("out_dir", help="directorio de la medición")
    parser.add_argument("--label", default=None)
    args = parser.parse_args(argv)

    run_dirs = []
    for root, _dirs, files in os.walk(args.out_dir):
        if "events.jsonl" in files or "run.json" in files:
            run_dirs.append(root)
    runs = [load_run(path) for path in sorted(run_dirs)]
    runs = [run for run in runs if run["route"]]
    if not runs:
        print("no hay corridas en %s" % args.out_dir, file=sys.stderr)
        return 2

    rows = aggregate(runs)
    table = format_table(rows)
    detail = format_detail(runs, args.out_dir)

    with open(os.path.join(args.out_dir, "results.json"), "w", encoding="utf-8") as handle:
        json.dump({"label": args.label, "routes": rows, "runs": runs}, handle, indent=2)
        handle.write("\n")

    with open(os.path.join(args.out_dir, "resumen.md"), "w", encoding="utf-8") as handle:
        handle.write("# WK-01 · resumen de la medición\n\n")
        if args.label:
            handle.write("Etiqueta: `%s`\n\n" % args.label)
        handle.write(table + "\n\n## Detalle por corrida\n\n" + detail + "\n")

    print(table)
    print()
    print("detalle y results.json en %s" % args.out_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
