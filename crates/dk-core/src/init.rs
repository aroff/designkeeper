//! `dk init` scaffolding: materialize `.dk/` and write/update `dk.toml`.
//!
//! This is the domain side of init (CONTEXT.md §"Init Flow"). The CLI layer
//! handles argument parsing and interactive prompts; here we only perform the
//! filesystem effects: laying down the template pack and the control file.

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::DocumentMut;

use crate::pack;

/// Where the materialized template pack contents originated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackSource {
    /// Wrote the embedded default pack.
    Embedded,
    /// Copied from a local directory.
    LocalDir(PathBuf),
}

/// Parameters collected for an init run.
#[derive(Debug, Clone)]
pub struct InitParams {
    pub agent: String,
    pub model: Option<String>,
    /// `"default"`, a local folder path, or a remote URL reference.
    pub pack: String,
}

/// Result of a successful init run.
#[derive(Debug)]
pub struct InitOutcome {
    pub dk_dir: PathBuf,
    pub config_path: PathBuf,
    pub pack_source: PackSource,
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

/// Scaffold `.dk/` under `working_dir` and write (or update) `dk.toml`.
///
/// Re-running is safe and iterative: an existing `dk.toml` is parsed and only
/// the `[agent]` and `[templates]` fields are overwritten, preserving any
/// `[scan]`/`[output]` the user added.
pub fn run_init(working_dir: &Path, params: &InitParams) -> Result<InitOutcome, InitError> {
    let dk_dir = working_dir.join(".dk");
    let staging = working_dir.join(".dk.tmp");

    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }

    let pack_source = materialize_pack(&staging, &params.pack)?;

    let config_path = working_dir.join("dk.toml");
    let updated_existing = config_path.is_file();
    write_config(&config_path, params)?;

    if dk_dir.exists() {
        let backup = working_dir.join(".dk.old");
        if backup.exists() {
            std::fs::remove_dir_all(&backup)?;
        }
        std::fs::rename(&dk_dir, &backup)?;
    }
    std::fs::rename(&staging, &dk_dir)?;

    Ok(InitOutcome {
        dk_dir,
        config_path,
        pack_source,
        updated_existing,
    })
}

/// Lay down the template pack under `dk_dir`.
///
/// A pack reference that points at an existing local directory is copied
/// verbatim. Anything else (`"default"` or a remote URL we cannot fetch
/// offline) seeds `.dk/` with the embedded defaults so `dk review` works
/// immediately; the reference itself is recorded in `dk.toml`.
fn materialize_pack(dk_dir: &Path, pack_ref: &str) -> Result<PackSource, InitError> {
    let candidate = Path::new(pack_ref);
    if pack_ref != "default" && candidate.is_dir() {
        copy_dir_recursive(candidate, dk_dir)?;
        return Ok(PackSource::LocalDir(candidate.to_path_buf()));
    }
    pack::write_default_pack(dk_dir)?;
    Ok(PackSource::Embedded)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
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
    set_str(&mut doc, "templates", "pack", &params.pack);

    std::fs::write(path, doc.to_string())?;
    Ok(())
}

fn set_str(doc: &mut DocumentMut, section: &str, key: &str, value: &str) {
    doc[section][key] = toml_edit::value(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::resolve_config;
    use tempfile::tempdir;

    #[test]
    fn writes_pack_and_config() {
        let dir = tempdir().unwrap();
        let params = InitParams {
            agent: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            pack: "default".to_string(),
        };
        let out = run_init(dir.path(), &params).unwrap();

        assert_eq!(out.pack_source, PackSource::Embedded);
        assert!(!out.updated_existing);
        assert!(pack::prompt_path(&out.dk_dir).is_file());
        assert!(out.config_path.is_file());

        // The written dk.toml must round-trip through the resolver.
        let cfg = resolve_config(dir.path()).unwrap();
        assert_eq!(cfg.agent.agent, "codex");
        assert_eq!(cfg.agent.model.as_deref(), Some("gpt-5"));
        assert_eq!(cfg.templates.pack, "default");
    }

    #[test]
    fn empty_model_round_trips_as_unset() {
        let dir = tempdir().unwrap();
        let params = InitParams {
            agent: "claude".to_string(),
            model: None,
            pack: "default".to_string(),
        };
        run_init(dir.path(), &params).unwrap();
        let cfg = resolve_config(dir.path()).unwrap();
        assert_eq!(cfg.agent.model, None);
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
            pack: "default".to_string(),
        };
        let out = run_init(dir.path(), &params).unwrap();
        assert!(out.updated_existing);

        let cfg = resolve_config(dir.path()).unwrap();
        assert_eq!(cfg.agent.agent, "gemini");
        // The user's [scan] override survived the rewrite.
        assert_eq!(cfg.scan.extensions, vec![".rs"]);
    }

    #[test]
    fn copies_local_pack_dir() {
        let src = tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("templates")).unwrap();
        std::fs::write(src.path().join("templates").join("review.md"), "custom").unwrap();

        let dir = tempdir().unwrap();
        let params = InitParams {
            agent: "claude".to_string(),
            model: None,
            pack: src.path().to_string_lossy().into_owned(),
        };
        let out = run_init(dir.path(), &params).unwrap();
        assert_eq!(
            out.pack_source,
            PackSource::LocalDir(src.path().to_path_buf())
        );
        let copied = std::fs::read_to_string(pack::prompt_path(&out.dk_dir)).unwrap();
        assert_eq!(copied, "custom");
    }
}
