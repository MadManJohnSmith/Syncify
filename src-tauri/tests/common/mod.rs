//! Shared sandbox helpers for integration tests.
//!
//! `dirs::document_dir()`, `dirs::download_dir()` and `dirs::audio_dir()` only
//! read `$XDG_CONFIG_HOME/user-dirs.dirs` on Linux and return `None` when that
//! file is absent. The `ubuntu-latest` CI runner does not provision it, so
//! helpers that called `dirs::document_dir().expect(...)` panicked before
//! reaching a single assertion.
//!
//! These helpers derive the sandbox base from the production allowed-directory
//! resolvers instead of from the ambient XDG user directories, so a test always
//! writes inside the very sandbox whose confinement it asserts, on any runner.
//!
//! This module is compiled into every test binary that declares `mod common;`, and
//! each binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::PathBuf;

/// Returns the allowed base directories shared by every sandbox in `sandboxes`.
///
/// A base is kept only when the production resolvers report it for *all* the
/// sandboxes under test, so a single artifact directory is accepted by every
/// command the suite exercises. Bases that resolve from `$HOME` alone (the app
/// data directory and Music) are ranked as the fallback, because they exist on
/// a bare CI container where Documents and Downloads do not.
pub fn shared_allowed_bases(sandboxes: &[Vec<PathBuf>]) -> Vec<PathBuf> {
    if sandboxes.is_empty() {
        return Vec::new();
    }

    let mut common: Vec<PathBuf> = sandboxes[0]
        .iter()
        .filter(|base| sandboxes.iter().all(|sandbox| sandbox.contains(base)))
        .cloned()
        .collect();
    common.sort();
    common.dedup();
    common.sort_by_key(|base| {
        let name = base.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let rank = match name {
            "Documents" => 0,
            "Downloads" => 1,
            "com.syncify.app" => 2,
            "Music" => 3,
            _ => 4,
        };
        (rank, base.clone())
    });
    common
}

/// Resolves a writable, sandbox-compliant base directory for test artifacts.
///
/// Two families of candidates are tried, in order:
///
/// 1. a `target/` directory inside the checkout, used only when the checkout is
///    itself confined by the sandbox under test. It is always writable and keeps
///    artifacts out of the user's real data. On CI the checkout lives outside
///    every allowed base, so this candidate is skipped.
/// 2. each allowed base reported by the production resolvers, which is what a
///    bare CI runner has to offer: its `$HOME`-derived app data and Music
///    directories resolve without `$XDG_CONFIG_HOME/user-dirs.dirs`.
pub fn resolve_sandbox_test_base(sandboxes: &[Vec<PathBuf>], artifact_dir: &str) -> PathBuf {
    let bases = shared_allowed_bases(sandboxes);

    if let Ok(cwd) = std::env::current_dir() {
        for base in &bases {
            if cwd.starts_with(base) {
                let candidate = cwd.join("target").join(artifact_dir);
                if std::fs::create_dir_all(&candidate).is_ok() {
                    return candidate;
                }
            }
        }
    }

    for base in bases {
        let candidate = base.join(artifact_dir);
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }

    panic!("no writable sandbox base could be resolved for the confinement suite");
}

/// Returns a second writable location for test artifacts, distinct from
/// `primary`, so a suite can still assert that legitimate paths outside its
/// primary test directory are accepted. Falls back to a sibling of `primary`
/// when no further allowed base is writable.
pub fn resolve_secondary_sandbox_dir(
    primary: &PathBuf,
    sandboxes: &[Vec<PathBuf>],
    artifact_dir: &str,
) -> PathBuf {
    let bases = shared_allowed_bases(sandboxes);

    for base in &bases {
        if primary.starts_with(base) || base.starts_with(primary) {
            continue;
        }
        let candidate = base.join(artifact_dir);
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }

    if let Some(sibling) = primary.parent() {
        let candidate = sibling.join(format!("{}_secondary", artifact_dir));
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }

    primary.clone()
}
