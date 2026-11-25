//! Hijo GTK/WebKitGTK: ejecuta la pila real en un perfil y grupo aislados.
//!
//! La fixture es la misma pantalla de WK-01: fondo oscuro, bandas de color y un
//! bloque rojo en movimiento. `requestAnimationFrame` mide producción del web
//! process; `after-paint` mide pintado de GTK; solo los píxeles de la ventana X11
//! prueban presentación. En Wayland el snapshot queda como control de render,
//! nunca como prueba de píxeles visibles.

use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use javascriptcore::ValueExt;
use serde::Deserialize;
use webkit2gtk::{
    HardwareAccelerationPolicy, JavascriptResult, LoadEvent, SettingsExt, SnapshotOptions,
    SnapshotRegion, UserContentManagerExt, WebViewExt,
};

use super::report::{
    Backend, PixelEvidence, ProbeEvidence, PROBE_CONTRACT_VERSION, PROBE_FIXTURE_ID,
};
use super::{changed_pixels, pixel_stats, ChildRequest};

const WIDTH: i32 = 480;
const HEIGHT: i32 = 360;
const WARMUP: Duration = Duration::from_millis(1400);
const MEASURE: Duration = Duration::from_millis(2400);
const CHILD_DEADLINE: Duration = Duration::from_millis(6000);
const HANDLER: &str = "syncifyProbe";

const FIXTURE: &str = r##"<!doctype html>
<html lang="es">
<head><meta charset="utf-8"><style>
html, body { margin: 0; padding: 0; background: #101820; overflow: hidden; }
canvas { display: block; }
</style></head>
<body><canvas id="stage"></canvas><script>
(function () {
  "use strict";
  const W = 480, H = 360;
  const stage = document.getElementById("stage");
  stage.width = W; stage.height = H;
  const ctx = stage.getContext("2d");
  const post = (payload) => {
    try { window.webkit.messageHandlers.syncifyProbe.postMessage(JSON.stringify(payload)); }
    catch (_) { /* el puente solo existe en la sonda */ }
  };
  let n = 0;
  function frame() {
    n += 1;
    ctx.fillStyle = "#101820"; ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = "#16425b"; ctx.fillRect(0, 0, W, 40);
    ctx.fillStyle = "#1f7a8c"; ctx.fillRect(0, H - 40, W, 40);
    ctx.fillStyle = "#e04040"; ctx.fillRect((n * 5) % (W - 60), 120, 60, 60);
    ctx.fillStyle = "#f2f4f8"; ctx.font = "20px sans-serif";
    ctx.fillText("WK-02 frame " + n, 16, 28);
    post({ n: n, hidden: document.hidden });
    requestAnimationFrame(frame);
  }
  post({ ready: true, hidden: document.hidden });
  requestAnimationFrame(frame);
})();
</script></body></html>"##;

#[derive(Debug, Deserialize, Default)]
struct Message {
    ready: Option<bool>,
    hidden: Option<bool>,
    n: Option<u64>,
}

struct Capture {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    channels: usize,
    stride: usize,
    evidence: PixelEvidence,
}

impl Capture {
    fn from_pixbuf(pixbuf: &Pixbuf) -> Option<Self> {
        let width = u32::try_from(pixbuf.width()).ok()?;
        let height = u32::try_from(pixbuf.height()).ok()?;
        let channels = usize::try_from(pixbuf.n_channels()).ok()?;
        let stride = usize::try_from(pixbuf.rowstride()).ok()?;
        let row_bytes = width as usize * channels;
        let length = height as usize * stride;
        if width == 0 || height == 0 || !(3..=4).contains(&channels) || stride < row_bytes {
            return None;
        }
        // GDK Pixbuf expone su búfer propio; se copia sin mutarlo y el búfer
        // efímero no se comparte con callbacks de GTK.
        let bytes = unsafe { pixbuf.pixels() }.get(..length)?.to_vec();
        let evidence = pixel_stats(&bytes, width, height, channels, stride, 0, 1, 2);
        evidence.available.then_some(Self {
            bytes,
            width,
            height,
            channels,
            stride,
            evidence,
        })
    }
}

struct ProbeState {
    requested_backend: Backend,
    observed_backend: Option<Backend>,
    ready: bool,
    hidden: Option<bool>,
    frames: u64,
    paints: u64,
    frames_at_t1: u64,
    paints_at_t1: u64,
    first_capture: Option<Capture>,
    window: PixelEvidence,
    snapshot: PixelEvidence,
    acceleration_policy: String,
    runtime_errors: Vec<String>,
    finished: bool,
}

impl ProbeState {
    fn new(requested_backend: Backend, observed_backend: Option<Backend>) -> Self {
        Self {
            requested_backend,
            observed_backend,
            ready: false,
            hidden: None,
            frames: 0,
            paints: 0,
            frames_at_t1: 0,
            paints_at_t1: 0,
            first_capture: None,
            window: PixelEvidence::default(),
            snapshot: PixelEvidence::default(),
            acceleration_policy: "unknown".into(),
            runtime_errors: Vec::new(),
            finished: false,
        }
    }

    fn push_error(&mut self, error: String) {
        if self.runtime_errors.len() < 8 && !self.runtime_errors.contains(&error) {
            self.runtime_errors.push(error);
        }
    }

    fn evidence(&self, viewable: bool) -> ProbeEvidence {
        ProbeEvidence {
            schema_version: PROBE_CONTRACT_VERSION,
            fixture_id: PROBE_FIXTURE_ID.into(),
            requested_backend: self.requested_backend,
            observed_backend: self.observed_backend,
            ready: self.ready,
            hidden: self.hidden,
            viewable,
            frames_delta: self.frames.saturating_sub(self.frames_at_t1),
            paints_delta: self.paints.saturating_sub(self.paints_at_t1),
            window: self.window.clone(),
            window_red_px_t1: self
                .first_capture
                .as_ref()
                .map(|capture| capture.evidence.red_px)
                .unwrap_or(0),
            snapshot: self.snapshot.clone(),
            acceleration_policy: self.acceleration_policy.clone(),
            runtime_errors: self.runtime_errors.clone(),
        }
    }
}

fn observed_backend(display: &gtk::gdk::Display) -> Option<Backend> {
    match display.backend() {
        gtk::gdk::Backend::X11 => Some(Backend::X11),
        gtk::gdk::Backend::Wayland => Some(Backend::Wayland),
        _ => None,
    }
}

fn write_evidence(path: &Path, evidence: &ProbeEvidence) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer(&mut file, evidence)?;
    file.sync_all()
}

