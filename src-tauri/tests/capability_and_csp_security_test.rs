use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn get_src_tauri_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_default_capability_shell_allow_open_scope_restricted() {
    let capability_path = get_src_tauri_dir().join("capabilities/default.json");
    let content = fs::read_to_string(&capability_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", capability_path.display(), e));
    let json: Value = serde_json::from_str(&content).expect("Valid JSON expected in default.json");

    let permissions = json
        .get("permissions")
        .and_then(Value::as_array)
        .expect("permissions array expected in capability file");

    // 1. Ensure bare string "shell:allow-open" is NEVER present without scope
    for perm in permissions {
        if let Some(perm_str) = perm.as_str() {
            assert_ne!(
                perm_str, "shell:allow-open",
                "Bare un-scoped 'shell:allow-open' permission is forbidden (violates SEC-007)"
            );
        }
    }

    // 2. Locate scoped shell:allow-open entry
    let scoped_entry = permissions
        .iter()
        .find(|entry| {
            entry.is_object()
                && entry.get("identifier").and_then(Value::as_str) == Some("shell:allow-open")
        })
        .expect("Expected scoped 'shell:allow-open' object in permissions");

    let allow_rules = scoped_entry
        .get("allow")
        .and_then(Value::as_array)
        .expect("Expected 'allow' array in scoped shell:allow-open permission");

    assert!(
        !allow_rules.is_empty(),
        "Scoped shell:allow-open must have at least one allow rule"
    );

    // 3. Verify href pattern restricts to http/https schemes only
    let mut found_http_https_scope = false;
    for rule in allow_rules {
        let href_pattern = rule
            .get("href")
            .and_then(Value::as_str)
            .expect("Rule must define 'href' pattern string");

        assert!(
            href_pattern.starts_with("^https?://"),
            "href scope must strictly require http or https scheme, found: {}",
            href_pattern
        );

        let regex = regex::Regex::new(href_pattern)
            .unwrap_or_else(|e| panic!("Invalid regex pattern '{}': {}", href_pattern, e));

        // Must match legitimate web URLs
        assert!(regex.is_match("https://open.spotify.com/track/123"));
        assert!(regex.is_match("https://www.qobuz.com"));
        assert!(regex.is_match("http://example.com/test"));

        // Must reject dangerous schemes
        assert!(!regex.is_match("file:///etc/passwd"));
        assert!(!regex.is_match("cmd://calc.exe"));
        assert!(!regex.is_match("powershell://invoke"));
        assert!(!regex.is_match("ms-msdt://id"));
        assert!(!regex.is_match("javascript:alert(1)"));

        found_http_https_scope = true;
    }

    assert!(
        found_http_https_scope,
        "shell:allow-open must have verified http/https scope rule"
    );
}

#[test]
fn test_tauri_conf_with_global_tauri_disabled() {
    let conf_path = get_src_tauri_dir().join("tauri.conf.json");
    let content = fs::read_to_string(&conf_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", conf_path.display(), e));
    let json: Value = serde_json::from_str(&content).expect("Valid JSON expected in tauri.conf.json");

    let with_global_tauri = json
        .get("app")
        .and_then(|app| app.get("withGlobalTauri"))
        .and_then(Value::as_bool);

    assert_eq!(
        with_global_tauri,
        Some(false),
        "app.withGlobalTauri must be false to prevent full Tauri API exposure on window"
    );
}

#[test]
fn test_tauri_conf_csp_connect_src_no_localhost_wildcards() {
    let conf_path = get_src_tauri_dir().join("tauri.conf.json");
    let content = fs::read_to_string(&conf_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", conf_path.display(), e));
    let json: Value = serde_json::from_str(&content).expect("Valid JSON expected in tauri.conf.json");

    let csp = json
        .get("app")
        .and_then(|app| app.get("security"))
        .and_then(|sec| sec.get("csp"))
        .and_then(Value::as_str)
        .expect("Expected app.security.csp string in tauri.conf.json");

    // Parse CSP directives
    let directives: Vec<&str> = csp.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();

    let connect_src = directives
        .iter()
        .find(|d| d.starts_with("connect-src"))
        .expect("Expected connect-src directive in CSP");

    // Must NOT contain localhost wildcards
    assert!(
        !connect_src.contains("localhost:*"),
        "connect-src must NOT contain localhost:* wildcards (found in '{}')",
        connect_src
    );
    assert!(
        !connect_src.contains("http://localhost:*"),
        "connect-src must NOT contain http://localhost:* (found in '{}')",
        connect_src
    );
    assert!(
        !connect_src.contains("ws://localhost:*"),
        "connect-src must NOT contain ws://localhost:* (found in '{}')",
        connect_src
    );

    // Must strictly include necessary origins
    let tokens: Vec<&str> = connect_src.split_whitespace().collect();
    assert!(tokens.contains(&"'self'"), "connect-src must include 'self'");
    assert!(tokens.contains(&"ipc:"), "connect-src must include ipc:");
    assert!(
        tokens.contains(&"http://ipc.localhost"),
        "connect-src must include http://ipc.localhost"
    );

    // Authorized streaming and metadata APIs
    assert!(
        tokens.contains(&"https://api.spotify.com"),
        "connect-src must include https://api.spotify.com"
    );
    assert!(
        tokens.contains(&"https://api.tidal.com"),
        "connect-src must include https://api.tidal.com"
    );
    assert!(
        tokens.contains(&"https://play.qobuz.com"),
        "connect-src must include https://play.qobuz.com"
    );
    assert!(
        tokens.contains(&"https://api.deezer.com"),
        "connect-src must include https://api.deezer.com"
    );
    assert!(
        tokens.contains(&"https://musicbrainz.org"),
        "connect-src must include https://musicbrainz.org"
    );
}

#[test]
fn test_tauri_conf_csp_secure_baseline_directives() {
    let conf_path = get_src_tauri_dir().join("tauri.conf.json");
    let content = fs::read_to_string(&conf_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", conf_path.display(), e));
    let json: Value = serde_json::from_str(&content).expect("Valid JSON expected in tauri.conf.json");

    let csp = json
        .get("app")
        .and_then(|app| app.get("security"))
        .and_then(|sec| sec.get("csp"))
        .and_then(Value::as_str)
        .expect("Expected app.security.csp string in tauri.conf.json");

    let directives: Vec<&str> = csp.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();

    // 1. default-src 'self'
    let default_src = directives
        .iter()
        .find(|d| d.starts_with("default-src"))
        .expect("CSP must include default-src");
    assert!(
        default_src.contains("'self'"),
        "default-src must contain 'self'"
    );

    // 2. script-src 'self' and NO 'unsafe-inline'
    let script_src = directives
        .iter()
        .find(|d| d.starts_with("script-src"))
        .expect("CSP must include script-src");
    assert!(
        script_src.contains("'self'"),
        "script-src must contain 'self'"
    );
    assert!(
        !script_src.contains("'unsafe-inline'"),
        "script-src must NOT contain 'unsafe-inline'"
    );

    // 3. object-src 'none'
    let object_src = directives
        .iter()
        .find(|d| d.starts_with("object-src"))
        .expect("CSP must include object-src");
    assert_eq!(
        *object_src, "object-src 'none'",
        "object-src must be strictly 'none'"
    );

    // 4. base-uri 'self'
    let base_uri = directives
        .iter()
        .find(|d| d.starts_with("base-uri"))
        .expect("CSP must include base-uri");
    assert!(base_uri.contains("'self'"), "base-uri must contain 'self'");
}

#[test]
fn test_tauri_conf_plugin_shell_scope_hardening() {
    let conf_path = get_src_tauri_dir().join("tauri.conf.json");
    let content = fs::read_to_string(&conf_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", conf_path.display(), e));
    let json: Value = serde_json::from_str(&content).expect("Valid JSON expected in tauri.conf.json");

    let open_validator = json
        .get("plugins")
        .and_then(|p| p.get("shell"))
        .and_then(|s| s.get("open"))
        .and_then(Value::as_str)
        .expect("Expected plugins.shell.open in tauri.conf.json");

    assert!(
        open_validator == "https?://.+" || open_validator == "^https?://.+",
        "plugins.shell.open must enforce http/https regex validation, found: {}",
        open_validator
    );
}
