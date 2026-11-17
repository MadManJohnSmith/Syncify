//! Login dentro de la app por cuenta (issue #84, implementa #62).
//!
//! Contrato: ADR-0007, decisión 5 — webview con descriptor por proveedor,
//! lectura SOLO del artefacto necesario (cookie o clave de almacenamiento),
//! cancelación limpia al cerrar la ventana y ningún dato del formulario
//! (contraseña ni correo) capturado, guardado o registrado.
//!
//! Cada proveedor declara un [`LoginDescriptor`]. La ventana de webview se
//! abre con un `profile_key` por intento (directorio de perfil provisional
//! bajo `login-profiles/`), el bucle de captura sondea el artefacto y, al
//! capturarlo, lo entrega AL CAMINO EXISTENTE de guardado
//! ([`crate::commands::auth::save_auth_result`]). El perfil provisional se
//! borra SIEMPRE al terminar el intento (éxito, fallo o cancelación): la
//! sesión se recuerda mediante el artefacto cifrado en la BD, nunca mediante
//! cookies en texto plano en disco.
//!
//! Los puentes Python dejan de lanzar navegadores EN ESTE CAMINO: aquí no se
//! invoca ningún flujo de Playwright; el único puente involucrado es el
//! device-code de Tidal (sus tokens no viven en el webview) dividido en
//! `login_start`/`login_poll` para que sea el webview quien muestre la URL de
//! verificación. Playwright y el lanzador del sistema siguen en el camino
//! histórico (`start_auth_and_save`) hasta la migración completa (#62).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde_json::Value;
use tauri::{Emitter, Manager};

use super::auth::run_auth_bridge_subprocess;

/// Directorio base de los perfiles provisionales de intento, relativo al
/// app_local_data_dir. Cada intento crea `<base>/<profile_key>` y lo borra al
/// cerrar la ventana.
pub const LOGIN_PROFILE_BASE: &str = "login-profiles";

/// Prefijo que delimita el eco del artefacto en el título de la ventana: el
/// bucle de captura sondea JavaScript con `eval` y lee el título; una página
/// honesta nunca produce este prefijo, así que un título propio del proveedor
/// no puede falsear un artefacto.
pub const ARTIFACT_TITLE_PREFIX: &str = "SYNCIFY_ARTIFACT::";

/// Ventana abierta durante el sondeo de cookies/JS.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Eco del artefacto vía título: espera entre `eval` y lectura del título.
const EVAL_ECHO_INTERVAL: Duration = Duration::from_millis(300);

// ==============================================
// DESCRIPTORES POR PROVEEDOR
// ==============================================

/// Qué artefacto captura el webview y cómo.
pub enum ArtifactSpec {
    /// Una cookie del jar nativo del webview (WebKitGTK/WebView2/WKWebView).
    /// Las HttpOnly no son legibles desde JavaScript: se leen del jar, como la
    /// sonda de Deezer (`debug_deezer_cookie_probe`).
    Cookie {
        name: &'static str,
        scope_url: &'static str,
        min_len: usize,
    },
    /// Valor capturado por JavaScript: un script de inicialización engancha
    /// `fetch`/`XMLHttpRequest` (cabeceras de autorización) y el script de
    /// sondeo también mira el `localStorage` del proveedor. El resultado viaja
    /// por el eco del título ([`ARTIFACT_TITLE_PREFIX`]).
    JavaScript {
        init_script: &'static str,
        poll_script: &'static str,
    },
    /// Cookie requerida del jar nativo más un valor opcional capturado por
    /// JavaScript (Apple Music: la cookie `media-user-token` es el artefacto;
    /// el developer token del reproductor viaja si está disponible).
    CookieWithJavaScript {
        init_script: &'static str,
        cookie_name: &'static str,
        scope_url: &'static str,
        min_len: usize,
        poll_script: &'static str,
    },
    /// El artefacto lo entrega el puente (device code de Tidal): el webview
    /// solo muestra la URL de verificación y el puente sondea los tokens.
    DeviceCode,
    /// Spotify: OAuth PKCE dentro del webview (`commands::auth`); el artefacto
    /// es el código de autorización intercambiado por tokens de refresco.
    PkceOAuth,
}

/// Descriptor de login de un proveedor (issue #62, decisión de implementación).
pub struct LoginDescriptor {
    pub service: &'static str,
    pub window_title: &'static str,
    /// URL de entrada del login. Para `DeviceCode` es informativa: la URL real
    /// la emite el puente (`verification_url` completa con el código).
    pub entry_url: &'static str,
    pub artifact: ArtifactSpec,
}

/// Script de inicialización compartido: engancha `fetch` y
/// `XMLHttpRequest.setRequestHeader` para capturar cabeceras de autorización.
/// NO lee ningún campo del formulario: ni contraseñas ni correos pasan por
/// aquí, solo el artefacto resultante de la sesión ya iniciada.
const AUTH_HEADER_HOOK_INIT: &str = r#"(() => {
    if (window.__syncifyCapture) return;
    const cap = (window.__syncifyCapture = { token: null, devToken: null });
    const raw = (v) => (typeof v === 'string' && v.trim() ? v.trim() : null);
    const note = (k, v) => {
        if (!v) return;
        if (k === 'authorization' && v.startsWith('Bearer eyJ')) cap.devToken = v.slice(7);
        if (k === 'authorization' && v.startsWith('OAuth ')) cap.token = v;
        if (k === 'media-user-token') cap.token = v;
        if (k === 'x-user-auth-token') cap.token = v;
    };
    try {
        const origFetch = window.fetch;
        if (origFetch) {
            window.fetch = function (resource, init) {
                try {
                    const headers = (init && init.headers) || (resource && resource.headers) || null;
                    if (headers && typeof headers.forEach === 'function') {
                        headers.forEach((value, key) => note(String(key).toLowerCase(), raw(value)));
                    }
                } catch (e) {}
                return origFetch.apply(this, arguments);
            };
        }
    } catch (e) {}
    try {
        const origSet = XMLHttpRequest.prototype.setRequestHeader;
        if (origSet) {
            XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
                try { note(String(name).toLowerCase(), raw(value)); } catch (e) {}
                return origSet.apply(this, arguments);
            };
        }
    } catch (e) {}
})();"#;

