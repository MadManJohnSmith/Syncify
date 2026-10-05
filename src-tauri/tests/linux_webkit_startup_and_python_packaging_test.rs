//! linux_webkit_startup_and_python_packaging_test.rs
//!
//! Congela los dos arreglos del carril Linux/paquetado:
//!
//! 1. R7 — el arranque en Linux no puede depender de que WebKitGTK consiga un
//!    display EGL. En el AppImage v0.3.0 el proceso moría entero con
//!    "Could not create default EGL display: EGL_BAD_PARAMETER. Aborting..."
//!    (Iris Xe / KDE Wayland). `WEBKIT_DISABLE_DMABUF_RENDERER` solo apaga el
//!    renderer DMABuf y NO basta: el que evita abrir EGL es
//!    `WEBKIT_DISABLE_COMPOSITING_MODE`. Las dos tienen que fijarse en el
//!    proceso, antes de que exista la webview, porque el proceso UI de WebKit
//!    hereda el entorno albornarse.
//!
//! 2. Auditoría 1 — el instalador NSIS de Windows tiene que llevar el runtime
//!    de Python embebido. `tauri.windows.conf.json` lo declara como recurso y
//!    Tauri lo instala en `<exe>/_up_/python/`, que es exactamente donde
//!    `cmd_utils::packaged_python_candidates` lo busca a partir del `scripts/`
//!    que resuelve `cmd_utils::scripts_dir_from`. Si esa cadena se rompe, la
//!    instalación limpia vuelve al Python del sistema en silencio.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use syncify_tauri_lib::cmd_utils::{packaged_python_candidates, scripts_dir_from};
use tempfile::TempDir;

fn src_tauri_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_src(relative: &str) -> String {
    let path = src_tauri_dir().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("No se pudo leer {}: {}", path.display(), e))
}

/// Traducción del recurso de Tauri al path dentro del paquete: cada `..` del
/// glob se convierte en el directorio `_up_` (`resource_relpath` en
/// tauri-utils). Se replica aquí para comprobar, sin depender de tauri-utils,
/// dónde acaba cada recurso que declara la configuración.
fn resource_relpath(path: &Path) -> PathBuf {
    let mut dest = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) => {}
            Component::RootDir => dest.push("_root_"),
            Component::CurDir => {}
            Component::ParentDir => dest.push("_up_"),
            Component::Normal(segment) => dest.push(segment),
        }
    }
    dest
}

// ═══════════════════════════════════════════════════════
// 1. ARRANQUE EN LINUX SIN EGL
// ═══════════════════════════════════════════════════════

#[test]
fn linux_startup_disables_dmabuf_and_compositing_before_the_webview_exists() {
    let src = read_src("src/main.rs");

    // Se buscan las cadenas entre comillas del array: el mismo nombre aparece
    // en el comentario que explica por qué están ahí.
    let dmabuf_at = src
        .find("\"WEBKIT_DISABLE_DMABUF_RENDERER\"")
        .expect("main.rs debe seguir desactivando el renderer DMABUF de WebKitGTK");
    let compositing_at = src
        .find("\"WEBKIT_DISABLE_COMPOSITING_MODE\"")
        .expect("main.rs debe desactivar WEBKIT_DISABLE_COMPOSITING_MODE: sin esto WebKitGTK sigue abriendo EGL y aborta el proceso");
    let builder_at = src
        .find("tauri::Builder::default()")
        .expect("main.rs debe seguir montando el Builder de Tauri");

    assert!(
        dmabuf_at < builder_at && compositing_at < builder_at,
        "Las variables de WebKitGTK se fijan en main() antes de tauri::Builder: el proceso UI de WebKit hereda el entorno al nacerse ({} / {} vs {})",
        dmabuf_at,
        compositing_at,
        builder_at
    );

    let linux_cfg_at = src[..dmabuf_at]
        .rfind("#[cfg(target_os = \"linux\")]")
        .expect("El bloque de WebKitGTK debe estar detrás de un #[cfg(target_os = \"linux\")]");
    assert!(
        dmabuf_at - linux_cfg_at < 200,
        "El cfg de Linux debe seguir pegado a las variables de WebKitGTK"
    );

    let block = &src[linux_cfg_at..builder_at];
    assert!(
        block.contains("set_var"),
        "Las variables se fijan con std::env::set_var dentro del bloque"
    );
}

#[test]
fn linux_startup_does_not_force_software_rendering_on_every_machine() {
    let src = read_src("src/main.rs");
    let webkit_block_end = src
        .find("tauri::Builder::default()")
        .expect("main.rs debe seguir montando el Builder de Tauri");
    let webkit_block = &src[..webkit_block_end];

    // Solo cuenta que se fije la variable: el comentario que explica por qué NO
    // se fuerza sí la nombra.
    assert!(
        !webkit_block.contains("set_var(\"LIBGL_ALWAYS_SOFTWARE\""),
        "Forzar llvmpipe globalmente degrada el render en las máquinas cuya GPU sí funciona: la salida del abort es WEBKIT_DISABLE_COMPOSITING_MODE"
    );
}

