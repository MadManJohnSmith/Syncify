use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// External tools the app spawns by bare name. When a copy ships with the
/// application (installer, AppImage, DEB, portable, tarball: beside the
/// executable or in a `bin/` directory), that copy is used before the OS
/// falls back to `PATH`.
pub const EXTERNAL_TOOLS: &[&str] = &["ffmpeg", "ffprobe", "fpcalc", "flac"];

/// Env override for a tool, e.g. `ffmpeg` -> `FFMPEG_PATH`.
fn tool_env_override(name: &str) -> Option<PathBuf> {
    std::env::var(format!("{}_PATH", name.to_uppercase()))
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
}

/// Candidate locations for a bundled tool: the executable directory, `bin/`
/// beside it, and the same two one level up (portable/tarball/repo layouts).
pub fn bundled_tool_candidates(name: &str) -> Vec<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let mut dir = exe_dir.to_path_buf();
            for _ in 0..2 {
                dirs.push(dir.clone());
                match dir.parent() {
                    Some(parent) => dir = parent.to_path_buf(),
                    None => break,
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    let mut candidates = Vec::new();
    for dir in dirs {
        candidates.push(dir.join(&file));
        candidates.push(dir.join("bin").join(&file));
    }
    candidates
}

/// Absolute path of an external tool available outside `PATH`: an explicit
/// override (`FFMPEG_PATH`, `FFPROBE_PATH`, `FPCALC_PATH`, `FLAC_PATH`), a
/// binary shipped with the app, or one in the repository `bin/` directory.
/// `None` lets the OS resolve the bare name through `PATH`.
/// A candidate is usable only if it is an executable file: tauri-build copies
/// the `externalBin` placeholders next to the binary on every build
/// (`target/<profile>/ffmpeg`), and those placeholder copies carry no exec bit
/// while real bundled binaries do. Without this check the resolver would
/// prefer the inert placeholder over a working system `PATH` tool.
#[cfg(unix)]
pub fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

pub fn resolve_tool(name: &str) -> Option<PathBuf> {
    if !EXTERNAL_TOOLS.contains(&name) {
        return None;
    }
    if let Some(path) = tool_env_override(name) {
        return Some(path);
    }
    bundled_tool_candidates(name)
        .into_iter()
        .find(|p| is_executable_file(p))
}

/// Marker file that identifies the app's Python `scripts/` directory.
const SCRIPTS_DIR_MARKER: &str = "dependency_manager.py";

/// Directory name Tauri derives for the resource glob `../scripts/**/*.py`
/// relative to the binary directory (`../scripts` -> `<exe_dir>/_up_/scripts`).
const TAURI_UP_DIR: &str = "_up_";

/// Resolves the directory that holds the Python bridges (`*_bridge.py`,
/// `dependency_manager.py`, `requirements.txt`), or `None` if no packaged or
/// dev location can be validated.
///
/// Packaged layouts diverge from the dev tree: Tauri maps the resource glob
/// `../scripts/**/*.py` to `<exe_dir>/_up_/scripts` (Windows NSIS/portable)
/// or `<exe_dir>/../lib/<product>/_up_/scripts` (AppImage/DEB: binary in
/// `usr/bin`, resources in `usr/lib/<product>`), while the dev tree keeps
/// them at `<project_root>/scripts`. `get_project_root()` only walks UP the
/// tree, so it can never reach either packaged location — and its cwd
/// fallback (the AppRun sets cwd to `$APPDIR/usr`) yielded `usr/scripts`,
/// which does not exist. Every candidate is validated by the presence of the
/// marker bridge, so a hit cannot be a coincidence.
pub fn scripts_dir_from(exe_dir: Option<&Path>, env_override: Option<&str>) -> Option<PathBuf> {
    if let Some(dir) = env_override
        .map(PathBuf::from)
        .filter(|p| p.join(SCRIPTS_DIR_MARKER).is_file())
    {
        return Some(dir);
    }
    let exe_dir = exe_dir?;
    // Tauri resource layout: beside the executable (Windows NSIS/portable,
    // tarball raw binary).
    let mut candidates = vec![
        exe_dir.join(TAURI_UP_DIR).join("scripts"),
        exe_dir.join("scripts"),
    ];
    // AppImage/DEB: binary in `usr/bin`, resources in `usr/lib/<product>/_up_`.
    if let Some(lib_root) = exe_dir.parent().map(|p| p.join("lib")) {
        push_packaged_script_dirs(&lib_root, &mut candidates);
    }
    // AppRun.wrapped-at-root variant: `<appdir>/usr/lib/<product>/_up_`.
    let usr_lib = exe_dir.join("usr").join("lib");
    if usr_lib.is_dir() {
        push_packaged_script_dirs(&usr_lib, &mut candidates);
    }
    candidates
        .into_iter()
        .find(|dir| dir.join(SCRIPTS_DIR_MARKER).is_file())
}

fn push_packaged_script_dirs(lib_root: &Path, candidates: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(lib_root) {
        for entry in entries.flatten() {
            candidates.push(entry.path().join(TAURI_UP_DIR).join("scripts"));
        }
    }
}

/// Absolute path of the app's Python `scripts/` directory: packaged layouts
/// first (see [`scripts_dir_from`]), falling back to the dev tree's
/// `<project_root>/scripts` (the historical behavior).
pub fn find_scripts_dir(project_root: &Path) -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let env_override = std::env::var("SYNCIFY_SCRIPTS_DIR").ok();
    scripts_dir_from(exe_dir.as_deref(), env_override.as_deref())
        .unwrap_or_else(|| project_root.join("scripts"))
}