/// Sondeo Qobuz: token de la API capturado de las peticiones o del
/// `localStorage` del reproductor web. Sin cookies de sesión ni formularios.
const QOBUZ_POLL_SCRIPT: &str = r#"(() => {
    try {
        const cap = window.__syncifyCapture || {};
        let tok = typeof cap.token === 'string' ? cap.token.trim() : null;
        if (!tok && window.localStorage) {
            for (const key of ['qobuz_user_auth_token', 'user_auth_token', 'userAuthToken']) {
                const v = localStorage.getItem(key);
                if (typeof v === 'string' && v.length >= 16 && !v.includes('{') && !/\s/.test(v)) {
                    tok = v.trim();
                    break;
                }
            }
        }
        if (tok && tok.length >= 16 && !/\s/.test(tok) && !tok.includes('{')) {
            document.title = 'SYNCIFY_ARTIFACT::' + JSON.stringify({ user_auth_token: tok });
        }
    } catch (e) {}
})();"#;

/// Sondeo SoundCloud: token OAuth de las peticiones API o del `localStorage`
/// del reproductor (`V2::oauth::token`), igual que hacía la captura del puente.
const SOUNDCLOUD_POLL_SCRIPT: &str = r#"(() => {
    try {
        const cap = window.__syncifyCapture || {};
        let tok = typeof cap.token === 'string' ? cap.token.trim() : null;
        if (!tok && window.localStorage) {
            const raw = localStorage.getItem('V2::oauth::token');
            if (raw) {
                try {
                    const parsed = JSON.parse(raw);
                    if (parsed && typeof parsed.access_token === 'string') tok = 'OAuth ' + parsed.access_token;
                } catch (e) {}
            }
        }
        if (tok) {
            if (!tok.startsWith('OAuth ')) tok = 'OAuth ' + tok;
            const bare = tok.slice(6);
            if (bare.length >= 20 && !/\s/.test(bare)) {
                document.title = 'SYNCIFY_ARTIFACT::' + JSON.stringify({ oauth_token: tok, access_token: bare });
            }
        }
    } catch (e) {}
})();"#;

/// Sondeo Apple Music (complementario a la cookie `media-user-token`): solo el
/// developer token del reproductor, opcional. El artefacto requerido es la
/// cookie y se lee del jar nativo, no de aquí.
const APPLE_POLL_SCRIPT: &str = r#"(() => {
    try {
        const cap = window.__syncifyCapture || {};
        const dev = cap.devToken;
        if (dev && dev.length > 40) {
            document.title = 'SYNCIFY_ARTIFACT::' + JSON.stringify({ developer_token: dev });
        }
    } catch (e) {}
})();"#;

/// Los seis descriptores (mismo gesto para todos los servicios; solo cambia
/// qué se lee). Contrato probado por
/// `tests/account_login_descriptor_test.rs`.
pub const SERVICE_DESCRIPTORS: &[LoginDescriptor] = &[
    LoginDescriptor {
        service: "spotify",
        window_title: "Connect Spotify",
        entry_url: "https://accounts.spotify.com/authorize",
        artifact: ArtifactSpec::PkceOAuth,
    },
    LoginDescriptor {
        service: "qobuz",
        window_title: "Connect Qobuz",
        entry_url: "https://www.qobuz.com/login",
        artifact: ArtifactSpec::JavaScript {
            init_script: AUTH_HEADER_HOOK_INIT,
            poll_script: QOBUZ_POLL_SCRIPT,
        },
    },
    LoginDescriptor {
        service: "tidal",
        window_title: "Connect Tidal",
        entry_url: "https://link.tidal.com/",
        artifact: ArtifactSpec::DeviceCode,
    },
    LoginDescriptor {
        service: "deezer",
        window_title: "Connect Deezer",
        entry_url: "https://www.deezer.com/login",
        artifact: ArtifactSpec::Cookie {
            name: "arl",
            scope_url: "https://www.deezer.com/",
            min_len: 101,
        },
    },
    LoginDescriptor {
        service: "soundcloud",
        window_title: "Connect SoundCloud",
        entry_url: "https://soundcloud.com/signin",
        artifact: ArtifactSpec::JavaScript {
            init_script: AUTH_HEADER_HOOK_INIT,
            poll_script: SOUNDCLOUD_POLL_SCRIPT,
        },
    },
    LoginDescriptor {
        service: "apple_music",
        window_title: "Connect Apple Music",
        entry_url: "https://music.apple.com/",
        artifact: ArtifactSpec::CookieWithJavaScript {
            init_script: AUTH_HEADER_HOOK_INIT,
            cookie_name: "media-user-token",
            scope_url: "https://music.apple.com/",
            min_len: 20,
            poll_script: APPLE_POLL_SCRIPT,
        },
    },
];

/// Descriptor canónico de un servicio (`service` en minúsculas, p. ej.
/// `apple_music`). `None` para un servicio desconocido: la UI nunca abre una
/// ventana de login sin descriptor.
pub fn login_descriptor(service: &str) -> Option<&'static LoginDescriptor> {
    let lowered = service.trim().to_ascii_lowercase();
    SERVICE_DESCRIPTORS.iter().find(|d| d.service == lowered)
}