// ═══════════════════════════════════════════════════════
// 3. PLUGINS DE GSTREAMER DENTRO DEL APPIMAGE (auditoría 34)
// ═══════════════════════════════════════════════════════

/// Bloque de configuración de GStreamer dentro de `main.rs`: desde su banner
/// hasta el banner del bloque siguiente.
fn gstreamer_block() -> String {
    let src = read_src("src/main.rs");
    let start = src
        .find("GSTREAMER PLUGINS")
        .expect("main.rs debe seguir configurando GST_PLUGIN_SYSTEM_PATH_1_0");
    let end = src[start..]
        .find("NEUTRALIZE PYTHON ENV")
        .map(|offset| start + offset)
        .expect("El bloque de GStreamer debe seguir seguido por el de Python");
    src[start..end].to_string()
}

#[test]
fn appimage_still_injects_gstreamer_plugins_under_usr_lib() {
    let workflow_path = src_tauri_dir()
        .parent()
        .unwrap()
        .join(".github/workflows/build-linux.yml");
    let workflow = fs::read_to_string(&workflow_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {}: {}", workflow_path.display(), e));
    let plugdir = workflow
        .lines()
        .find(|line| line.trim_start().starts_with("PLUGDIR="))
        .expect("build-linux.yml debe seguir inyectando los plugins con PLUGDIR=");

    assert!(
        plugdir.contains("usr/lib/gstreamer-1.0"),
        "El AppImage guarda los plugins en <AppDir>/usr/lib/gstreamer-1.0, que es la ruta que main.rs tiene que buscar: {}",
        plugdir.trim()
    );
}

#[test]
fn gstreamer_search_reaches_the_appimage_layout_without_touching_system_dirs() {
    let block = gstreamer_block();

    // El ejecutable del AppImage vive en <AppDir>/usr/bin: los plugins están un
    // nivel más arriba, en <AppDir>/usr/lib, y NO cuelgan de `project_root`
    // (que resuelve a <AppDir>/resources por los scripts).
    assert!(
        block.contains("current_exe()"),
        "La búsqueda tiene que mirar junto al ejecutable, no solo junto a project_root"
    );
    assert!(
        block.contains("exe_dir.parent()"),
        "Sin exe_dir.parent() el AppImage sigue sin encontrar sus plugins"
    );
    assert!(
        block.contains(".join(\"lib\").join(\"gstreamer-1.0\")"),
        "El sufijo tiene que seguir siendo lib/gstreamer-1.0"
    );
    assert!(
        block.contains("starts_with(\"/usr\")"),
        "Un DEB instala el binario en /usr/bin: apuntar ahí la variable taparía las rutas por defecto de GStreamer"
    );
}

#[test]
fn gstreamer_env_var_is_only_set_to_a_directory_that_has_plugins() {
    let block = gstreamer_block();

    assert!(
        block.contains("read_dir("),
        "Un directorio de plugins vacío es peor que no tocar nada: GST_PLUGIN_SYSTEM_PATH_1_0 sustituye la lista por defecto"
    );
    assert!(
        !block.contains("find(|d| d.is_dir())"),
        "El criterio de selección sigue siendo `is_dir()`, que acepta un directorio vacío"
    );
}

// ═══════════════════════════════════════════════════════
// 2. PYTHON EMBEBIDO EN EL INSTALADOR DE WINDOWS
// ═══════════════════════════════════════════════════════

fn config_resources(name: &str) -> Vec<String> {
    let path = src_tauri_dir().join(name);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("No se pudo leer {}: {}", path.display(), e));
    let json: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("JSON inválido en {}: {}", path.display(), e));
    json.get("bundle")
        .and_then(|bundle| bundle.get("resources"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} debe declarar bundle.resources", path.display()))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("bundle.resources solo admite strings: {}", entry))
                .to_string()
        })
        .collect()
}

#[test]
fn windows_bundle_ships_the_embedded_python_runtime() {
    let resources = config_resources("tauri.windows.conf.json");

    assert!(
        resources.iter().any(|r| r.starts_with("../python/")),
        "El instalador de Windows tiene que empaquetar el runtime embebido: {:?}",
        resources
    );
    assert!(
        !resources.iter().any(|r| r == "../python"),
        "Un glob recursivo evita que un archivo suelto en la raíz de python/ quede fuera: {:?}",
        resources
    );
}

