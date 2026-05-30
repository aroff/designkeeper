//! Built-in template pack and template-pack path layout.
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
//! `dk init` (out of scope here) materializes this under `.dk/`. Until then the
//! defaults are embedded from `templates/default/` and can be written to any
//! directory via [`write_default_pack`] so `dk review` works without init.

use std::io;
use std::path::{Path, PathBuf};

/// Default prompt template (`templates/review.md`).
pub const PROMPT_TEMPLATE: &str = include_str!("../../../templates/default/templates/review.md");
/// Default rubric (`templates/methodology.md`).
pub const METHODOLOGY: &str = include_str!("../../../templates/default/templates/methodology.md");
/// Default report layout (`reports/review.md`).
pub const REPORT_TEMPLATE: &str = include_str!("../../../templates/default/reports/review.md");
/// Input schema (`schemas/review-input.json`).
pub const INPUT_SCHEMA: &str = include_str!("../../../templates/default/schemas/review-input.json");
/// Output schema (`schemas/review.json`).
pub const OUTPUT_SCHEMA: &str = include_str!("../../../templates/default/schemas/review.json");

/// Embedded pack manifest for structural template.
pub const STRUCTURAL_PROMPT_TEMPLATE: &str =
    include_str!("../../../templates/structural/templates/review.md");
pub const STRUCTURAL_METHODOLOGY: &str =
    include_str!("../../../templates/structural/templates/methodology.md");
pub const STRUCTURAL_REPORT_TEMPLATE: &str =
    include_str!("../../../templates/structural/reports/review.md");
pub const STRUCTURAL_INPUT_SCHEMA: &str =
    include_str!("../../../templates/structural/schemas/review-input.json");
pub const STRUCTURAL_OUTPUT_SCHEMA: &str =
    include_str!("../../../templates/structural/schemas/review.json");

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

fn create_pack_dirs(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir.join("templates"))?;
    std::fs::create_dir_all(dir.join("schemas"))?;
    std::fs::create_dir_all(dir.join("reports"))?;
    Ok(())
}

/// Write the embedded default template pack under `dir`, creating subdirs.
pub fn write_default_pack(dir: &Path) -> io::Result<()> {
    create_pack_dirs(dir)?;
    std::fs::write(prompt_path(dir), PROMPT_TEMPLATE)?;
    std::fs::write(methodology_path(dir), METHODOLOGY)?;
    std::fs::write(report_path(dir), REPORT_TEMPLATE)?;
    std::fs::write(input_schema_path(dir), INPUT_SCHEMA)?;
    std::fs::write(output_schema_path(dir), OUTPUT_SCHEMA)?;
    Ok(())
}

/// Write the embedded structural template pack under `dir`, creating subdirs.
pub fn write_structural_pack(dir: &Path) -> io::Result<()> {
    create_pack_dirs(dir)?;
    std::fs::write(prompt_path(dir), STRUCTURAL_PROMPT_TEMPLATE)?;
    std::fs::write(methodology_path(dir), STRUCTURAL_METHODOLOGY)?;
    std::fs::write(report_path(dir), STRUCTURAL_REPORT_TEMPLATE)?;
    std::fs::write(input_schema_path(dir), STRUCTURAL_INPUT_SCHEMA)?;
    std::fs::write(output_schema_path(dir), STRUCTURAL_OUTPUT_SCHEMA)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn embedded_pack_is_nonempty() {
        assert!(PROMPT_TEMPLATE.contains("{{methodology}}"));
        assert!(PROMPT_TEMPLATE.contains("{{output_schema}}"));
        assert!(REPORT_TEMPLATE.contains("{{grades_table}}"));
        assert!(OUTPUT_SCHEMA.contains("\"verdict\""));
        assert!(INPUT_SCHEMA.contains("\"working_dir\""));
    }

    #[test]
    fn writes_full_pack() {
        let dir = tempdir().unwrap();
        write_default_pack(dir.path()).unwrap();
        assert!(prompt_path(dir.path()).is_file());
        assert!(methodology_path(dir.path()).is_file());
        assert!(report_path(dir.path()).is_file());
        assert!(input_schema_path(dir.path()).is_file());
        assert!(output_schema_path(dir.path()).is_file());
    }
}
