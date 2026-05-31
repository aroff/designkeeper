//! DesignKeeper domain layer.
//!
//! Hosts the `dk review` / `dk check` orchestration: config resolution, file
//! discovery, slot construction, the structured review pipeline (delegated to
//! `aikit-sdk`), and domain-specific post-validation.

#[cfg(any(test, feature = "test-utils"))]
pub mod testutil;

pub mod check;
pub mod config;
pub mod discovery;
pub mod git;
pub mod init;
pub mod pack;
pub mod pack_store;
pub mod remote;
pub mod review;
pub mod sarif;
pub mod slots;
pub mod types;
pub mod validation;

pub use check::{run_check, CheckResult};
pub use config::{
    default_config, find_up, resolve_config, AgentConfig, ConfigError, DkConfig, OutputConfig,
    OutputFormat, ScanConfig, TemplatesConfig,
};
pub use init::{run_init, InitError, InitOutcome, InitParams, PackSource};
pub use pack_store::{
    global_packs_dir, install_pack, install_pack_or_embedded_fallback, list_packs, resolve_pack,
    DkTemplatesManifest, InstalledPack, PackEntry, PackScope, PackStoreError,
};
pub use review::{build_agent_runner, run_review, Progress, ProgressFn, ReviewError};
pub use sarif::SarifRunMeta;
pub use types::{
    ChangeContext, Dimension, Finding, FocusArea, GradeEntry, NotEvaluatedGrade, ProjectHints,
    ReviewInput, ReviewOptions, ReviewOutput, ScoredGrade, Severity, Summary, Verdict,
    DEFAULT_MAX_FINDINGS,
};
pub use validation::{validate_output, ValidationWarning};
