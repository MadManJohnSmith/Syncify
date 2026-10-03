#!/usr/bin/env python3
"""Capture real Syncify app screenshots for the GitHub frontpage.

The debug build is launched under XWayland and driven with synthetic XTEST
input: keyboard shortcuts navigate between views and ImageMagick grabs the
window by id, so captures are pixel-exact (no decorations, no shadows).
Full context and the manual fallback live in
.agents/skills/syncify-screenshots/SKILL.md.

Prerequisites:
  - An X11/XWayland session (the app is forced to GDK_BACKEND=x11)
  - cargo build -p syncify-tauri                -> target/debug/syncify-tauri
  - python-xlib  (Arch: pip install --user --break-system-packages python-xlib)
  - ImageMagick 7 with X11 support (uses `magick import`)
  - A populated library database (the shots market the real app)

Usage:
  python3 scripts/capture_screenshots.py --list
  python3 scripts/capture_screenshots.py                     # marketing set
  python3 scripts/capture_screenshots.py --only library lyrics
  python3 scripts/capture_screenshots.py --onboarding        # also walk the 4-step wizard
  python3 scripts/capture_screenshots.py --width 1920 --keep-open

NOTE: click coordinates are window-local for the default 1920x1020 window.
If the UI layout changes, re-verify the recipes below and the onboarding walk.
"""

import argparse
import os
import shutil
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APP_BIN = os.path.join(REPO, "target", "debug", "syncify-tauri")
DEFAULT_OUT = os.path.join(REPO, "docs", "screenshots")
WINDOW_NAME = "syncify"
SETTLE = 2.0

# Window-local click targets (default 1920x1020 layout).
TOAST_X = (1891, 878)  # close button of the "Pro tip" toast
# Onboarding wizard walk: (label, local_x, local_y). Captured at the labelled
# step BEFORE clicking, so --onboarding yields services.png and done.png too.
ONBOARDING_WALK = [
    ("welcome: Get Started", 960, 722),
    ("step1 folder: Next", 1258, 755),
    ("step2 services: Next", 1248, 818),
    ("step3 quality: Next", 1248, 782),
    ("step4 import: Next", 1248, 769),
    ("done: Start Using Syncify", 951, 758),
]

# name -> (description, steps). Steps: ("combo", *keys) | ("click", x, y)
#   | ("wait", seconds) | ("shot", filename). Clicks are window-local.
RECIPES = {
    "dashboard": (
        "Hero: library overview, per-service sources, download statistics",
        [("close-toast",), ("wait", 1.5), ("shot", "dashboard.png")],
    ),
    "library": (
        "Full catalog table with covers, service badges and meta scores",
        [("combo", "ctrl", "1"), ("wait", SETTLE), ("shot", "library.png")],
    ),
    "downloads": (
        "Download queue: threads, retries, completed with quality badges",
        [("combo", "ctrl", "2"), ("wait", SETTLE), ("shot", "downloads.png")],
    ),
    "lyrics": (
        "Synced lyrics viewer with the built-in player (open a synced track)",
        [
            ("combo", "ctrl", "4"),
            ("wait", SETTLE),
            ("click", 500, 370),
            ("wait", SETTLE),
            ("shot", "lyrics.png"),
        ],
    ),
}

# Only meaningful while the onboarding wizard is pending (fresh localStorage).
ONBOARDING_SHOTS = {
    "services": "Onboarding step 2: six services Connected",
    "done": "Onboarding final: You're all set!",
}
# Walk-step index (after ONBOARDING_WALK[i] is clicked) -> shot to capture.
ONBOARDING_SHOTS_ON_STEP = {1: "services", 4: "done"}


