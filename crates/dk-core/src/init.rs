//! `dk init` scaffolding: install template packs to `.dk/packs/` and write `dk.toml`.

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::DocumentMut;

use crate::pack_store::{self, DkTemplatesManifest, InstalledPack};

/// Where a materialized pack came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackSource {
    Embedded,
    LocalDir(PathBuf),
    Remote(String),
}

/// Parameters collected for an init run.
#[derive(Debug, Clone)]
pub struct InitParams {
    pub agent: String,
    pub model: Option<String>,
}

/// Result of a successful init run.
#[derive(Debug)]
pub struct InitOutcome {
    pub dk_dir: PathBuf,
    pub config_path: PathBuf,
    pub installed_packs: Vec<InstalledPack>,
    /// `true` if `dk.toml` already existed and was updated in place.
    pub updated_existing: bool,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("io error during init: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse existing dk.toml at {path}: {message}")]
    ConfigParse { path: String, message: String },
}

impl InitError {
    pub fn code(&self) -> &'static str {
        match self {
            InitError::Io(_) => "DK_IO_ERROR",
            InitError::ConfigParse { .. } => "DK_CONFIG_PARSE",
        }
    }
}

/// Scaffold `.dk/packs/` under `working_dir`, install all official packs, and
/// write (or update) `dk.toml`.
///
/// Re-running is safe: existing packs are overwritten, `dk.toml` sections other
/// than `[agent]` are preserved.
pub fn run_init(working_dir: &Path, params: &InitParams) -> Result<InitOutcome, InitError> {
    let dk_dir = working_dir.join(".dk");
    let packs_dir = dk_dir.join("packs");
    std::fs::create_dir_all(&packs_dir)?;

    let manifest = DkTemplatesManifest::resolve(working_dir);
    let installed_packs = install_packs_from_manifest(&manifest, &packs_dir);

    let config_path = working_dir.join("dk.toml");
    let updated_existing = config_path.is_file();
    write_config(&config_path, params)?;

    Ok(InitOutcome {
        dk_dir,
        config_path,
        installed_packs,
        updated_existing,
    })
}

/// Install all packs listed in the manifest.
///
/// Failures for individual packs are non-fatal — we fall back to the embedded
/// copy for built-ins and skip unknown packs that can't be fetched.
fn install_packs_from_manifest(
    manifest: &DkTemplatesManifest,
    packs_dir: &Path,
) -> Vec<InstalledPack> {
    manifest
        .packs
        .iter()
        .filter_map(|entry| {
            match pack_store::install_pack(&entry.source, packs_dir) {
                Ok(installed) => Some(installed),
                Err(_) => {
                    // Fall back: write embedded copy for known built-ins
                    write_embedded_fallback(&entry.name, packs_dir)
                }
            }
        })
        .collect()
}

fn write_embedded_fallback(name: &str, packs_dir: &Path) -> Option<InstalledPack> {
    let dest = packs_dir.join(name);
    let result = match name {
        "default" => crate::pack::write_default_pack(&dest),
        "structural" => crate::pack::write_structural_pack(&dest),
        _ => return None,
    };
    result.ok().map(|_| InstalledPack {
        name: name.to_string(),
        path: dest,
        scope: crate::pack_store::PackScope::Embedded,
    })
}

fn write_config(path: &Path, params: &InitParams) -> Result<(), InitError> {
    let mut doc = if path.is_file() {
        let text = std::fs::read_to_string(path)?;
        text.parse::<DocumentMut>()
            .map_err(|e| InitError::ConfigParse {
                path: path.display().to_string(),
                message: e.to_string(),
            })?
    } else {
        DocumentMut::new()
    };

    set_str(&mut doc, "agent", "agent", &params.agent);
    set_str(
        &mut doc,
        "agent",
        "model",
        params.model.as_deref().unwrap_or(""),
    );

    std::fs::write(path, doc.to_string())?;
    Ok(())
}

fn set_str(doc: &mut DocumentMut, section: &str, key: &str, value: &str) {
    doc[section][key] = toml_edit::value(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_creates_packs_dir_and_config() {
        let dir = tempdir().unwrap();
        let params = InitParams {
            agent: "claude".to_string(),
            model: None,
        };
        let out = run_init(dir.path(), &params).unwrap();
        assert!(out.dk_dir.join("packs").is_dir());
        assert!(out.config_path.is_file());
        assert!(!out.updated_existing);
    }

    #[test]
    fn update_in_place_preserves_other_sections() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("dk.toml"),
            "[scan]\nextensions = [\".rs\"]\n\n[agent]\nagent = \"claude\"\n",
        )
        .unwrap();

        let params = InitParams {
            agent: "gemini".to_string(),
            model: None,
        };
        let out = run_init(dir.path(), &params).unwrap();
        assert!(out.updated_existing);

        let cfg = crate::config::resolve_config(dir.path()).unwrap();
        assert_eq!(cfg.agent.agent, "gemini");
        assert_eq!(cfg.scan.extensions, vec![".rs"]);
    }
}