fn capture_window(window: &gtk::gdk::Window) -> Result<Capture, String> {
    let width = window.width();
    let height = window.height();
    if width <= 0 || height <= 0 {
        return Err("x11-window-geometry-unavailable".into());
    }
    let pixbuf = window
        .pixbuf(0, 0, width, height)
        .ok_or_else(|| "x11-window-capture-unavailable".to_string())?;
    Capture::from_pixbuf(&pixbuf).ok_or_else(|| "x11-window-pixels-invalid".to_string())
}

fn finish(
    result_path: &Path,
    window: &gtk::gdk::Window,
    state: &Rc<RefCell<ProbeState>>,
    exit_code: &Rc<Cell<i32>>,
) {
    let viewable = window.is_viewable();
    let mut state = state.borrow_mut();
    if state.finished {
        return;
    }
    state.finished = true;
    let evidence = state.evidence(viewable);
    drop(state);
    match write_evidence(result_path, &evidence) {
        Ok(()) => exit_code.set(0),
        Err(error) => {
            eprintln!("syncify-webkit-probe: no se pudo escribir evidencia: {error}");
            exit_code.set(4);
        }
    }
    gtk::main_quit();
}

fn snapshot_pixels(
    surface: &gtk::cairo::Surface,
    width: i32,
    height: i32,
) -> Result<PixelEvidence, String> {
    if width <= 0 || height <= 0 {
        return Err("snapshot-geometry-unavailable".into());
    }
    let pixbuf = gtk::gdk::pixbuf_get_from_surface(surface, 0, 0, width, height)
        .ok_or_else(|| "snapshot-capture-unavailable".to_string())?;
    Capture::from_pixbuf(&pixbuf)
        .map(|capture| capture.evidence)
        .ok_or_else(|| "snapshot-pixels-invalid".to_string())
}

