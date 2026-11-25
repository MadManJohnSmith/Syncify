//! Pruebas puras y de proceso del probe de composición WebKitGTK.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tempfile::TempDir;

use super::child_request_from_args;
use super::environment::{
    fingerprint_display, fingerprint_wayland_display, parse_drm_uevent, parse_mesa_package_version,
    select_backend, EnvironmentFingerprint,
};
use super::report::{assess, Backend, FrameAssessment, ProbeEvidence, Verdict};
use super::supervisor::{supervise, Supervision};
use super::{
    changed_pixels, configure_child_environment, evidence_assessment, parse_evidence_bytes,
    pixel_stats, supervision_failure,
};
use crate::cmd_utils::create_tokio_command;

fn evidence() -> ProbeEvidence {
    ProbeEvidence {
        ready: true,
        hidden: Some(false),
        viewable: true,
        observed_backend: Some(Backend::X11),
        frames_delta: 60,
        paints_delta: 60,
        acceleration_policy: "always".into(),
        ..ProbeEvidence::default()
    }
}

fn x11_pixels() -> ProbeEvidence {
    let mut value = evidence();
    value.window.available = true;
    value.window.capture_supported = true;
    value.window.width = 480;
    value.window.height = 360;
    value.window.red_px = 3697;
    value.window_red_px_t1 = 3697;
    value.window.colored_px = 20_000;
    value.window.max_channel = 242;
    value.window.changed_px = Some(7518);
    value.snapshot.available = true;
    value.snapshot.red_px = 3700;
    value
}

#[test]
fn backend_matrix_keeps_session_and_effective_backend_separate() {
    let cases = [
        (
            None,
            Some("x11"),
            Some(":0"),
            Some("wayland-1"),
            Some(Backend::X11),
        ),
        (
            None,
            Some("wayland"),
            Some(":0"),
            Some("wayland-1"),
            Some(Backend::Wayland),
        ),
        (
            Some("x11"),
            Some("wayland"),
            Some(":0"),
            Some("wayland-1"),
            Some(Backend::X11),
        ),
        (
            Some("wayland"),
            Some("x11"),
            Some(":0"),
            Some("wayland-1"),
            Some(Backend::Wayland),
        ),
        (Some("x11"), Some("wayland"), None, Some("wayland-1"), None),
        (Some("wayland"), Some("x11"), Some(":0"), None, None),
        (None, Some("tty"), Some(":0"), Some("wayland-1"), None),
        (None, Some("x11"), None, Some("wayland-1"), None),
        (None, Some("wayland"), Some(":0"), None, None),
        (None, None, Some(":0"), None, Some(Backend::X11)),
        (None, None, None, Some("wayland-1"), Some(Backend::Wayland)),
        (None, None, Some(":0"), Some("wayland-1"), None),
        (None, None, None, None, None),
        (Some("broadway"), None, Some(":0"), None, None),
    ];

    for (gdk, session, display, wayland, expected) in cases {
        assert_eq!(
            select_backend(gdk, session, display, wayland),
            expected,
            "gdk={gdk:?} session={session:?} display={display:?} wayland={wayland:?}"
        );
    }
}

#[test]
fn drm_uevent_parser_extracts_driver_and_pci_identity() {
    let parsed = parse_drm_uevent(
        "DRIVER=i915\nPCI_ID=8086:9A49\nPCI_SLOT_NAME=0000:00:02.0\nMODALIAS=pci:v00008086d00009A49\n",
    );
    assert_eq!(parsed.driver.as_deref(), Some("i915"));
    assert_eq!(parsed.pci_id.as_deref(), Some("8086:9A49"));
}

#[test]
fn mesa_package_versions_are_normalized_without_distro_epochs() {
    assert_eq!(
        parse_mesa_package_version("mesa 3:26.3.1-5\n"),
        Some("26.3.1-5".into())
    );
    assert_eq!(
        parse_mesa_package_version("26.2.4-1\n"),
        Some("26.2.4-1".into())
    );
    assert_eq!(parse_mesa_package_version("mesa unknown\n"), None);
}

#[test]
fn fingerprint_endpoints_do_not_leak_remote_hosts_users_or_temp_paths() {
    assert_eq!(fingerprint_display(":0".into()), ":0");
    let remote = fingerprint_display("private-host.example:7".into());
    assert!(remote.starts_with("remote:"));
    assert!(!remote.contains("private-host.example"));
    assert_eq!(
        fingerprint_display("unix:/tmp/private/.X11-unix/X0".into()),
        "unix-local:0"
    );
    assert_eq!(
        fingerprint_wayland_display("/run/user/1234/wayland-7".into()),
        "socket:wayland-7"
    );
}

