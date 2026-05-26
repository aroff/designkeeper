//! DesignKeeper domain layer.
//!
//! Hosts the `dk review` / `dk check` orchestration: config resolution, file
//! discovery, slot construction, the structured review pipeline, and
//! domain-specific post-validation. This crate is deliberately free of any
//! `cli-framework` dependency so it can be reused by future `serve` / `mcp`
//! front-ends.
//!
//! The structured pipeline (template render -> agent -> extract + schema
//! validate -> report render) is defined here as a small trait-based shim
//! (see [`pipeline`]). ADR 0001 delegates this to `aikit-sdk`; until its
//! structured-pipeline API lands, the shim is the local fallback sanctioned by
//! the implementation spec.

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

pub use check::{run_check, CheckResult};
pub use config::{
    default_config, resolve_config, AgentConfig, ConfigError, DkConfig, OutputConfig, OutputFormat,
    ScanConfig, TemplatesConfig,
};
pub use init::{run_init, InitError, InitOutcome, InitParams, PackSource};
pub use pipeline::{validate_json, AgentRunner, Pipeline, PipelineError};
pub use review::{
    run_review, run_review_with_agent, ChangeContext, Dimension, Finding, FocusArea, GradeEntry,
    ProjectHints, ReviewError, ReviewInput, ReviewOptions, ReviewOutput, Severity, Summary,
    Verdict,
};
pub use validation::{validate_output, ValidationWarning};