pub(super) fn run_child(request: &ChildRequest) -> i32 {
    // Este modo existe para medir la ruta acelerada. No hereda ni los defaults
    // compatibles de la app ni un override previo del operador.
    for variable in [
        "WEBKIT_DISABLE_DMABUF_RENDERER",
        "WEBKIT_DISABLE_COMPOSITING_MODE",
        "WEBKIT_DMABUF_RENDERER_DISABLE_GBM",
    ] {
        std::env::remove_var(variable);
    }

    if let Err(error) = gtk::init() {
        eprintln!("syncify-webkit-probe: GTK init falló: {error}");
        return 4;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        eprintln!("syncify-webkit-probe: sin display GDK");
        return 4;
    };
    let observed = observed_backend(&display);
    let state = Rc::new(RefCell::new(ProbeState::new(request.backend, observed)));
    if observed != Some(request.backend) {
        state.borrow_mut().push_error("backend-mismatch".into());
        let evidence = state.borrow().evidence(false);
        return match write_evidence(&request.result_path, &evidence) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("syncify-webkit-probe: no se pudo escribir evidencia: {error}");
                4
            }
        };
    }

    let manager = webkit2gtk::UserContentManager::new();
    if !manager.register_script_message_handler(HANDLER) {
        eprintln!("syncify-webkit-probe: no se pudo registrar el puente JavaScript");
        return 4;
    }
    let message_state = Rc::clone(&state);
    manager.connect_script_message_received(Some(HANDLER), move |_, result: &JavascriptResult| {
        let Some(value) = result.js_value() else {
            message_state
                .borrow_mut()
                .push_error("javascript-value-missing".into());
            return;
        };
        let Ok(message) = serde_json::from_str::<Message>(&value.to_str()) else {
            message_state
                .borrow_mut()
                .push_error("javascript-message-invalid".into());
            return;
        };
        let mut state = message_state.borrow_mut();
        if let Some(ready) = message.ready {
            state.ready = ready;
        }
        if let Some(hidden) = message.hidden {
            state.hidden = Some(hidden);
        }
        if let Some(frames) = message.n {
            state.frames = state.frames.max(frames);
        }
    });

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Syncify WebKitGTK startup probe");
    window.set_default_size(WIDTH, HEIGHT);
    window.set_resizable(false);
    window.set_focus_on_map(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    let view = webkit2gtk::WebView::with_user_content_manager(&manager);
    let Some(settings) = WebViewExt::settings(&view) else {
        eprintln!("syncify-webkit-probe: WebKitGTK no expone settings");
        return 4;
    };
    settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);
    settings.set_enable_developer_extras(false);
    settings.set_enable_write_console_messages_to_stdout(false);
    state.borrow_mut().acceleration_policy = match settings.hardware_acceleration_policy() {
        HardwareAccelerationPolicy::Always => "always".into(),
        HardwareAccelerationPolicy::Never => "never".into(),
        HardwareAccelerationPolicy::OnDemand => "on-demand".into(),
        _ => "unknown".into(),
    };
    window.add(&view);
    window.show_all();
    let Some(gdk_window) = window.window() else {
        eprintln!("syncify-webkit-probe: la ventana no tiene GdkWindow");
        return 4;
    };
    state.borrow_mut().window.width = gdk_window.width().max(0) as u32;
    state.borrow_mut().window.height = gdk_window.height().max(0) as u32;
    if request.backend == Backend::X11 {
        state.borrow_mut().window.capture_supported = true;
    }

    let Some(clock) = gdk_window.frame_clock() else {
        eprintln!("syncify-webkit-probe: la ventana no tiene frame clock");
        return 4;
    };
    let paint_state = Rc::clone(&state);
    clock.connect_after_paint(move |_| {
        let paints = paint_state.borrow().paints.saturating_add(1);
        paint_state.borrow_mut().paints = paints;
    });
    let load_state = Rc::clone(&state);
    view.connect_load_changed(move |_, event| {
        if event == LoadEvent::Finished && !load_state.borrow().ready {
            load_state
                .borrow_mut()
                .push_error("load-finished-without-ready".into());
        }
    });
    let termination_state = Rc::clone(&state);
    view.connect_web_process_terminated(move |_, reason| {
        termination_state
            .borrow_mut()
            .push_error(format!("web-process-terminated:{reason:?}"));
    });

    let exit_code = Rc::new(Cell::new(4));
    let warm_state = Rc::clone(&state);
    let warm_window = gdk_window.clone();
    let warm_view = view.clone();
    let warm_result = request.result_path.clone();
    let warm_exit = Rc::clone(&exit_code);
    let _ = gtk::glib::timeout_add_local_once(WARMUP, move || {
        let (frames, paints) = {
            let mut state = warm_state.borrow_mut();
            state.frames_at_t1 = state.frames;
            state.paints_at_t1 = state.paints;
            (state.frames, state.paints)
        };
        let capture = if warm_state.borrow().requested_backend == Backend::X11 {
            match capture_window(&warm_window) {
                Ok(capture) => Some(capture),
                Err(error) => {
                    warm_state.borrow_mut().push_error(error);
                    None
                }
            }
        } else {
            None
        };
        {
            let mut state = warm_state.borrow_mut();
            state.frames_at_t1 = frames;
            state.paints_at_t1 = paints;
            state.first_capture = capture;
        }

        let measure_state = Rc::clone(&warm_state);
        let measure_window = warm_window.clone();
        let measure_view = warm_view.clone();
        let measure_result = warm_result.clone();
        let measure_exit = warm_exit.clone();
        let _ = gtk::glib::timeout_add_local_once(MEASURE, move || {
            let second = if measure_state.borrow().requested_backend == Backend::X11 {
                match capture_window(&measure_window) {
                    Ok(capture) => Some(capture),
                    Err(error) => {
                        measure_state.borrow_mut().push_error(error);
                        None
                    }
                }
            } else {
                None
            };
            if let Some(second) = second {
                let mut state = measure_state.borrow_mut();
                state.window = second.evidence.clone();
                state.window.changed_px = state.first_capture.as_ref().and_then(|first| {
                    if first.width != second.width
                        || first.height != second.height
                        || first.channels != second.channels
                    {
                        return None;
                    }
                    changed_pixels(
                        &first.bytes,
                        &second.bytes,
                        first.width,
                        first.height,
                        first.channels,
                        second.channels,
                        first.stride,
                        second.stride,
                    )
                });
            }

            let snapshot_state = Rc::clone(&measure_state);
            let snapshot_window = measure_window.clone();
            let snapshot_result = measure_result.clone();
            let snapshot_exit = measure_exit.clone();
            let snapshot_scale = measure_view.scale_factor().max(1);
            let snapshot_width = measure_view.allocated_width() * snapshot_scale;
            let snapshot_height = measure_view.allocated_height() * snapshot_scale;
            measure_view.snapshot(
                SnapshotRegion::Visible,
                SnapshotOptions::NONE,
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    let snapshot = match result {
                        Ok(surface) => snapshot_pixels(&surface, snapshot_width, snapshot_height),
                        Err(error) => Err(error.to_string()),
                    };
                    match snapshot {
                        Ok(snapshot) => snapshot_state.borrow_mut().snapshot = snapshot,
                        Err(error) => snapshot_state.borrow_mut().push_error(error),
                    }
                    finish(
                        &snapshot_result,
                        &snapshot_window,
                        &snapshot_state,
                        &snapshot_exit,
                    );
                },
            );
        });
    });

    let deadline_state = Rc::clone(&state);
    let deadline_window = gdk_window.clone();
    let deadline_result = request.result_path.clone();
    let deadline_exit = Rc::clone(&exit_code);
    let _ = gtk::glib::timeout_add_local_once(CHILD_DEADLINE, move || {
        deadline_state
            .borrow_mut()
            .push_error("child-deadline".into());
        finish(
            &deadline_result,
            &deadline_window,
            &deadline_state,
            &deadline_exit,
        );
    });

    view.load_html(FIXTURE, None);
    gtk::main();
    exit_code.get()
}