class X:
    """Minimal Xlib/XTEST driver: focus, warp, click, keys, capture."""

    def __init__(self, display: str):
        try:
            from Xlib import display as xdisplay, X
            from Xlib.ext import xtest
        except ImportError:
            sys.exit(
                "python-xlib is required: "
                "pip install --user --break-system-packages python-xlib"
            )
        self._X = X
        self._xtest = xtest
        self.d = xdisplay.Display(display)
        self.root = self.d.screen().root
        self.win = None
        self.ox = self.oy = self.w = self.h = 0

    def find_window(self, name: str):
        hits = []

        def walk(w):
            try:
                wm = w.get_wm_name()
            except Exception:
                wm = None
            if wm and name in str(wm).lower():
                hits.append(w)
            try:
                for child in w.query_tree().children:
                    walk(child)
            except Exception:
                pass

        walk(self.root)
        return hits[0] if hits else None

    def focus(self):
        self.win.configure(stack_mode=self._X.Above)
        self.win.set_input_focus(self._X.RevertToPointerRoot, self._X.CurrentTime)
        self.d.flush()
        self.d.sync()
        time.sleep(0.4)
        geom = self.win.get_geometry()
        tr = self.win.translate_coords(self.root, 0, 0)
        self.ox, self.oy = tr.x, tr.y
        self.w, self.h = geom.width, geom.height

    def _keycode(self, name: str) -> int:
        from Xlib import XK

        name = {
            "ctrl": "Control_L",
            "alt": "Alt_L",
            "shift": "Shift_L",
            "super": "Super_L",
        }.get(name.lower(), name)
        code = self.d.keysym_to_keycode(XK.string_to_keysym(name))
        if not code:
            sys.exit(f"unknown keysym: {name}")
        return code

    def _press(self, code: int):
        self._xtest.fake_input(self.d, self._X.KeyPress, code)
        self.d.sync()
        self._xtest.fake_input(self.d, self._X.KeyRelease, code)
        self.d.sync()

    def combo(self, *names: str):
        codes = [self._keycode(n) for n in names]
        for code in codes[:-1]:
            self._xtest.fake_input(self.d, self._X.KeyPress, code)
        self.d.sync()
        self._press(codes[-1])
        for code in reversed(codes[:-1]):
            self._xtest.fake_input(self.d, self._X.KeyRelease, code)
        self.d.sync()

    def click(self, lx: int, ly: int):
        # XWayland needs a real XTEST motion (not just a warp) and a beat
        # between warp/press for the webview to register hover + click.
        self.root.warp_pointer(self.ox + lx, self.oy + ly)
        self.d.sync()
        self._xtest.fake_input(self.d, self._X.MotionNotify, x=self.ox + lx, y=self.oy + ly)
        self.d.sync()
        time.sleep(0.3)
        self._xtest.fake_input(self.d, self._X.ButtonPress, 1)
        self.d.sync()
        time.sleep(0.08)
        self._xtest.fake_input(self.d, self._X.ButtonRelease, 1)
        self.d.sync()

    def capture(self, path: str):
        subprocess.run(
            ["magick", "import", "-window", hex(self.win.id), path],
            check=True,
            cwd=REPO,
        )


