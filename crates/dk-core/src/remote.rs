//! Remote template pack fetching — thin wrapper over aikit-sdk fetch/install.
//!
//! Fetches a pack from GitHub, a direct zip URL, or a local path and places it
//! under `dest_dir/{pack_name}/`.

use std::io;
use std::path::{Path, PathBuf};

use aikit_sdk::{InstallError, TemplateSource};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("invalid source '{src}': {message}")]
    InvalidSource { src: String, message: String },
    #[error("fetch failed for '{src}': {message}")]
    FetchFailed { src: String, message: String },
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

impl RemoteError {
    pub fn code(&self) -> &'static str {
        match self {
            RemoteError::InvalidSource { .. } => "DK_REMOTE_INVALID_SOURCE",
            RemoteError::FetchFailed { .. } => "DK_REMOTE_FETCH_FAILED",
            RemoteError::Io(_) => "DK_IO_ERROR",
        }
    }
}

fn map_install_err(src: &str, e: InstallError) -> RemoteError {
    match e {
        InstallError::InvalidSource(msg) => RemoteError::InvalidSource {
            src: src.to_string(),
            message: msg,
        },
        other => RemoteError::FetchFailed {
            src: src.to_string(),
            message: other.to_string(),
        },
    }
}

/// Fetch a template pack from `source_str` and place it at `dest_dir/{pack_name}/`.
///
/// Returns the resolved pack directory path.
pub fn fetch_pack(source_str: &str, dest_dir: &Path) -> Result<PathBuf, RemoteError> {
    let source = TemplateSource::parse(source_str)
        .map_err(|e| map_install_err(source_str, e))?;

    let staging =
        tempfile::tempdir().map_err(|e| RemoteError::Io(io::Error::new(io::ErrorKind::Other, e)))?;

    let (manifest, pack_root) = aikit_sdk::fetch::fetch_package_to_dir(&source, staging.path())
        .map_err(|e| map_install_err(source_str, e))?;

    let pack_name = &manifest.package.name;
    let final_dest = dest_dir.join(pack_name);

    if final_dest.exists() {
        std::fs::remove_dir_all(&final_dest)?;
    }
    std::fs::create_dir_all(dest_dir)?;
    copy_dir_recursive(&pack_root, &final_dest)?;

    Ok(final_dest)
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
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