/// Bundled-Python candidates sitting next to the resolved `scripts/`
/// directory. Linux packages carry a self-contained CPython (with the
/// `requirements.txt` dependencies installed) at `<parent>/python`, mirroring
/// the `python/python.exe` embed Windows has always shipped: `<_up_>/python`
/// for AppImage/DEB resources, `<appdir>/resources/python` via the project
/// root, and `<tarball_root>/python` for the raw tarball. The caller must
/// still check each candidate for existence; the first hit wins.
pub fn packaged_python_candidates(scripts_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(base) = scripts_dir.parent() {
        candidates.push(if cfg!(windows) {
            base.join("python").join("python.exe")
        } else {
            base.join("python").join("bin").join("python")
        });
    }
    candidates
}

/// Resolves the program name when it is one of the external tools; anything
/// else (absolute paths, `python`, shell builtins) passes through untouched.
fn resolve_known_program<S: AsRef<OsStr>>(program: S) -> OsString {
    let name = program.as_ref().to_string_lossy().to_string();
    match resolve_tool(&name) {
        Some(path) => path.into_os_string(),
        None => program.as_ref().to_os_string(),
    }
}

/// Publishes every resolved tool as `NAME_PATH` so child processes (Python
/// bridges, pyacoustid) find them without a `PATH` installation.
pub trait CommandEnv {
    fn set_env(&mut self, key: String, value: PathBuf);
}

impl CommandEnv for std::process::Command {
    fn set_env(&mut self, key: String, value: PathBuf) {
        self.env(key, value);
    }
}

impl CommandEnv for tokio::process::Command {
    fn set_env(&mut self, key: String, value: PathBuf) {
        self.env(key, value);
    }
}

pub fn apply_bundled_tool_env<C: CommandEnv>(cmd: &mut C) {
    for tool in EXTERNAL_TOOLS {
        if let Some(path) = resolve_tool(tool) {
            cmd.set_env(format!("{}_PATH", tool.to_uppercase()), path);
        }
    }
}

/// Creates a `std::process::Command` configured with `CREATE_NO_WINDOW` on Windows
/// to prevent cmd.exe console popups.
#[allow(unused_mut)]
pub fn create_std_command<S: AsRef<OsStr>>(program: S) -> std::process::Command {
    let mut cmd = std::process::Command::new(resolve_known_program(program));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Creates a `tokio::process::Command` configured with `CREATE_NO_WINDOW` on Windows
/// to prevent cmd.exe console popups.
#[allow(unused_mut)]
pub fn create_tokio_command<S: AsRef<OsStr>>(program: S) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(resolve_known_program(program));
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Creates a `tokio::process::Command` configured with `PYTHONUNBUFFERED=1` and `PYTHONIOENCODING=utf-8`
#[allow(dead_code)] // Cubierta por `tests/cmd_utils_test.rs`.
pub fn create_python_tokio_command<S: AsRef<OsStr>>(
    program: S,
    scripts_dir: Option<&Path>,
) -> tokio::process::Command {
    let mut cmd = create_tokio_command(program);
    cmd.env("PYTHONUNBUFFERED", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    apply_bundled_tool_env(&mut cmd);
    if let Some(dir) = scripts_dir {
        cmd.env("PYTHONPATH", dir);
        cmd.current_dir(dir);
    }
    cmd
}

/// Creates a `std::process::Command` configured with Python unbuffered environment
#[allow(dead_code)] // Cubierta por `tests/cmd_utils_test.rs`.
pub fn create_python_std_command<S: AsRef<OsStr>>(
    program: S,
    scripts_dir: Option<&Path>,
) -> std::process::Command {
    let mut cmd = create_std_command(program);
    cmd.env("PYTHONUNBUFFERED", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    apply_bundled_tool_env(&mut cmd);
    if let Some(dir) = scripts_dir {
        cmd.env("PYTHONPATH", dir);
        cmd.current_dir(dir);
    }
    cmd
}

/// Default timeout for asynchronous bridge operations (45 seconds)
pub const DEFAULT_BRIDGE_TIMEOUT: Duration = Duration::from_secs(45);

/// Runs an asynchronous tokio process command with a timeout guard
#[allow(dead_code)] // Cubierta por `tests/cmd_utils_test.rs`.
pub async fn run_command_with_timeout(
    mut cmd: tokio::process::Command,
    timeout_duration: Duration,
) -> Result<std::process::Output, String> {
    match tokio::time::timeout(timeout_duration, cmd.output()).await {
        Ok(res) => res.map_err(|e| format!("Command execution failed: {}", e)),
        Err(_) => Err(format!(
            "Command timed out after {} seconds",
            timeout_duration.as_secs()
        )),
    }
}