def run_steps(x: X, steps, tmpdir: str, label: str):
    for step in steps:
        verb = step[0]
        if verb == "combo":
            x.focus()
            x.combo(*step[1:])
            print(f"  [{label}] keys {'+'.join(step[1:])}")
        elif verb == "click":
            x.focus()
            x.click(*step[1:])
            print(f"  [{label}] click {step[1]},{step[2]}")
        elif verb == "close-toast":
            x.focus()
            x.click(*TOAST_X)
            print(f"  [{label}] toast closed (best effort)")
        elif verb == "wait":
            time.sleep(step[1])
        elif verb == "shot":
            x.focus()
            out = os.path.join(tmpdir, step[1])
            x.capture(out)
            print(f"  [{label}] captured {step[1]}")
        else:
            sys.exit(f"unknown step: {step}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--only", nargs="*", choices=sorted(list(RECIPES) + list(ONBOARDING_SHOTS)),
                    help="capture only these shots")
    ap.add_argument("--onboarding", action="store_true",
                    help="walk the 4-step wizard first (fresh data dirs) and "
                         "capture the services/done shots")
    ap.add_argument("--width", type=int, default=1600, help="output width (default 1600)")
    ap.add_argument("--out-dir", default=DEFAULT_OUT)
    ap.add_argument("--display", default=os.environ.get("DISPLAY", ":0"))
    ap.add_argument("--keep-open", action="store_true", help="leave the app running")
    args = ap.parse_args()

    if shutil.which("magick") is None:
        sys.exit("ImageMagick 7 (magick) is required")
    if not os.path.exists(APP_BIN):
        sys.exit(f"missing {APP_BIN} — run: cargo build -p syncify-tauri")

    wanted = args.only if args.only else list(RECIPES)
    if args.onboarding:
        wanted = wanted + [s for s in ONBOARDING_SHOTS if s not in wanted]
    unknown = [w for w in wanted if w not in RECIPES and w not in ONBOARDING_SHOTS]
    if unknown:
        sys.exit(f"unknown shots: {unknown}")

    x = X(args.display)
    launched = None
    win = x.find_window(WINDOW_NAME)
    if win is None:
        print("Launching the app (GDK_BACKEND=x11)...")
        env = dict(os.environ, GDK_BACKEND="x11", WEBKIT_DISABLE_DMABUF_RENDERER="1")
        launched = subprocess.Popen(
            [APP_BIN], cwd=REPO, env=env,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        for _ in range(30):
            time.sleep(1)
            win = x.find_window(WINDOW_NAME)
            if win is not None:
                break
        if win is None:
            launched.terminate()
            sys.exit("Syncify window not found after 30s")
    else:
        print("Reusing a running Syncify instance")

    x.win = win
    x.focus()
    print(f"Window focused: {x.w}x{x.h} at root ({x.ox},{x.oy})")

    tmpdir = "/tmp/syncify-shots"
    shutil.rmtree(tmpdir, ignore_errors=True)
    os.makedirs(tmpdir, exist_ok=True)

    if args.onboarding:
        print("Walking the onboarding wizard...")
        for i, (label, lx, ly) in enumerate(ONBOARDING_WALK):
            x.focus()
            x.click(lx, ly)
            print(f"  [wizard] {label}")
            time.sleep(1.6)
            shot = ONBOARDING_SHOTS_ON_STEP.get(i)
            if shot and shot in wanted:
                x.focus()
                x.capture(os.path.join(tmpdir, f"{shot}.png"))
                print(f"  [wizard] captured {shot}.png")

    for name in wanted:
        if name in ONBOARDING_SHOTS and not args.onboarding:
            print(f"  [{name}] skipped (requires --onboarding)")
            continue
        desc, steps = RECIPES[name]
        print(f"Capturing {name}: {desc}")
        run_steps(x, steps, tmpdir, name)

    if not args.keep_open and launched is not None:
        launched.terminate()
        try:
            launched.wait(timeout=10)
        except subprocess.TimeoutExpired:
            launched.kill()
        print("App closed")

    os.makedirs(args.out_dir, exist_ok=True)
    print(f"\nOptimizing into {args.out_dir}/ (width {args.width}):")
    for name in wanted:
        if name in ONBOARDING_SHOTS and not args.onboarding:
            continue
        src = os.path.join(tmpdir, f"{name}.png")
        if not os.path.exists(src):
            print(f"  {name}: MISSING raw capture, skipped")
            continue
        dst = os.path.join(args.out_dir, f"{name}.png")
        subprocess.run(
            ["magick", src, "-resize", f"{args.width}x", "-strip", dst],
            check=True,
        )
        size = os.path.getsize(dst) // 1024
        print(f"  {dst} ({size} KB)")
    print(
        "\nNext steps: verify each PNG visually, then update the README hero/"
        "gallery if the shot set changed (see .agents/skills/syncify-screenshots)."
    )


if __name__ == "__main__":
    main()
