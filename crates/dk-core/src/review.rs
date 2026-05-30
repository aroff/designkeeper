//! Review orchestration: `run_review`, score reconciliation, and report rendering.

use std::path::{Path, PathBuf};

use thiserror::Error;

use aikit_sdk::runner::RunError;
use aikit_sdk::{AgentRunner, Pipeline, PipelineError, TemplateRenderer};

use crate::config::DkConfig;
use crate::pack;
use crate::types::{ReviewInput, ReviewOutput};
use crate::{slots, validation};


/// Tolerance for the mean-of-grades drift check (V2).
pub const MEAN_DRIFT_TOLERANCE: f64 = 0.5;

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
    template_dir: &Path,
    runner: AgentRunner,
    progress: &ProgressFn,
) -> Result<ReviewOutput, ReviewError> {
    let working_dir = Path::new(&input.working_dir);
    if !working_dir.is_dir() {
        return Err(ReviewError::WorkingDirInvalid {
            path: working_dir.to_path_buf(),
        });
    }

    let prompt_slots = slots::build_prompt_slots(&input, config, template_dir)?;

    let prompt_template = read_template(&pack::prompt_path(template_dir))?;
    let schema_str = read_template(&pack::output_schema_path(template_dir))?;

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

    let mut output: ReviewOutput =
        serde_json::from_value(result.data).map_err(|e| ReviewError::PipelineFailure {
            message: format!("output deserialize: {e}"),
        })?;

    reconcile_scores(&mut output);
    for warning in validation::validate_output(&output) {
        tracing::warn!(rule = %warning.rule, "{}", warning.message);
    }

    Ok(output)
}

fn reconcile_scores(output: &mut ReviewOutput) {
    let mean = match output.mean_grade_score() {
        Some(m) => (m * 10.0).round() / 10.0,
        None => output.overall_score,
    };
    let needs_fix = (output.summary.overall_score - output.overall_score).abs()
        > MEAN_DRIFT_TOLERANCE
        || (output.summary.overall_score - mean).abs() > MEAN_DRIFT_TOLERANCE;
    if needs_fix {
        tracing::warn!(
            rule = "V1",
            "score reconciled: summary={} top_level={} → {mean}",
            output.summary.overall_score,
            output.overall_score
        );
        output.summary.overall_score = mean;
        output.overall_score = mean;
    }
}

/// Render the markdown report for a validated review output.
pub fn render_report(output: &ReviewOutput, template_dir: &Path) -> Result<String, ReviewError> {
    let template = read_template(&pack::report_path(template_dir))?;
    let report_slots = slots::build_report_slots(output);
    let slots_vec = slots::slots_as_pairs(&report_slots);
    TemplateRenderer::render(&template, &slots_vec).map_err(map_pipeline_error)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

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
}
