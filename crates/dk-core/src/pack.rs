//! Template pack path layout helpers.
//!
//! The review template pack lives under a directory laid out as (spec §40-54):
//!
//! ```text
//! <dir>/
//! ├── templates/
//! │   ├── review.md        # prompt template
//! │   └── methodology.md   # rubric (user-editable)
//! ├── schemas/
//! │   ├── review-input.json
//! │   └── review.json      # output schema
//! └── reports/
//!     └── review.md        # report layout
//! ```
//!
//! Packs must be installed via `dk install` or `dk init` before commands that
//! require them can run.

use std::path::{Path, PathBuf};

/// Official pack manifest (embedded from repo root `dk-templates.toml`).
pub const DK_TEMPLATES_MANIFEST: &str = include_str!("../../../dk-templates.toml");

pub fn prompt_path(dir: &Path) -> PathBuf {
    dir.join("templates").join("review.md")
}

pub fn methodology_path(dir: &Path) -> PathBuf {
    dir.join("templates").join("methodology.md")
}

pub fn report_path(dir: &Path) -> PathBuf {
    dir.join("reports").join("review.md")
}

pub fn input_schema_path(dir: &Path) -> PathBuf {
    dir.join("schemas").join("review-input.json")
}

pub fn output_schema_path(dir: &Path) -> PathBuf {
    dir.join("schemas").join("review.json")
}