#[test]
fn fingerprint_key_is_stable_and_every_material_field_invalidates_it() {
    let base = EnvironmentFingerprint::minimal(Some(Backend::X11));
    assert_eq!(base.key(), base.clone().key());
    assert!(!base.is_complete());

    let mut changes = Vec::new();
    let mut value = base.clone();
    value.backend = Some(Backend::Wayland);
    changes.push(value.key());
    let mut value = base.clone();
    value.session_type = "wayland".into();
    changes.push(value.key());
    let mut value = base.clone();
    value.driver_version = "580.196.07".into();
    changes.push(value.key());
    let mut value = base.clone();
    value.webkitgtk_version = "2.54.3".into();
    changes.push(value.key());
    let mut value = base.clone();
    value.mesa_version = "26.3.1".into();
    changes.push(value.key());
    let mut value = base.clone();
    value.mesa_source = "pacman".into();
    changes.push(value.key());
    let mut value = base.clone();
    value.drm_devices.push(super::environment::DrmDevice {
        node: "card1".into(),
        driver: "i915".into(),
        driver_version: "kernel:test".into(),
        pci_id: Some("8086:9A49".into()),
    });
    changes.push(value.key());
    let mut value = base.clone();
    value.rendering_overrides.dri_prime = Some("selected-gpu".into());
    changes.push(value.key());
    let mut value = base.clone();
    value.probe_contract_version += 1;
    changes.push(value.key());

    for key in changes {
        assert_ne!(key, base.key());
    }
}

#[test]
fn incomplete_fingerprint_or_wrong_child_backend_never_yields_supported() {
    let evidence = x11_pixels();
    assert_eq!(
        evidence_assessment(&evidence, Backend::X11, false).verdict,
        Verdict::Inconclusive
    );
    let mut wrong_backend = evidence.clone();
    wrong_backend.requested_backend = Backend::Wayland;
    assert_eq!(
        evidence_assessment(&wrong_backend, Backend::X11, true).verdict,
        Verdict::Inconclusive
    );
    assert_eq!(
        evidence_assessment(&evidence, Backend::X11, true).verdict,
        Verdict::Supported
    );
}

#[test]
fn child_environment_scrubs_credentials_and_redirects_home() {
    const SECRET: &str = "SYNCIFY_WEBKIT_PROBE_TEST_SECRET";
    let previous = std::env::var_os(SECRET);
    std::env::set_var(SECRET, "not-a-real-credential");
    let temp = TempDir::new().unwrap();
    let mut command = create_tokio_command("/bin/true");
    configure_child_environment(&mut command, temp.path(), Backend::X11);
    let mut has_env = |name: &str| {
        command
            .as_std_mut()
            .get_envs()
            .any(|(key, _)| key == OsStr::new(name))
    };
    assert!(!has_env(SECRET));
    assert!(!has_env("DEEZER_ARL"));
    assert!(!has_env("QOBUZ_API_SECRET"));
    assert!(has_env("HOME"));
    assert!(has_env("GDK_BACKEND"));
    assert!(has_env("XDG_DATA_HOME"));

    match previous {
        Some(value) => std::env::set_var(SECRET, value),
        None => std::env::remove_var(SECRET),
    }
}

#[test]
fn wk01_x11_visible_pixels_are_presented() {
    let assessment = assess(&x11_pixels());
    assert_eq!(assessment.verdict, Verdict::Supported);
    assert_eq!(assessment.frame, FrameAssessment::ContentObserved);
}

#[test]
fn wk01_x11_gbm_black_is_rendered_not_presented() {
    let mut value = evidence();
    value.frames_delta = 0;
    value.paints_delta = 0;
    value.window.available = true;
    value.window.capture_supported = true;
    value.window.width = 480;
    value.window.height = 360;
    value.window.max_channel = 0;
    value.window.changed_px = Some(0);
    value.snapshot.available = true;
    value.snapshot.red_px = 3679;

    let assessment = assess(&value);
    assert_eq!(assessment.verdict, Verdict::Unsupported);
    assert_eq!(assessment.frame, FrameAssessment::RenderedNotPresented);
}

#[test]
fn x11_threshold_requires_marker_in_both_captures_and_meaningful_change() {
    let mut boundary = x11_pixels();
    boundary.window.red_px = 300;
    boundary.window_red_px_t1 = 300;
    boundary.window.changed_px = Some(300);
    assert_eq!(assess(&boundary).verdict, Verdict::Supported);

    let mut too_few = boundary.clone();
    too_few.window.red_px = 299;
    assert_ne!(assess(&too_few).verdict, Verdict::Supported);

    let mut missing_initial_marker = boundary.clone();
    missing_initial_marker.window_red_px_t1 = 299;
    assert_ne!(assess(&missing_initial_marker).verdict, Verdict::Supported);

    let mut too_little_change = boundary;
    too_little_change.window.changed_px = Some(299);
    assert_ne!(assess(&too_little_change).verdict, Verdict::Supported);
}