// ==============================================
// VALIDACIÓN DEL ARTEFACTO (ARTEFACTO FALSO EN TESTS)
// ==============================================

/// Validez mínima del artefacto capturado. Los tests usan valores falsos que
/// pasan por estas mismas reglas (sin red, sin webview real).
pub fn valid_artifact_value(service: &str, value: &str) -> bool {
    let value = value.trim();
    match service {
        "deezer" => {
            // La regla histórica del puente: ARL largo (típicamente ~192 chars).
            value.len() > 100
        }
        "apple_music" => value.len() >= 20,
        "soundcloud" => value.len() >= 20 && !value.chars().any(char::is_whitespace),
        "qobuz" => {
            // Espejo de `is_viable_qobuz_token_auth` (commands::auth): sin
            // JSON serializado, sin espacios, longitud mínima.
            value.len() >= 16
                && !value.starts_with('{')
                && !value.starts_with('[')
                && !value.chars().any(char::is_whitespace)
        }
        _ => false,
    }
}

// ==============================================
// ARTEFACTO -> PAYLOAD DEL CAMINO EXISTENTE
// ==============================================

/// Traduce el artefacto capturado al payload que el puente habría devuelto en
/// `AuthResult.data`. SOLO claves de artefacto: nunca `password`, nunca el
/// correo del formulario (criterio 5 de #84).
pub fn artifact_payload(service: &str, artifact: &Value) -> Result<Value, String> {
    match service {
        "deezer" => {
            let arl = artifact
                .get("arl")
                .and_then(Value::as_str)
                .ok_or("Deezer artifact missing 'arl'")?;
            Ok(serde_json::json!({
                "message": "Connected to Deezer",
                "arl": arl,
                // La compatibilidad histórica usa ARL como access_token.
                "access_token": arl,
            }))
        }
        "qobuz" => {
            let token = artifact
                .get("user_auth_token")
                .and_then(Value::as_str)
                .ok_or("Qobuz artifact missing 'user_auth_token'")?;
            Ok(serde_json::json!({
                "user_auth_token": token,
                "auth_token": token,
            }))
        }
        "soundcloud" => {
            let raw = artifact
                .get("oauth_token")
                .and_then(Value::as_str)
                .ok_or("SoundCloud artifact missing 'oauth_token'")?;
            let prefixed = if raw.starts_with("OAuth ") {
                raw.to_string()
            } else {
                format!("OAuth {}", raw.trim())
            };
            Ok(serde_json::json!({
                "oauth_token": prefixed,
                "access_token": prefixed.trim_start_matches("OAuth ").to_string(),
            }))
        }
        "apple_music" => {
            let music_user_token = artifact
                .get("music_user_token")
                .and_then(Value::as_str)
                .ok_or("Apple Music artifact missing 'music_user_token'")?;
            let mut payload = serde_json::json!({
                "message": "Connected to Apple Music",
                "music_user_token": music_user_token,
            });
            if let Some(dev) = artifact.get("developer_token").and_then(Value::as_str) {
                payload["developer_token"] = Value::String(dev.to_string());
            }
            Ok(payload)
        }
        other => Err(format!(
            "service '{}' does not map webview artifacts to bridge payloads",
            other
        )),
    }
}

// ==============================================
// REGISTRO DE INTENTOS Y LIMPIEZA
// ==============================================

/// Registro vivo de intentos. Un intento: ventana abierta con perfil
/// provisional y captura en curso. La entrada se RETIRA al finalizar
/// (completado, fallo o cancelación); la retirada atómica garantiza que la
/// limpieza y el evento solo ocurren una vez aunque la cancelación por comando
/// y el bucle de captura compitan.
static ATTEMPTS: LazyLock<Mutex<HashMap<String, AttemptRecord>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct AttemptRecord {
    window_label: String,
    profile_dir: PathBuf,
}

/// Directorio base de los perfiles de intento para esta instalación.
fn login_profiles_base(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    app.path()
        .app_local_data_dir()
        .map(|dir| dir.join(LOGIN_PROFILE_BASE))
        .map_err(|e| format!("Failed to resolve app local data dir: {}", e))
}

/// Crea el perfil provisional de un intento y lo registra. Devuelve el
/// `profile_key`, el directorio del perfil y el label de la ventana.
fn begin_attempt(app: &tauri::AppHandle) -> Result<(String, PathBuf, String), String> {
    {
        let attempts = ATTEMPTS
            .lock()
            .map_err(|e| format!("Login attempt registry poisoned: {}", e))?;
        if !attempts.is_empty() {
            return Err(
                "A login window is already open. Close it or cancel the attempt first.".to_string(),
            );
        }
    }

    let profile_key = uuid::Uuid::new_v4().simple().to_string();
    let base = login_profiles_base(app)?;
    let profile_dir = base.join(&profile_key);
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("Failed to create login profile directory: {}", e))?;

    let window_label = format!("login-{}", &profile_key[..12]);
    ATTEMPTS
        .lock()
        .map_err(|e| format!("Login attempt registry poisoned: {}", e))?
        .insert(
            profile_key.clone(),
            AttemptRecord {
                window_label: window_label.clone(),
                profile_dir: profile_dir.clone(),
            },
        );
    Ok((profile_key, profile_dir, window_label))
}

/// Borra el perfil provisional de un intento. Best-effort: un fallo de borrado
/// se registra (nunca silencioso), pero no deshace el final del intento.
fn cleanup_profile_dir(dir: &std::path::Path) {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(
            "Failed to remove temporary login profile directory {}: {}",
            dir.display(),
            e
        ),
    }
}

