//! Review orchestration: `run_review`, input/contract validation, and report rendering.

use std::path::{Path, PathBuf};

use thiserror::Error;

use aikit_sdk::runner::RunError;
use aikit_sdk::{AgentRunner, Pipeline, PipelineError, TemplateRenderer};

use crate::config::DkConfig;
use crate::contract::ContractValidator;
use crate::pack;
use crate::slots;
use crate::types::{ReviewDocument, ReviewInput};

// ---------------------------------------------------------------------------
// Progress events
// ---------------------------------------------------------------------------

/// Progress events emitted during review orchestration.
#[derive(Debug, Clone, Copy)]
pub enum Progress {
    AgentRunning { attempt: u32, total: u32 },
    Validating { attempt: u32, total: u32 },
}

/// Callback that receives [`Progress`] events. Use `&|_| {}` for none.
pub type ProgressFn<'a> = dyn Fn(Progress) + 'a;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ReviewError {
    #[error("working_dir does not exist or is not a directory: {}", path.display())]
    WorkingDirInvalid { path: PathBuf },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("agent quota exceeded{}", .raw_message.as_deref().map(|m| format!(": {m}")).unwrap_or_default())]
    AgentQuotaExceeded { raw_message: Option<String> },
    #[error("agent timed out")]
    AgentTimeout,
    #[error("configured agent not found: {agent}")]
    AgentNotFound { agent: String },
    #[error("template file not found: {path}")]
    TemplateMissing { path: String },
    #[error("pipeline error: {message}")]
    PipelineFailure { message: String },
    #[error("template slot missing: {slot}")]
    TemplateSlotsError { slot: String },
    #[error("input validation failed: {}", errors.join("; "))]
    InputValidationFailed { errors: Vec<String> },
    #[error("contract violation: {}", errors.join("; "))]
    ContractViolation { errors: Vec<String> },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ReviewError {
    pub fn code(&self) -> &'static str {
        match self {
            ReviewError::WorkingDirInvalid { .. } => "DK_WORKING_DIR_INVALID",
            ReviewError::Config(c) => c.code(),
            ReviewError::AgentQuotaExceeded { .. } => "DK_AGENT_QUOTA",
            ReviewError::AgentTimeout => "DK_AGENT_TIMEOUT",
            ReviewError::AgentNotFound { .. } => "DK_AGENT_NOT_FOUND",
            ReviewError::TemplateMissing { .. } => "DK_TEMPLATE_NOT_FOUND",
            ReviewError::PipelineFailure { .. } => "DK_PIPELINE_ERROR",
            ReviewError::TemplateSlotsError { .. } => "DK_TEMPLATE_SLOT",
            ReviewError::InputValidationFailed { .. } => "DK_INPUT_VALIDATION",
            ReviewError::ContractViolation { .. } => "DK_CONTRACT_VIOLATION",
            ReviewError::Io(_) => "DK_IO_ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Run the review pipeline against an injected `AgentRunner`.
pub fn run_review(
    input: ReviewInput,
    config: &DkConfig,
    pack_dir: &Path,
    runner: AgentRunner,
    progress: &ProgressFn,
) -> Result<ReviewDocument, ReviewError> {
    let working_dir = Path::new(&input.working_dir);
    if !working_dir.is_dir() {
        return Err(ReviewError::WorkingDirInvalid {
            path: working_dir.to_path_buf(),
        });
    }

    // Validate input against Pack's review-input.json (if present).
    validate_input(&input, pack_dir)?;

    let prompt_slots = slots::build_prompt_slots(&input, config, pack_dir)?;

    let prompt_template = read_template(&pack::prompt_path(pack_dir))?;
    let schema_str = read_template(&pack::output_schema_path(pack_dir))?;

    let slots_vec = slots::slots_as_pairs(&prompt_slots);
    progress(Progress::AgentRunning {
        attempt: 1,
        total: 1,
    });
    let result = Pipeline::new(&prompt_template, &schema_str)
        .max_retries(config.agent.max_retries.unwrap_or(2))
        .run(&slots_vec, runner)
        .map_err(map_pipeline_error)?;
    progress(Progress::Validating {
        attempt: 1,
        total: 1,
    });

    // Validate against the embedded core contract.
    ContractValidator::new()
        .validate(&result.data)
        .map_err(|errors| ReviewError::ContractViolation { errors })?;

    Ok(ReviewDocument::from_value(result.data))
}

/// Render the markdown report for a validated review document.
pub fn render_report(doc: &ReviewDocument, pack_dir: &Path) -> Result<String, ReviewError> {
    let template = read_template(&pack::report_path(pack_dir))?;
    let report_slots = slots::build_report_slots(doc, pack_dir);
    let slots_vec = slots::slots_as_pairs(&report_slots);
    TemplateRenderer::render(&template, &slots_vec).map_err(map_pipeline_error)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Validate serialized input against the Pack's `schemas/review-input.json`.
/// Skips validation if the schema file does not exist (optional per Pack).
fn validate_input(input: &ReviewInput, pack_dir: &Path) -> Result<(), ReviewError> {
    let schema_path = pack::input_schema_path(pack_dir);
    let schema_text = match std::fs::read_to_string(&schema_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(ReviewError::Io(e)),
    };
    let schema: serde_json::Value =
        serde_json::from_str(&schema_text).map_err(|e| ReviewError::InputValidationFailed {
            errors: vec![format!("invalid review-input.json: {e}")],
        })?;
    let input_value =
        serde_json::to_value(input).map_err(|e| ReviewError::InputValidationFailed {
            errors: vec![format!("input serialize: {e}")],
        })?;
    let validator =
        jsonschema::validator_for(&schema).map_err(|e| ReviewError::InputValidationFailed {
            errors: vec![format!("invalid schema: {e}")],
        })?;
    let errors: Vec<String> = validator
        .iter_errors(&input_value)
        .map(|e| e.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ReviewError::InputValidationFailed { errors })
    }
}

/// Build an `AgentRunner` from the resolved config and review input.
pub fn build_agent_runner(config: &DkConfig, input: &ReviewInput) -> AgentRunner {
    let mut runner = AgentRunner::new()
        .agent(&config.agent.agent)
        .working_dir(input.working_dir.as_str());
    if let Some(model) = &config.agent.model {
        runner = runner.model(model);
    }
    if let Some(secs) = config.agent.timeout_secs {
        runner = runner.timeout(std::time::Duration::from_secs(secs));
    }
    runner
}

/// Map an `aikit_sdk::PipelineError` to a `ReviewError`.
fn map_pipeline_error(e: PipelineError) -> ReviewError {
    match e {
        PipelineError::AgentInvocation {
            source: RunError::QuotaExceeded(info),
        } => ReviewError::AgentQuotaExceeded {
            raw_message: Some(info.raw_message.clone()),
        },
        PipelineError::AgentInvocation {
            source: RunError::TimedOut { .. },
        } => ReviewError::AgentTimeout,
        PipelineError::AgentInvocation {
            source: RunError::AgentNotRunnable(key),
        } => ReviewError::AgentNotFound { agent: key },
        PipelineError::AgentInvocation { source } => ReviewError::PipelineFailure {
            message: source.to_string(),
        },
        PipelineError::TemplateSlotMissing { slot } | PipelineError::ReportRender { slot } => {
            ReviewError::TemplateSlotsError { slot }
        }
        PipelineError::ValidationFailed { errors, .. } => ReviewError::PipelineFailure {
            message: errors.join("; "),
        },
        PipelineError::MaxRetriesExceeded { last_error } => ReviewError::PipelineFailure {
            message: last_error.to_string(),
        },
    }
}

fn read_template(path: &Path) -> Result<String, ReviewError> {
    std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ReviewError::TemplateMissing {
                path: path.display().to_string(),
            }
        } else {
            ReviewError::Io(e)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ReviewOptions;

    #[test]
    fn working_dir_invalid_code() {
        let input = ReviewInput {
            working_dir: "/no/such/dir/at/all".to_string(),
            target: None,
            change_context: None,
            focus: vec![],
            project_hints: None,
            options: ReviewOptions::default(),
        };
        let cfg = crate::config::default_config();
        let (runner, _) = AgentRunner::with_mock(vec![]);
        let err = run_review(input, &cfg, Path::new("/tmp"), runner, &|_| {}).unwrap_err();
        assert_eq!(err.code(), "DK_WORKING_DIR_INVALID");
    }

    #[test]
    fn map_pipeline_error_quota_exceeded() {
        use aikit_sdk::runner::{QuotaCategory, QuotaExceededInfo};
        let info = QuotaExceededInfo {
            agent_key: "claude".to_string(),
            category: QuotaCategory::Unknown,
            raw_message: "quota exceeded".to_string(),
        };
        let e = PipelineError::AgentInvocation {
            source: RunError::QuotaExceeded(info),
        };
        assert_eq!(map_pipeline_error(e).code(), "DK_AGENT_QUOTA");
    }

    #[test]
    fn map_pipeline_error_timed_out() {
        let e = PipelineError::AgentInvocation {
            source: RunError::TimedOut {
                timeout: std::time::Duration::from_secs(1),
                stdout: vec![],
                stderr: vec![],
            },
        };
        assert_eq!(map_pipeline_error(e).code(), "DK_AGENT_TIMEOUT");
    }

    #[test]
    fn map_pipeline_error_agent_not_runnable() {
        let e = PipelineError::AgentInvocation {
            source: RunError::AgentNotRunnable("fakek".to_string()),
        };
        assert_eq!(map_pipeline_error(e).code(), "DK_AGENT_NOT_FOUND");
    }

    #[test]
    fn map_pipeline_error_validation_failed() {
        let e = PipelineError::ValidationFailed {
            raw_output: "bad".to_string(),
            errors: vec!["err".to_string()],
        };
        assert_eq!(map_pipeline_error(e).code(), "DK_PIPELINE_ERROR");
    }

    #[test]
    fn map_pipeline_error_template_slot_missing() {
        let e = PipelineError::TemplateSlotMissing {
            slot: "foo".to_string(),
        };
        assert_eq!(map_pipeline_error(e).code(), "DK_TEMPLATE_SLOT");
    }

    #[test]
    fn map_pipeline_error_report_render() {
        let e = PipelineError::ReportRender {
            slot: "bar".to_string(),
        };
        assert_eq!(map_pipeline_error(e).code(), "DK_TEMPLATE_SLOT");
    }

    #[test]
    fn input_validation_failed_code() {
        let e = ReviewError::InputValidationFailed {
            errors: vec!["missing working_dir".to_string()],
        };
        assert_eq!(e.code(), "DK_INPUT_VALIDATION");
    }

    #[test]
    fn contract_violation_code() {
        let e = ReviewError::ContractViolation {
            errors: vec!["missing verdict".to_string()],
        };
        assert_eq!(e.code(), "DK_CONTRACT_VIOLATION");
    }
}