#[test]
fn black_x11_capture_is_inconclusive_without_a_compatible_control() {
    let mut value = evidence();
    value.window.available = true;
    value.window.capture_supported = true;
    value.window.width = 480;
    value.window.height = 360;
    value.window.max_channel = 0;
    value.window.changed_px = Some(0);
    value.snapshot.available = true;
    value.snapshot.red_px = 3600;

    let assessment = assess(&value);
    assert_eq!(assessment.verdict, Verdict::Inconclusive);
    assert_eq!(assessment.frame, FrameAssessment::SuspectedBlack);
}

#[test]
fn wayland_black_capture_is_paint_without_pixel_proof() {
    let mut value = evidence();
    value.requested_backend = Backend::Wayland;
    value.observed_backend = Some(Backend::Wayland);
    value.window.available = true;
    value.window.capture_supported = false;
    value.window.max_channel = 0;
    value.snapshot.available = true;
    value.snapshot.red_px = 3600;

    let assessment = assess(&value);
    assert_eq!(assessment.verdict, Verdict::Inconclusive);
    assert_eq!(assessment.frame, FrameAssessment::PaintObservedNoPixelProof);
}

#[test]
fn wayland_negative_signals_remain_inconclusive() {
    let mut not_ready = evidence();
    not_ready.requested_backend = Backend::Wayland;
    not_ready.observed_backend = Some(Backend::Wayland);
    not_ready.ready = false;
    assert_eq!(assess(&not_ready).verdict, Verdict::Inconclusive);

    let mut hidden = evidence();
    hidden.requested_backend = Backend::Wayland;
    hidden.observed_backend = Some(Backend::Wayland);
    hidden.hidden = Some(true);
    assert_eq!(assess(&hidden).verdict, Verdict::Inconclusive);

    let mut no_paints = evidence();
    no_paints.requested_backend = Backend::Wayland;
    no_paints.observed_backend = Some(Backend::Wayland);
    no_paints.paints_delta = 0;
    no_paints.snapshot.available = true;
    no_paints.snapshot.red_px = 3600;
    assert_eq!(assess(&no_paints).verdict, Verdict::Inconclusive);

    let mut no_frames = evidence();
    no_frames.requested_backend = Backend::Wayland;
    no_frames.observed_backend = Some(Backend::Wayland);
    no_frames.frames_delta = 0;
    assert_eq!(assess(&no_frames).verdict, Verdict::Inconclusive);
}

#[test]
fn wayland_process_failure_is_inconclusive_but_x11_failure_is_unsupported() {
    let failed = Supervision {
        signal: Some(libc::SIGABRT),
        ..Supervision::default()
    };
    assert_eq!(
        supervision_failure(Backend::Wayland, &failed)
            .expect("failed supervision")
            .verdict,
        Verdict::Inconclusive
    );
    assert_eq!(
        supervision_failure(Backend::X11, &failed)
            .expect("failed supervision")
            .verdict,
        Verdict::Unsupported
    );
    assert!(supervision_failure(
        Backend::X11,
        &Supervision {
            exit_code: Some(0),
            ..Supervision::default()
        }
    )
    .is_none());
}

#[test]
fn fixture_mismatch_or_fatal_runtime_error_blocks_a_positive() {
    let mut wrong_fixture = x11_pixels();
    wrong_fixture.fixture_id = "other-fixture".into();
    assert_eq!(assess(&wrong_fixture).verdict, Verdict::Inconclusive);

    let mut crashed = x11_pixels();
    crashed
        .runtime_errors
        .push("web-process-terminated:Crashed".into());
    assert_eq!(assess(&crashed).verdict, Verdict::Inconclusive);
}

#[test]
fn raf_snapshot_and_acceleration_policy_alone_never_prove_presentation() {
    let mut value = evidence();
    value.paints_delta = 0;
    value.snapshot.available = true;
    value.snapshot.red_px = 3600;
    value.acceleration_policy = "always".into();
    assert_ne!(assess(&value).verdict, Verdict::Supported);

    let mut wrong_policy = x11_pixels();
    wrong_policy.acceleration_policy = "never".into();
    assert_eq!(assess(&wrong_policy).verdict, Verdict::Inconclusive);
}

#[test]
fn hidden_unviewable_or_backend_mismatch_is_never_positive() {
    let mut hidden = x11_pixels();
    hidden.hidden = Some(true);
    assert_ne!(assess(&hidden).verdict, Verdict::Supported);

    let mut unviewable = x11_pixels();
    unviewable.viewable = false;
    assert_ne!(assess(&unviewable).verdict, Verdict::Supported);

    let mut mismatch = x11_pixels();
    mismatch.observed_backend = Some(Backend::Wayland);
    assert_eq!(assess(&mismatch).verdict, Verdict::Inconclusive);
}

