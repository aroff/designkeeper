//! Template pack resolution and installation.
//!
//! Resolution order per pack name:
//!   1. `.dk/packs/{name}/` walking up from cwd (project-local)
//!   2. `~/.dk/packs/{name}/` (user-global)
//!   3. Error: `PackStoreError::NotFound`

use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::config::find_up;
use crate::pack;
use crate::remote::{self, RemoteError};

// ---- Error ----

#[derive(Debug, Error)]
pub enum PackStoreError {
    #[error("template pack '{name}' is not installed. Run `dk install` first.")]
    NotFound { name: String },
    #[error("failed to install pack from '{source}': {cause}")]
    InstallFailed {
        source: String,
        #[source]
        cause: RemoteError,
    },
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

impl PackStoreError {
    pub fn code(&self) -> &'static str {
        match self {
            PackStoreError::NotFound { .. } => "DK_PACK_NOT_INSTALLED",
            PackStoreError::InstallFailed { .. } => "DK_PACK_INSTALL_FAILED",
            PackStoreError::Io(_) => "DK_IO_ERROR",
        }
    }
}

// ---- DkTemplatesManifest ----

/// Entry in `dk-templates.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PackEntry {
    pub name: String,
    pub description: String,
    pub source: String,
}

/// Parsed `dk-templates.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct DkTemplatesManifest {
    #[serde(rename = "packs")]
    pub packs: Vec<PackEntry>,
}

impl DkTemplatesManifest {
    pub fn parse(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Parse the embedded manifest bundled into the binary.
    pub fn embedded() -> Self {
        Self::parse(pack::DK_TEMPLATES_MANIFEST).expect("embedded dk-templates.toml must parse")
    }

    /// Walk up from `cwd` for a `dk-templates.toml`; fall back to embedded.
    pub fn resolve(cwd: &Path) -> Self {
        find_up(cwd, |dir| {
            let candidate = dir.join("dk-templates.toml");
            if candidate.is_file() {
                std::fs::read_to_string(&candidate)
                    .ok()
                    .and_then(|s| Self::parse(&s).ok())
            } else {
                None
            }
        })
        .unwrap_or_else(Self::embedded)
    }
}

// ---- Installed pack info ----

#[derive(Debug, Clone)]
pub struct InstalledPack {
    pub name: String,
    pub path: PathBuf,
    pub scope: PackScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackScope {
    Project,
    Global,
}

impl std::fmt::Display for PackScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackScope::Project => write!(f, "project"),
            PackScope::Global => write!(f, "global"),
        }
    }
}

// ---- Resolution ----

pub fn global_packs_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".dk").join("packs"))
}

fn project_pack_dir(name: &str, cwd: &Path) -> Option<PathBuf> {
    find_up(cwd, |dir| {
        let candidate = dir.join(".dk").join("packs").join(name);
        if pack::prompt_path(&candidate).is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

fn global_pack_dir(name: &str) -> Option<PathBuf> {
    global_packs_dir().and_then(|base| {
        let candidate = base.join(name);
        if pack::prompt_path(&candidate).is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

/// Resolve the directory for a named pack.
///
/// Tries project-local → global → error.
pub fn resolve_pack(name: &str, cwd: &Path) -> Result<PathBuf, PackStoreError> {
    if let Some(dir) = project_pack_dir(name, cwd) {
        return Ok(dir);
    }
    if let Some(dir) = global_pack_dir(name) {
        return Ok(dir);
    }
    Err(PackStoreError::NotFound {
        name: name.to_string(),
    })
}

// ---- Installation ----

/// Install a pack from `source_str` into `dest_base/{pack_name}/`.
pub fn install_pack(
    source_str: &str,
    dest_base: &Path,
    scope: PackScope,
) -> Result<InstalledPack, PackStoreError> {
    let pack_dir =
        remote::fetch_pack(source_str, dest_base).map_err(|e| PackStoreError::InstallFailed {
            source: source_str.to_string(),
            cause: e,
        })?;

    let name = pack_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(InstalledPack {
        name,
        path: pack_dir,
        scope,
    })
}

// ---- Listing ----

fn collect_packs_in(packs_dir: &Path, scope: PackScope) -> Vec<InstalledPack> {
    if !packs_dir.is_dir() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(packs_dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && pack::prompt_path(&e.path()).is_file()
        })
        .map(|e| InstalledPack {
            name: e.file_name().to_string_lossy().into_owned(),
            path: e.path(),
            scope: scope.clone(),
        })
        .collect()
}

/// List all installed packs (project-local and global), project-local wins on name conflict.
pub fn list_packs(cwd: &Path) -> Vec<InstalledPack> {
    let project_dk = find_up(cwd, |dir| {
        let dk = dir.join(".dk");
        if dk.is_dir() {
            Some(dk)
        } else {
            None
        }
    });

    let mut seen = std::collections::HashSet::new();
    let mut result = vec![];

    if let Some(dk) = project_dk {
        for p in collect_packs_in(&dk.join("packs"), PackScope::Project) {
            seen.insert(p.name.clone());
            result.push(p);
        }
    }

    if let Some(packs_dir) = global_packs_dir() {
        for p in collect_packs_in(&packs_dir, PackScope::Global) {
            if !seen.contains(&p.name) {
                result.push(p);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn list_packs_finds_project_pack_with_prompt_file() {
        let dir = tempdir().unwrap();
        let pack_dir = dir.path().join(".dk").join("packs").join("my-pack");
        let prompt = pack::prompt_path(&pack_dir);
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(&prompt, "# prompt").unwrap();

        let packs = list_packs(dir.path());
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "my-pack");
        assert_eq!(packs[0].scope, PackScope::Project);
    }
}
