//! Caracterización del contrato único de puentes (`bridges::BridgeClient`).
//!
//! Cada test fija un desenlace que antes dependía de uno de los tres
//! lanzadores: éxito con envelope, fallo de dominio con exit 1, salida no-cero
//! sin envelope, stdout ilegible, timeout y entrega por stdin. Los fixtures son
//! scripts Python efímeros en un directorio temporal; `BridgeClient` acepta
//! rutas absolutas, así que no hace falta tocar `scripts/`.

use std::io::Write;
use std::time::Duration;

use syncify_tauri_lib::bridges::{BridgeClient, CallOptions};

fn script(dir: &std::path::Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("create fixture");
    file.write_all(body.as_bytes()).expect("write fixture");
    path.to_string_lossy().into_owned()
}

fn options() -> CallOptions<'static> {
    CallOptions::with_timeout(Duration::from_secs(20))
}

#[tokio::test]
async fn call_parses_success_envelope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "ok.py",
        "print('{\"success\": true, \"data\": {\"n\": 1}}')\n",
    );
    let envelope = BridgeClient::call::<serde_json::Value>(&path, &[], options())
        .await
        .expect("call should succeed");
    assert!(envelope.success);
    assert_eq!(envelope.data.unwrap()["n"], 1);
    assert!(envelope.error.is_none());
}

#[tokio::test]
async fn call_parses_domain_failure_even_with_nonzero_exit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "fail.py",
        "import sys\nprint('{\"success\": false, \"error\": \"boom\"}')\nsys.exit(1)\n",
    );
    let envelope = BridgeClient::call::<serde_json::Value>(&path, &[], options())
        .await
        .expect("a domain failure is still a readable envelope");
    assert!(!envelope.success);
    assert_eq!(envelope.error.as_deref(), Some("boom"));
}

#[tokio::test]
async fn call_reports_exit_failure_when_stdout_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "empty_exit.py",
        "import sys\nprint('kaput', file=sys.stderr)\nsys.exit(3)\n",
    );
    let err = BridgeClient::call::<serde_json::Value>(&path, &[], options())
        .await
        .expect_err("no envelope means an exit failure");
    assert_eq!(err.to_string(), format!("Bridge {} failed: kaput", path));
}

#[tokio::test]
async fn call_reports_parse_failure_with_stderr_context() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "garbage.py",
        "print('not json')\nprint('bad', file=__import__('sys').stderr)\n",
    );
    let err = BridgeClient::call::<serde_json::Value>(&path, &[], options())
        .await
        .expect_err("garbage stdout must not parse");
    assert_eq!(
        err.to_string(),
        format!(
            "Failed to parse result from {}: not json (stderr: bad)",
            path
        )
    );
}

#[tokio::test]
async fn call_reports_no_output_when_stdout_is_empty_and_stderr_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "stderr_only.py",
        "print('problema', file=__import__('sys').stderr)\n",
    );
    let err = BridgeClient::call::<serde_json::Value>(&path, &[], options())
        .await
        .expect_err("no stdout means no output");
    assert_eq!(
        err.to_string(),
        format!("Bridge {} produced no output. Error: problema", path)
    );
}

#[tokio::test]
async fn call_times_out_with_declared_budget() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(dir.path(), "slow.py", "import time\ntime.sleep(5)\n");
    let err = BridgeClient::call::<serde_json::Value>(
        &path,
        &[],
        CallOptions::with_timeout(Duration::from_secs(1)),
    )
    .await
    .expect_err("must time out");
    assert_eq!(
        err.to_string(),
        format!("Bridge {} timed out after 1 seconds", path)
    );
}

#[tokio::test]
async fn capture_delivers_stdin_payload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "echo.py",
        "import sys\ndata = sys.stdin.read().strip()\nprint('{\"success\": true, \"data\": {\"echo\": \"%s\"}}' % data)\n",
    );
    let raw = BridgeClient::capture(
        &path,
        &[],
        CallOptions {
            timeout: Duration::from_secs(20),
            stdin: Some(b"secreto"),
        },
    )
    .await
    .expect("capture should succeed");
    assert!(raw.exit_ok());
    assert!(raw.stdout.contains("secreto"), "stdout: {}", raw.stdout);
}

#[tokio::test]
async fn capture_exposes_exit_status_for_callers_with_own_payload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = script(
        dir.path(),
        "code7.py",
        "import sys\nprint('{\"success\": true}')\nsys.exit(7)\n",
    );
    let raw = BridgeClient::capture(&path, &[], options())
        .await
        .expect("capture should succeed");
    assert!(!raw.exit_ok());
    assert_eq!(raw.exit_code(), Some(7));
    assert!(raw.stdout.contains("\"success\""));
}