/// Cierra un intento: retira el registro (transición única), borra el perfil
/// provisional y emite el evento del resultado. Devuelve `true` solo si ESTA
/// llamada hizo la transición.
fn finalize_attempt(
    profile_key: &str,
    outcome: AttemptOutcome,
    account_id: Option<i64>,
    error: Option<&str>,
) -> bool {
    let Some(record) = take_attempt(profile_key) else {
        // Otro camino ya finalizó (p. ej. cancel_account_login).
        return false;
    };

    cleanup_profile_dir(&record.profile_dir);

    if let Some(app) = super::get_global_app_handle() {
        let mut payload = serde_json::json!({
            "profile_key": profile_key,
        });
        if let Some(id) = account_id {
            payload["account_id"] = Value::from(id);
        }
        if let Some(err) = error {
            // Redactado por la misma regla que el resto del auth: el error puede
            // citar la salida del puente y no debe llevar tokens.
            payload["error"] = Value::String(crate::commands::auth::redact_auth_payload(err));
        }
        let event = match outcome {
            AttemptOutcome::Completed => "account-login-completed",
            AttemptOutcome::Cancelled => "account-login-cancelled",
            AttemptOutcome::Failed => "account-login-failed",
        };
        let _ = app.emit(event, &payload);
    }
    true
}

/// Retira el registro del intento (transición atómica); `None` si ya no existe.
fn take_attempt(profile_key: &str) -> Option<AttemptRecord> {
    let mut attempts = ATTEMPTS.lock().ok()?;
    attempts.remove(profile_key)
}

enum AttemptOutcome {
    Completed,
    Cancelled,
    Failed,
}

/// `true` si la ventana del intento sigue abierta.
fn window_is_open(app: &tauri::AppHandle, label: &str) -> bool {
    use tauri::Manager;
    app.get_webview_window(label).is_some()
}

/// Cierra la ventana de un intento si sigue abierta.
fn close_attempt_window(app: &tauri::AppHandle, label: &str) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    }
}

// ==============================================
// BUCLES DE CAPTURA
// ==============================================

/// Eco del título: ejecuta el script de sondeo y lee el resultado si apareció.
async fn poll_js_artifact(window: &tauri::WebviewWindow, poll_script: &str) -> Option<Value> {
    window.eval(poll_script).ok()?;
    tokio::time::sleep(EVAL_ECHO_INTERVAL).await;
    let title = window.title().ok()?;
    let raw = title.strip_prefix(ARTIFACT_TITLE_PREFIX)?;
    serde_json::from_str::<Value>(raw).ok()
}

