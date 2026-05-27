//! DesignKeeper domain layer.
//!
//! Hosts the `dk review` / `dk check` orchestration: config resolution, file
//! discovery, slot construction, the structured review pipeline (delegated to
//! `aikit-sdk`), and domain-specific post-validation.

pub mod check;
pub mod config;
pub mod discovery;
pub mod git;
pub mod init;
pub mod pack;
pub mod pipeline;
pub mod review;
pub mod slots;
pub mod validation;

pub use check::{run_check, run_check_with_runner, CheckResult};
pub use config::{
    default_config, resolve_config, AgentConfig, ConfigError, DkConfig, OutputConfig, OutputFormat,
    ScanConfig, TemplatesConfig,
};
pub use init::{run_init, InitError, InitOutcome, InitParams, PackSource};
pub use pipeline::{extract_json_block, validate_json, Progress, ProgressFn};
pub use review::{
    run_review, run_review_with_runner, ChangeContext, Dimension, Finding, FocusArea, GradeEntry,
    ProjectHints, ReviewError, ReviewInput, ReviewOptions, ReviewOutput, Severity, Summary, Verdict,
};
pub use validation::{validate_output, ValidationWarning};