#[test]
fn windows_resources_keep_everything_the_shared_config_declares() {
    // La CLI fusiona `tauri.windows.conf.json` con json_patch::merge, que
    // SUSTITUYE los arrays: lo que no se repita aquí desaparece del instalador.
    let shared = config_resources("tauri.conf.json");
    let windows = config_resources("tauri.windows.conf.json");

    for entry in &shared {
        assert!(
            windows.contains(entry),
            "tauri.windows.conf.json reemplaza bundle.resources y se ha perdido {:?}",
            entry
        );
    }
}

#[test]
fn bundled_python_lands_where_the_resolver_looks_for_it_on_windows() {
    let temp = TempDir::new().unwrap();
    let exe_dir = temp.path().join("Syncify");
    let up_scripts = exe_dir.join("_up_").join("scripts");
    fs::create_dir_all(&up_scripts).unwrap();
    // Marcador con el que `scripts_dir_from` valida un candidate (cmd_utils.rs).
    fs::write(up_scripts.join("dependency_manager.py"), "").unwrap();

    let scripts_dir = scripts_dir_from(Some(&exe_dir), None)
        .expect("El layout del instalador (scripts en _up_/scripts) debe resolverse");
    assert_eq!(scripts_dir, up_scripts);

    // En Windows el candidato es `<padre del scripts>/python/python.exe`
    // (cmd_utils.rs, rama `cfg!(windows)` de packaged_python_candidates).
    let cmd_utils = read_src("src/cmd_utils.rs");
    assert!(
        cmd_utils.contains(r#"const TAURI_UP_DIR: &str = "_up_";"#),
        "cmd_utils.rs debe seguir mapeando `..` al directorio _up_ de Tauri"
    );
    let windows_branch = cmd_utils
        .split("pub fn packaged_python_candidates")
        .nth(1)
        .and_then(|tail| tail.split("candidates.push").nth(1))
        .expect("Debe existir packaged_python_candidates");
    assert!(
        windows_branch.contains(r#"base.join("python").join("python.exe")"#),
        "La rama de Windows de packaged_python_candidates debe seguir apuntando a <_up_>/python/python.exe: {:?}",
        windows_branch
    );

    // Y el recurso declarado en la configuración acaba exactamente ahí.
    let installed = resource_relpath(Path::new("../python/python.exe"));
    assert_eq!(
        installed,
        Path::new("_up_").join("python").join("python.exe")
    );
    assert_eq!(
        exe_dir.join(&installed),
        scripts_dir
            .parent()
            .unwrap()
            .join("python")
            .join("python.exe"),
        "El runtime instalado y el que busca el resolutor tienen que ser el mismo fichero"
    );

    // Comprobación funcional del resolutor con el layout real de la instalación.
    let candidates = packaged_python_candidates(&scripts_dir);
    let expected = if cfg!(windows) {
        exe_dir.join("_up_").join("python").join("python.exe")
    } else {
        exe_dir
            .join("_up_")
            .join("python")
            .join("bin")
            .join("python")
    };
    assert!(
        candidates.contains(&expected),
        "packaged_python_candidates debe ofrecer {:?}, offertas: {:?}",
        expected,
        candidates
    );
}

// ═══════════════════════════════════════════════════════
// 4. RECUPERACIÓN ADECUADA AL PAQUETE (auditoría 36)
// ═══════════════════════════════════════════════════════

#[test]
fn bundled_python_failure_does_not_ask_the_user_for_a_system_pip() {
    let src = read_src("src/main.rs");

    // La detección de runtime empaquetado tiene que usar el mismo resolutor que
    // usa la app, no una heurística nueva.
    assert!(
        src.contains("packaged_python_candidates("),
        "main.rs debe saber si el Python resuelto es el empaquetado antes de redactar la recuperación"
    );

    let recovery_start = src
        .find("let recovery_hint =")
        .expect("main.rs debe elegir el texto de recuperación según el runtime");
    let recovery_end = src[recovery_start..]
        .find("};")
        .map(|offset| recovery_start + offset)
        .expect("El bloque de recovery_hint debe terminar");
    let recovery = &src[recovery_start..recovery_end];

    let bundled_branch = recovery
        .split("if bundled_python {")
        .nth(1)
        .and_then(|tail| tail.split("} else {").next())
        .expect("recovery_hint debe distinguir el runtime empaquetado");
    assert!(
        bundled_branch.contains("reinstall the app"),
        "Al usuario del AppImage/instalador no le sirve un `pip install` en su máquina: {:?}",
        bundled_branch
    );
    assert!(
        !bundled_branch.contains("--break-system-packages"),
        "El texto del runtime empaquetado no debe pedir tocar el Python del sistema: {:?}",
        bundled_branch
    );
    assert!(
        src.contains("\"message\": recovery_hint"),
        "El evento python_deps_missing debe llevar la recuperación elegida, no una fija"
    );
}