/// Captura de una cookie del jar nativo. `None` = la ventana se cerró antes de
/// capturar (cancelación por parte del usuario).
async fn capture_cookie(
    app: &tauri::AppHandle,
    window_label: &str,
    scope_url: tauri::Url,
    name: &str,
    min_len: usize,
    validate: impl Fn(&str) -> bool,
) -> Option<String> {
    loop {
        if !window_is_open(app, window_label) {
            return None;
        }
        if let Some(window) = app.get_webview_window(window_label) {
            if let Ok(cookies) = window.cookies_for_url(scope_url.clone()) {
                if let Some(cookie) = cookies.iter().find(|c| c.name() == name) {
                    let value = cookie.value();
                    if value.len() >= min_len && validate(value) {
                        return Some(value.to_string());
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Captura por JavaScript (eco del título). `None` = ventana cerrada.
async fn capture_js(
    app: &tauri::AppHandle,
    window_label: &str,
    poll_script: &str,
    validate: impl Fn(&Value) -> bool,
) -> Option<Value> {
    loop {
        if !window_is_open(app, window_label) {
            return None;
        }
        if let Some(window) = app.get_webview_window(window_label) {
            if let Some(value) = poll_js_artifact(&window, poll_script).await {
                if validate(&value) {
                    return Some(value);
                }
            }
        }
        tokio::time::sleep(EVAL_ECHO_INTERVAL).await;
    }
}

/// Watcher de cierre de ventana para `tokio::select!`.
async fn wait_window_closed(app: tauri::AppHandle, window_label: String) {
    loop {
        if !window_is_open(&app, &window_label) {
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

// ==============================================
// FLUJOS POR DESCRIPTOR
// ==============================================

/// Clave obligatoria del artefacto JS por servicio.
fn artifact_key(service: &str) -> &'static str {
    match service {
        "qobuz" => "user_auth_token",
        "soundcloud" => "oauth_token",
        _ => "music_user_token",
    }
}

/// Ejecuta el intento completo para un descriptor cookie/JS: captura, cierre de
/// ventana, guardado por el camino existente y finalización.
async fn run_web_artifact_attempt(
    app: tauri::AppHandle,
    db: sqlx::SqlitePool,
    descriptor: &'static LoginDescriptor,
    profile_key: String,
    window_label: String,
) {
    let service = descriptor.service;

    let artifact = match &descriptor.artifact {
        ArtifactSpec::Cookie {
            name,
            scope_url,
            min_len,
            ..
        } => {
            let Ok(scope) = tauri::Url::parse(scope_url) else {
                finalize_attempt(
                    &profile_key,
                    AttemptOutcome::Failed,
                    None,
                    Some(&format!("invalid cookie scope URL: {}", scope_url)),
                );
                return;
            };
            capture_cookie(&app, &window_label, scope, name, *min_len, |v| {
                valid_artifact_value(service, v)
            })
            .await
            .map(|value| {
                let mut artifact = serde_json::Map::new();
                artifact.insert((*name).to_string(), Value::String(value));
                Value::Object(artifact)
            })
        }
        ArtifactSpec::JavaScript { poll_script, .. } => {
            capture_js(&app, &window_label, poll_script, |value| {
                value
                    .get(artifact_key(service))
                    .and_then(Value::as_str)
                    .is_some_and(|v| valid_artifact_value(service, v))
            })
            .await
        }
        ArtifactSpec::CookieWithJavaScript {
            cookie_name,
            scope_url,
            min_len,
            poll_script,
            ..
        } => {
            capture_cookie_with_js(
                &app,
                &window_label,
                scope_url,
                cookie_name,
                *min_len,
                poll_script,
            )
            .await
        }
        ArtifactSpec::DeviceCode | ArtifactSpec::PkceOAuth => {
            // Manejados por run_account_login; este brazo no se alcanza.
            return;
        }
    };

    // La ventana cumplió su propósito en cuanto el artefacto se capturó.
    close_attempt_window(&app, &window_label);

    let Some(artifact) = artifact else {
        // El usuario cerró la ventana: cancelación limpia, sin cuenta ni perfil
        // a medias.
        finalize_attempt(&profile_key, AttemptOutcome::Cancelled, None, None);
        return;
    };

    let payload = match artifact_payload(service, &artifact) {
        Ok(payload) => payload,
        Err(e) => {
            finalize_attempt(&profile_key, AttemptOutcome::Failed, None, Some(e.as_str()));
            return;
        }
    };

    match crate::commands::auth::save_auth_result(service, payload, &db).await {
        Ok(result) => {
            if result.success {
                let account_id = result
                    .data
                    .as_ref()
                    .and_then(|d| d.get("account_id"))
                    .and_then(Value::as_i64);
                finalize_attempt(&profile_key, AttemptOutcome::Completed, account_id, None);
            } else {
                finalize_attempt(
                    &profile_key,
                    AttemptOutcome::Failed,
                    None,
                    result.error.as_deref(),
                );
            }
        }
        Err(e) => {
            finalize_attempt(&profile_key, AttemptOutcome::Failed, None, Some(e.as_str()));
        }
    }
}

/// Apple Music: cookie `media-user-token` requerida del jar nativo más el
/// developer token opcional capturado por JS. `None` = ventana cerrada.
async fn capture_cookie_with_js(
    app: &tauri::AppHandle,
    window_label: &str,
    scope_url: &str,
    cookie_name: &str,
    min_len: usize,
    poll_script: &str,
) -> Option<Value> {
    let Ok(scope) = tauri::Url::parse(scope_url) else {
        return None;
    };
    let mut developer_token: Option<String> = None;
    loop {
        if !window_is_open(app, window_label) {
            return None;
        }
        if let Some(window) = app.get_webview_window(window_label) {
            // El developer token es opcional: mejor esfuerzo en cada vuelta.
            if developer_token.is_none() {
                if let Some(value) = poll_js_artifact(&window, poll_script).await {
                    developer_token = value
                        .get("developer_token")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            if let Ok(cookies) = window.cookies_for_url(scope.clone()) {
                if let Some(cookie) = cookies.iter().find(|c| c.name() == cookie_name) {
                    let value = cookie.value();
                    if value.len() >= min_len && valid_artifact_value("apple_music", value) {
                        let mut artifact = serde_json::Map::new();
                        artifact.insert(cookie_name.to_string(), Value::String(value.to_string()));
                        if let Some(dev) = &developer_token {
                            artifact
                                .insert("developer_token".to_string(), Value::String(dev.clone()));
                        }
                        return Some(Value::Object(artifact));
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Tidal: el puente inicia el device code (`login_start`), el webview muestra
/// la URL de verificación completa (el consentimiento NO salta a un navegador
/// externo) y el puente sondea los tokens (`login_poll`). El artefacto son los
/// tokens del puente: el webview no captura nada de la página.
async fn run_tidal_device_code_attempt(
    app: tauri::AppHandle,
    db: sqlx::SqlitePool,
    profile_key: String,
    window_label: String,
    profile_dir: PathBuf,
) {
    // 1. El puente pide el device code. Sin navegador: solo HTTP.
    let started = match run_auth_bridge_subprocess("tidal", "login_start", None).await {
        Ok(result) if result.success => result,
        Ok(result) => {
            close_attempt_window(&app, &window_label);
            finalize_attempt(
                &profile_key,
                AttemptOutcome::Failed,
                None,
                result.error.as_deref(),
            );
            return;
        }
        Err(e) => {
            close_attempt_window(&app, &window_label);
            finalize_attempt(&profile_key, AttemptOutcome::Failed, None, Some(e.as_str()));
            return;
        }
    };

    let Some(start_data) = started.data else {
        close_attempt_window(&app, &window_label);
        finalize_attempt(
            &profile_key,
            AttemptOutcome::Failed,
            None,
            Some("Tidal device code flow returned no data"),
        );
        return;
    };

    let verification_url = start_data
        .get("verification_url")
        .and_then(Value::as_str)
        .unwrap_or("https://link.tidal.com/");
    let device_code = start_data
        .get("device_code")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    // 2. Ventana con el perfil del intento en la URL de verificación completa.
    let Ok(url) = tauri::Url::parse(verification_url) else {
        close_attempt_window(&app, &window_label);
        finalize_attempt(
            &profile_key,
            AttemptOutcome::Failed,
            None,
            Some(&format!(
                "invalid Tidal verification URL: {}",
                verification_url
            )),
        );
        return;
    };
    if build_login_window(
        &app,
        &window_label,
        "Connect Tidal",
        &url,
        &profile_dir,
        None,
    )
    .is_err()
    {
        finalize_attempt(
            &profile_key,
            AttemptOutcome::Failed,
            None,
            Some("Failed to create Tidal login window"),
        );
        return;
    }
    emit_login_started("tidal", &profile_key);

    // 3. Sondeo del puente en paralelo con el watcher de cierre de ventana:
    //    cerrar la ventana aborta el puente (kill_on_drop) y cancela limpio.
    let poll_payload = serde_json::json!({ "device_code": device_code }).to_string();
    tokio::select! {
        bridge = run_auth_bridge_subprocess("tidal", "login_poll", Some(&poll_payload)) => {
            close_attempt_window(&app, &window_label);
            match bridge {
                Ok(result) if result.success => {
                    let payload = result.data.clone().unwrap_or(Value::Null);
                    match crate::commands::auth::save_auth_result("tidal", payload, &db).await {
                        Ok(saved) if saved.success => {
                            let account_id = saved
                                .data
                                .as_ref()
                                .and_then(|d| d.get("account_id"))
                                .and_then(Value::as_i64);
                            finalize_attempt(&profile_key, AttemptOutcome::Completed, account_id, None);
                        }
                        Ok(saved) => {
                            finalize_attempt(
                                &profile_key,
                                AttemptOutcome::Failed,
                                None,
                                saved.error.as_deref(),
                            );
                        }
                        Err(e) => {
                            finalize_attempt(&profile_key, AttemptOutcome::Failed, None, Some(e.as_str()));
                        }
                    }
                }
                Ok(result) => {
                    finalize_attempt(
                        &profile_key,
                        AttemptOutcome::Failed,
                        None,
                        result.error.as_deref(),
                    );
                }
                Err(e) => {
                    finalize_attempt(&profile_key, AttemptOutcome::Failed, None, Some(e.as_str()));
                }
            }
        }
        _ = wait_window_closed(app.clone(), window_label.clone()) => {
            // Cerrada por el usuario: el futuro del puente se cancela aquí.
            finalize_attempt(&profile_key, AttemptOutcome::Cancelled, None, None);
        }
    }
}

/// Spotify: el runner PKCE existente, con la ventana y el perfil del intento.
async fn run_spotify_attempt(
    app: tauri::AppHandle,
    db: sqlx::SqlitePool,
    profile_key: String,
    window_label: String,
    profile_dir: PathBuf,
) {
    let spec = crate::commands::auth::SpotifyAuthWindowSpec {
        label: window_label.clone(),
        title: "Connect Spotify",
        data_directory: Some(profile_dir.clone()),
    };
    emit_login_started("spotify", &profile_key);

    let result = crate::commands::auth::run_spotify_pkce_login(app.clone(), db, spec).await;
    let outcome = match result {
        Ok(result) if result.success => AttemptOutcome::Completed,
        Ok(_) => AttemptOutcome::Failed,
        Err(_) if !window_is_open(&app, &window_label) => {
            // El runner terminó sin callback porque el usuario cerró la
            // ventana: cancelación, no fallo.
            AttemptOutcome::Cancelled
        }
        Err(_) => AttemptOutcome::Failed,
    };
    match outcome {
        AttemptOutcome::Completed => {
            finalize_attempt(&profile_key, AttemptOutcome::Completed, None, None);
        }
        AttemptOutcome::Cancelled => {
            finalize_attempt(&profile_key, AttemptOutcome::Cancelled, None, None);
        }
        AttemptOutcome::Failed => {
            finalize_attempt(&profile_key, AttemptOutcome::Failed, None, None);
        }
    }
}

// ==============================================
// VENTANA DE LOGIN
// ==============================================

/// Abre la ventana de webview del intento con su perfil provisional.
fn build_login_window(
    app: &tauri::AppHandle,
    label: &str,
    title: &str,
    url: &tauri::Url,
    data_directory: &std::path::Path,
    init_script: Option<&str>,
) -> Result<tauri::WebviewWindow, String> {
    let mut builder =
        tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::External(url.clone()))
            .title(title)
            .inner_size(500.0, 700.0)
            .resizable(false)
            .closable(true)
            .data_directory(data_directory.to_path_buf());
    if let Some(script) = init_script {
        builder = builder.initialization_script(script);
    }
    builder
        .build()
        .map_err(|e| format!("Failed to create login window: {}", e))
}

fn emit_login_started(service: &str, profile_key: &str) {
    if let Some(app) = super::get_global_app_handle() {
        let _ = app.emit(
            "account-login-started",
            serde_json::json!({ "service": service, "profile_key": profile_key }),
        );
    }
}

// ==============================================
// COMANDOS IPC
// ==============================================

/// Inicia el login dentro de la app para un servicio: abre la ventana de
/// webview con un `profile_key` por intento y lanza la captura del artefacto.
/// Devuelve enseguida; el resultado llega por los eventos
/// `account-login-completed` / `account-login-failed` /
/// `account-login-cancelled`.
///
/// El artefacto capturado entra al camino existente de guardado
/// (`commands::auth::save_auth_result`): cifrado, upsert por identidad y
/// re-encola de descargas idénticos a los del puente.
#[tauri::command]
pub async fn start_account_login(
    service: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Value, String> {
    let Some(descriptor) = login_descriptor(&service) else {
        return Err(format!(
            "No login descriptor for service '{}': in-app login is only available for the six integrated services",
            service
        ));
    };
    let service_static = descriptor.service;

    let (profile_key, profile_dir, window_label) = begin_attempt(&app)?;
    let db = state.db.clone();

    match &descriptor.artifact {
        ArtifactSpec::PkceOAuth => {
            // El runner PKCE abre su propia ventana con el perfil del intento.
            tokio::spawn(run_spotify_attempt(
                app,
                db,
                profile_key.clone(),
                window_label,
                profile_dir,
            ));
        }
        ArtifactSpec::DeviceCode => {
            // El puente pide el device code ANTES de abrir la ventana; si
            // falla, el intento termina limpio sin ventana.
            tokio::spawn(run_tidal_device_code_attempt(
                app,
                db,
                profile_key.clone(),
                window_label,
                profile_dir,
            ));
        }
        ArtifactSpec::Cookie { .. }
        | ArtifactSpec::JavaScript { .. }
        | ArtifactSpec::CookieWithJavaScript { .. } => {
            let Ok(url) = tauri::Url::parse(descriptor.entry_url) else {
                let message = format!("Invalid login entry URL for {}", service);
                cleanup_profile_dir(&profile_dir);
                take_attempt(&profile_key);
                return Err(message);
            };
            let init_script = match &descriptor.artifact {
                ArtifactSpec::JavaScript { init_script, .. } => Some(*init_script),
                ArtifactSpec::CookieWithJavaScript { init_script, .. } => Some(*init_script),
                _ => None,
            };
            if let Err(e) = build_login_window(
                &app,
                &window_label,
                descriptor.window_title,
                &url,
                &profile_dir,
                init_script,
            ) {
                cleanup_profile_dir(&profile_dir);
                take_attempt(&profile_key);
                return Err(e);
            }
            emit_login_started(service_static, &profile_key);

            tokio::spawn(run_web_artifact_attempt(
                app,
                db,
                descriptor,
                profile_key.clone(),
                window_label,
            ));
        }
    }

    Ok(serde_json::json!({
        "service": service_static,
        "profile_key": profile_key,
    }))
}

/// Cancela un intento de login: cierra la ventana, borra el perfil provisional
/// y emite `account-login-cancelled`. Sin cuenta ni perfil a medias.
#[tauri::command]
pub async fn cancel_account_login(profile_key: String) -> Result<Value, String> {
    let record = ATTEMPTS
        .lock()
        .map_err(|e| format!("Login attempt registry poisoned: {}", e))?
        .get(&profile_key)
        .map(|r| (r.window_label.clone(), r.profile_dir.clone()));

    let Some((window_label, profile_dir)) = record else {
        return Err(format!(
            "No active login attempt for profile_key '{}'",
            profile_key
        ));
    };

    if let Some(app) = super::get_global_app_handle() {
        close_attempt_window(app, &window_label);
    }
    cleanup_profile_dir(&profile_dir);
    // La transición atómica garantiza un solo evento: si el bucle de captura
    // ya finalizó el intento al ver la ventana cerrada, esto no duplica nada.
    finalize_attempt(&profile_key, AttemptOutcome::Cancelled, None, None);

    Ok(serde_json::json!({ "cancelled": true, "profile_key": profile_key }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Descriptores ────────────────────────────────────────────────────────

    #[test]
    fn los_seis_servicios_tienen_descriptor() {
        for service in [
            "spotify",
            "qobuz",
            "tidal",
            "deezer",
            "soundcloud",
            "apple_music",
        ] {
            let d = login_descriptor(service)
                .unwrap_or_else(|| panic!("falta descriptor para {}", service));
            assert_eq!(d.service, service);
            assert!(
                d.entry_url.starts_with("https://"),
                "{}: URL https",
                service
            );
            assert!(!d.window_title.is_empty());
        }
    }

    #[test]
    fn descriptores_unicos_y_sin_servicio_desconocido() {
        let mut names: Vec<_> = SERVICE_DESCRIPTORS.iter().map(|d| d.service).collect();
        names.sort_unstable();
        assert_eq!(names.len(), 6);
        names.dedup();
        assert_eq!(names.len(), 6, "descriptores duplicados");
        assert!(login_descriptor("no_existe").is_none());
        assert!(login_descriptor("").is_none());
        // Normalización del nombre de servicio.
        assert!(login_descriptor("  Apple_Music ").is_some());
    }

    #[test]
    fn descriptores_de_artefacto_contra_el_proveedor() {
        // Deezer: cookie ARL del jar nativo (la HttpOnly de la sonda #63).
        let deezer = &login_descriptor("deezer").unwrap().artifact;
        match deezer {
            ArtifactSpec::Cookie { name, min_len, .. } => {
                assert_eq!(*name, "arl");
                assert!(*min_len > 100);
            }
            _ => panic!("deezer debe capturar la cookie arl"),
        }
        // Apple Music: cookie media-user-token + developer token opcional.
        let apple = &login_descriptor("apple_music").unwrap().artifact;
        match apple {
            ArtifactSpec::CookieWithJavaScript { cookie_name, .. } => {
                assert_eq!(*cookie_name, "media-user-token");
            }
            _ => panic!("apple_music debe capturar cookie + js"),
        }
        // Qobuz y SoundCloud: clave por JavaScript.
        for service in ["qobuz", "soundcloud"] {
            let spec = &login_descriptor(service).unwrap().artifact;
            match spec {
                ArtifactSpec::JavaScript {
                    init_script,
                    poll_script,
                } => {
                    assert!(!init_script.is_empty());
                    assert!(poll_script.contains("SYNCIFY_ARTIFACT::"));
                }
                _ => panic!("{} debe capturar por JavaScript", service),
            }
        }
        // Tidal: tokens del puente, webview solo muestra la verificación.
        assert!(matches!(
            login_descriptor("tidal").unwrap().artifact,
            ArtifactSpec::DeviceCode
        ));
        // Spotify: PKCE dentro del webview.
        assert!(matches!(
            login_descriptor("spotify").unwrap().artifact,
            ArtifactSpec::PkceOAuth
        ));
    }

    // ── Criterio 5: sin contraseña ni correo ────────────────────────────────

    /// Los scripts de captura nunca leen formularios: ni `password`, ni campos
    /// de email, ni ningún sondeo de inputs.
    #[test]
    fn los_scripts_de_captura_no_leen_credenciales_del_formulario() {
        let scripts = [
            AUTH_HEADER_HOOK_INIT,
            QOBUZ_POLL_SCRIPT,
            SOUNDCLOUD_POLL_SCRIPT,
            APPLE_POLL_SCRIPT,
        ];
        for script in scripts {
            let lowered = script.to_ascii_lowercase();
            assert!(!lowered.contains("password"), "script lee password");
            assert!(
                !lowered.contains("input"),
                "el script no debe tocar campos del formulario"
            );
            assert!(
                !lowered.contains("queryselector"),
                "no sondeo el DOM del formulario"
            );
        }
    }

    // ── Artefacto falso -> payload del camino existente ────────────────────

    #[test]
    fn artefacto_falso_deezer_produce_payload_de_puente() {
        assert!(valid_artifact_value("deezer", &"x".repeat(150)));
        assert!(!valid_artifact_value("deezer", &"x".repeat(50)));
        let payload = artifact_payload("deezer", &serde_json::json!({ "arl": "x".repeat(150) }))
            .expect("payload deezer");
        assert_eq!(payload["access_token"], payload["arl"]);
        assert!(payload["arl"].as_str().unwrap().starts_with("xxx"));
    }

    #[test]
    fn artefacto_falso_qobuz_pasa_el_filtro_de_viabilidad() {
        let fake = "q".repeat(20);
        assert!(valid_artifact_value("qobuz", &fake));
        // Un JSON serializado (token no viable) no pasa.
        assert!(!valid_artifact_value("qobuz", "{\"v\":29}"));
        assert!(!valid_artifact_value("qobuz", "corto"));
        assert!(!valid_artifact_value("qobuz", "con espacio interior"));
        let payload =
            artifact_payload("qobuz", &serde_json::json!({ "user_auth_token": fake })).unwrap();
        assert_eq!(payload["user_auth_token"], payload["auth_token"]);
    }

    #[test]
    fn artefacto_falso_soundcloud_normaliza_el_prefijo_oauth() {
        assert!(valid_artifact_value("soundcloud", &"s".repeat(30)));
        assert!(!valid_artifact_value("soundcloud", &"s".repeat(10)));
        let payload = artifact_payload(
            "soundcloud",
            &serde_json::json!({ "oauth_token": "c".repeat(30) }),
        )
        .unwrap();
        assert_eq!(payload["oauth_token"], format!("OAuth {}", "c".repeat(30)));
        assert_eq!(payload["access_token"], "c".repeat(30));
        // Ya prefijado no se duplica.
        let payload = artifact_payload(
            "soundcloud",
            &serde_json::json!({ "oauth_token": format!("OAuth {}", "c".repeat(30)) }),
        )
        .unwrap();
        assert_eq!(payload["oauth_token"], format!("OAuth {}", "c".repeat(30)));
    }

    #[test]
    fn artefacto_falso_apple_music_lleva_mut_y_dev_token_opcional() {
        let fake = "m".repeat(25);
        assert!(valid_artifact_value("apple_music", &fake));
        assert!(!valid_artifact_value("apple_music", "corto"));
        let payload = artifact_payload(
            "apple_music",
            &serde_json::json!({
                "music_user_token": fake,
                "developer_token": format!("eyJ{}", "d".repeat(60))
            }),
        )
        .unwrap();
        assert_eq!(payload["music_user_token"], fake);
        assert!(payload["developer_token"]
            .as_str()
            .unwrap()
            .starts_with("eyJ"));
        // Sin developer token el payload sigue siendo válido.
        let payload = artifact_payload(
            "apple_music",
            &serde_json::json!({ "music_user_token": fake }),
        )
        .unwrap();
        assert!(payload.get("developer_token").is_none());
    }

    #[test]
    fn ningun_payload_lleva_password_ni_correo() {
        for (service, artifact) in [
            ("deezer", serde_json::json!({ "arl": "x".repeat(150) })),
            (
                "qobuz",
                serde_json::json!({ "user_auth_token": "q".repeat(20) }),
            ),
            (
                "soundcloud",
                serde_json::json!({ "oauth_token": "c".repeat(30) }),
            ),
            (
                "apple_music",
                serde_json::json!({ "music_user_token": "m".repeat(25) }),
            ),
        ] {
            let payload = artifact_payload(service, &artifact).unwrap();
            let text = payload.to_string().to_ascii_lowercase();
            assert!(
                !text.contains("password"),
                "{}: payload con password",
                service
            );
            assert!(!text.contains("email"), "{}: payload con email", service);
        }
        // Un servicio sin builder explícito se rechaza: nada de passthrough.
        assert!(artifact_payload("no_existe", &serde_json::json!({})).is_err());
    }

    // ── Cancelación y limpieza con fixture ─────────────────────────────────

    #[test]
    fn cleanup_borra_el_perfil_provisional_y_tolera_doble_llamada() {
        let dir = std::env::temp_dir().join(format!("syncify-a84-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cookies"), b"fixture").unwrap();

        cleanup_profile_dir(&dir);
        assert!(!dir.exists(), "el perfil provisional se borra al cancelar");

        // Segunda llamada (ya inexistente) no falla ni hace ruido.
        cleanup_profile_dir(&dir);
        assert!(!dir.exists());
    }

    #[test]
    fn el_prefijo_del_eco_no_es_confundible_con_un_titulo_de_pagina() {
        // Un título del proveedor jamás empieza por el centinela.
        assert!("Qobuz — Music"
            .strip_prefix(ARTIFACT_TITLE_PREFIX)
            .is_none());
        // Y el eco del centinela se parsea como JSON de artefacto.
        let title = format!(
            "{}{{\"user_auth_token\":\"{}\"}}",
            ARTIFACT_TITLE_PREFIX,
            "q".repeat(20)
        );
        let value: Value =
            serde_json::from_str(title.strip_prefix(ARTIFACT_TITLE_PREFIX).unwrap()).unwrap();
        assert_eq!(value["user_auth_token"], "q".repeat(20));
    }
}