#[test]
fn pixel_stats_handle_rgb_stride_and_bgra_order() {
    let rgb = [224, 64, 64, 0, 16, 24, 32, 64, 16, 0, 0, 0];
    let stats = pixel_stats(&rgb, 2, 2, 3, 6, 0, 1, 2);
    assert_eq!(stats.red_px, 1);
    assert_eq!(stats.max_channel, 224);

    let bgra = [64, 64, 224, 255, 0, 0, 0, 255];
    let stats = pixel_stats(&bgra, 2, 1, 4, 8, 2, 1, 0);
    assert_eq!(stats.red_px, 1);
}

#[test]
fn changed_pixels_respects_stride_and_dimensions() {
    let first = [10, 10, 10, 0, 10, 10, 10, 99];
    let second = [10, 10, 10, 0, 11, 10, 10, 98];
    assert_eq!(changed_pixels(&first, &second, 2, 1, 4, 4, 8, 8), Some(1));
    assert_eq!(changed_pixels(&first, &second, 3, 1, 4, 4, 8, 8), None);
    assert_eq!(changed_pixels(&first, &second, 2, 1, 4, 3, 8, 8), None);
}

#[test]
fn child_request_parser_rejects_missing_or_invalid_backend() {
    let request = child_request_from_args([
        "--syncify-webkit-probe-child".into(),
        "/tmp/result.json".into(),
        "x11".into(),
    ]);
    let request = request.expect("flag present").expect("valid request");
    assert_eq!(request.result_path, PathBuf::from("/tmp/result.json"));
    assert_eq!(request.backend, Backend::X11);
    assert!(child_request_from_args(std::iter::empty::<OsString>()).is_none());
    assert!(child_request_from_args(["--syncify-webkit-probe-child".into()]).is_some());
}

#[test]
fn malformed_or_truncated_evidence_is_rejected() {
    assert!(parse_evidence_bytes(b"{\"schema_version\":1").is_none());
    assert!(parse_evidence_bytes(b"{}").is_none());
    let valid = serde_json::to_vec(&x11_pixels()).unwrap();
    assert!(parse_evidence_bytes(&valid).is_some());
}

#[tokio::test]
async fn supervisor_waits_for_a_fast_child_and_reports_success() {
    let mut command = create_tokio_command("/bin/sh");
    command.args(["-c", "exit 0"]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let outcome = supervise(command, Duration::from_secs(2), Duration::from_millis(100)).await;
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.timed_out);
    assert!(!outcome.kill_escalated);
    assert!(outcome.spawn_error.is_none());
}

#[tokio::test]
async fn supervisor_terms_a_graceful_child_but_still_reports_timeout() {
    let mut command = create_tokio_command("/bin/sh");
    command.args(["-c", "trap 'exit 0' TERM; while :; do sleep 0.05; done"]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let outcome = supervise(command, Duration::from_millis(80), Duration::from_secs(1)).await;
    assert!(outcome.timed_out);
    assert!(!outcome.kill_escalated);
}

#[tokio::test]
async fn supervisor_kills_a_term_resistant_process_group() {
    let temp = TempDir::new().unwrap();
    let pid_file = temp.path().join("pid");
    let descendant_file = temp.path().join("descendant-pid");
    let mut command = create_tokio_command("/bin/sh");
    command.args([
        "-c",
        "trap '' TERM; echo $$ > \"$1\"; sleep 30 & echo $! > \"$2\"; wait",
        "probe-test",
    ]);
    command.arg(&pid_file).arg(&descendant_file);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let outcome = supervise(
        command,
        Duration::from_millis(120),
        Duration::from_millis(120),
    )
    .await;
    assert!(outcome.timed_out);
    assert!(outcome.kill_escalated);
    assert_eq!(outcome.signal, Some(libc::SIGKILL));

    let leader: i32 = std::fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let descendant: i32 = std::fs::read_to_string(descendant_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    for pid in [leader, descendant] {
        for _ in 0..100 {
            let state = std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|stat| stat.rsplit(") ").next().map(|tail| tail.chars().next()));
            if !matches!(state, Some(Some('R' | 'S' | 'D'))) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let state = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| stat.rsplit(") ").next().map(|tail| tail.chars().next()));
        assert!(
            !matches!(state, Some(Some('R' | 'S' | 'D'))),
            "el proceso {pid} del grupo colgado debe estar muerto o zombi"
        );
    }
    assert!(
        unsafe { libc::kill(leader, 0) } == -1,
        "Tokio debe haber reapado al líder"
    );
}

#[tokio::test]
async fn supervisor_reports_spawn_failure_without_panicking() {
    let command = create_tokio_command("/definitely/missing/syncify-probe");
    let outcome = supervise(command, Duration::from_secs(1), Duration::from_millis(10)).await;
    assert!(outcome.spawn_error.is_some());
    assert!(!outcome.timed_out);
}
