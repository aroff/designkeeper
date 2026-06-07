//! `dk init` scaffolding: install template packs to `.dk/packs/` and write `dk.toml`.

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::DocumentMut;

use crate::pack_store::{self, DkTemplatesManifest, InstalledPack, PackScope, PackStoreError};

/// Where a materialized pack came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackSource {
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
    /// Packs that were already installed and skipped (not overwritten).
    pub skipped_packs: Vec<String>,
    /// `true` if `dk.toml` already existed and was updated in place.
    pub updated_existing: bool,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("io error during init: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse existing dk.toml at {path}: {message}")]
    ConfigParse { path: String, message: String },
    #[error("failed to install pack '{name}': {cause}")]
    InstallFailed {
        name: String,
        #[source]
        cause: PackStoreError,
    },
}

impl InitError {
    pub fn code(&self) -> &'static str {
        match self {
            InitError::Io(_) => "DK_IO_ERROR",
            InitError::ConfigParse { .. } => "DK_CONFIG_PARSE",
            InitError::InstallFailed { .. } => "DK_PACK_INSTALL_FAILED",
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
    let installed_packs = install_packs_from_manifest(&manifest, &packs_dir)?;

    let config_path = working_dir.join("dk.toml");
    let updated_existing = config_path.is_file();
    write_config(&config_path, params)?;

    Ok(InitOutcome {
        dk_dir,
        config_path,
        installed_packs: installed_packs.installed,
        skipped_packs: installed_packs.skipped,
        updated_existing,
    })
}

struct InstallResult {
    installed: Vec<InstalledPack>,
    skipped: Vec<String>,
}

/// Install packs from the manifest, skipping any that are already present.
fn install_packs_from_manifest(
    manifest: &DkTemplatesManifest,
    packs_dir: &Path,
) -> Result<InstallResult, InitError> {
    let mut installed = vec![];
    let mut skipped = vec![];
    for entry in &manifest.packs {
        let pack_dir = packs_dir.join(&entry.name);
        if crate::pack::prompt_path(&pack_dir).is_file() {
            skipped.push(entry.name.clone());
            continue;
        }
        let pack = pack_store::install_pack(&entry.source, packs_dir, PackScope::Project).map_err(
            |e| InitError::InstallFailed {
                name: entry.name.clone(),
                cause: e,
            },
        )?;
        installed.push(pack);
    }
    Ok(InstallResult { installed, skipped })
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
        // Use an empty manifest so install doesn't fail on placeholder sources.
        std::fs::write(dir.path().join("dk-templates.toml"), "packs = []\n").unwrap();
        let params = InitParams {
            agent: "claude".to_string(),
            model: None,
        };
        let out = run_init(dir.path(), &params).unwrap();
        assert!(out.dk_dir.join("packs").is_dir());
        assert!(out.config_path.is_file());
        assert!(!out.updated_existing);
        assert!(out.installed_packs.is_empty());
        assert!(out.skipped_packs.is_empty());
    }

    #[test]
    fn update_in_place_preserves_other_sections() {
        let dir = tempdir().unwrap();
        // Use an empty manifest so install doesn't fail on placeholder sources.
        std::fs::write(dir.path().join("dk-templates.toml"), "packs = []\n").unwrap();
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

    #[test]
    fn init_skips_already_installed_packs() {
        let dir = tempdir().unwrap();
        let manifest_toml =
            "[[packs]]\nname = \"default\"\ndescription = \"d\"\nsource = \"unused\"\n";
        std::fs::write(dir.path().join("dk-templates.toml"), manifest_toml).unwrap();
        // Pre-install the pack by creating its prompt file.
        let pack_dir = dir.path().join(".dk").join("packs").join("default");
        let prompt = crate::pack::prompt_path(&pack_dir);
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(&prompt, "# prompt").unwrap();

        let params = InitParams {
            agent: "claude".to_string(),
            model: None,
        };
        let out = run_init(dir.path(), &params).unwrap();
        assert!(out.installed_packs.is_empty(), "should not reinstall");
        assert_eq!(out.skipped_packs, vec!["default"]);
    }

    #[test]
    fn init_fails_when_pack_fetch_fails() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("dk-templates.toml"),
            "[[packs]]\nname = \"test\"\ndescription = \"test\"\nsource = \"not-a-real-source://invalid\"\n",
        )
        .unwrap();
        let params = InitParams {
            agent: "claude".to_string(),
            model: None,
        };
        let result = run_init(dir.path(), &params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "DK_PACK_INSTALL_FAILED");
    }
}
