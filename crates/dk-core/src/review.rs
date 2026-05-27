//! Review domain types and `run_review` orchestration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use aikit_sdk::runner::RunError;
use aikit_sdk::{AgentRunner, Pipeline, PipelineError, TemplateRenderer};

use crate::config::DkConfig;
use crate::pack;
use crate::pipeline::{validate_json, Progress, ProgressFn};
use crate::{slots, validation};

/// Tolerance for the V1 summary/top-level score equality check.
pub const SCORE_TOLERANCE: f64 = 0.01;

// ---------------------------------------------------------------------------
// Input types (mirror schemas/input.schema.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReviewInput {
    pub working_dir: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub change_context: Option<ChangeContext>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub focus: Vec<FocusArea>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub project_hints: Option<ProjectHints>,
    #[serde(default)]
    pub options: ReviewOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ChangeContext {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub head_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diff_stat: Option<String>,
}

impl ChangeContext {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.base_ref.is_none()
            && self.head_ref.is_none()
            && self.diff_stat.is_none()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusArea {
    Security,
    Concurrency,
    Accessibility,
    Internationalization,
    Privacy,
    Performance,
    ApiDesign,
    Ui,
}

impl FocusArea {
    pub fn as_key(&self) -> &'static str {
        match self {
            FocusArea::Security => "security",
            FocusArea::Concurrency => "concurrency",
            FocusArea::Accessibility => "accessibility",
            FocusArea::Internationalization => "internationalization",
            FocusArea::Privacy => "privacy",
            FocusArea::Performance => "performance",
            FocusArea::ApiDesign => "api_design",
            FocusArea::Ui => "ui",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "security" => Some(Self::Security),
            "concurrency" => Some(Self::Concurrency),
            "accessibility" => Some(Self::Accessibility),
            "internationalization" => Some(Self::Internationalization),
            "privacy" => Some(Self::Privacy),
            "performance" => Some(Self::Performance),
            "api_design" => Some(Self::ApiDesign),
            "ui" => Some(Self::Ui),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectHints {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub style_guide: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub contributing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub architecture_docs: Option<Vec<String>>,
}

impl ProjectHints {
    pub fn is_empty(&self) -> bool {
        self.style_guide.is_none()
            && self.contributing.is_none()
            && self.architecture_docs.as_ref().is_none_or(|d| d.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewOptions {
    pub max_findings: u8,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub include_dimensions: Option<Vec<Dimension>>,
}

impl Default for ReviewOptions {
    fn default() -> Self {
        Self {
            max_findings: 25,
            include_dimensions: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    OverallCodeHealth,
    ClDescription,
    ChangeScope,
    Design,
    Functionality,
    Complexity,
    Tests,
    Naming,
    Comments,
    Style,
    Consistency,
    Documentation,
    ContextAndReviewDepth,
}

impl Dimension {
    pub fn as_key(&self) -> &'static str {
        match self {
            Dimension::OverallCodeHealth => "overall_code_health",
            Dimension::ClDescription => "cl_description",
            Dimension::ChangeScope => "change_scope",
            Dimension::Design => "design",
            Dimension::Functionality => "functionality",
            Dimension::Complexity => "complexity",
            Dimension::Tests => "tests",
            Dimension::Naming => "naming",
            Dimension::Comments => "comments",
            Dimension::Style => "style",
            Dimension::Consistency => "consistency",
            Dimension::Documentation => "documentation",
            Dimension::ContextAndReviewDepth => "context_and_review_depth",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "overall_code_health" => Some(Self::OverallCodeHealth),
            "cl_description" => Some(Self::ClDescription),
            "change_scope" => Some(Self::ChangeScope),
            "design" => Some(Self::Design),
            "functionality" => Some(Self::Functionality),
            "complexity" => Some(Self::Complexity),
            "tests" => Some(Self::Tests),
            "naming" => Some(Self::Naming),
            "comments" => Some(Self::Comments),
            "style" => Some(Self::Style),
            "consistency" => Some(Self::Consistency),
            "documentation" => Some(Self::Documentation),
            "context_and_review_depth" => Some(Self::ContextAndReviewDepth),
            _ => None,
        }
    }
}

/// Declaration order is significant: it defines severity ranking (blockers
/// first) used when grouping findings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Blocker,
    Major,
    Minor,
    Nit,
}

impl Severity {
    pub fn as_key(&self) -> &'static str {
        match self {
            Severity::Blocker => "blocker",
            Severity::Major => "major",
            Severity::Minor => "minor",
            Severity::Nit => "nit",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approve,
    ApproveWithComments,
    RequestChanges,
    Reject,
}

impl Verdict {
    /// True when the verdict should map to a passing `dk check` (exit 0).
    pub fn is_pass(&self) -> bool {
        matches!(self, Verdict::Approve | Verdict::ApproveWithComments)
    }
}

// ---------------------------------------------------------------------------
// Output types (mirror schemas/output.schema.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReviewOutput {
    pub summary: Summary,
    pub grades: BTreeMap<Dimension, GradeEntry>,
    pub overall_score: f64,
    pub good_things: Vec<String>,
    pub findings: Vec<Finding>,
    pub limitations: Vec<String>,
    pub suggested_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub verdict: Verdict,
    pub overall_score: f64,
    pub one_paragraph: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GradeEntry {
    Scored {
        score: f64,
        rationale: String,
    },
    NotEvaluated {
        not_evaluated: bool,
        rationale: String,
    },
}

impl GradeEntry {
    pub fn score(&self) -> Option<f64> {
        match self {
            GradeEntry::Scored { score, .. } => Some(*score),
            GradeEntry::NotEvaluated { .. } => None,
        }
    }

    pub fn rationale(&self) -> &str {
        match self {
            GradeEntry::Scored { rationale, .. } => rationale,
            GradeEntry::NotEvaluated { rationale, .. } => rationale,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub dimension: Dimension,
    pub severity: Severity,
    pub location: String,
    pub observation: String,
    pub why_it_matters: String,
    pub recommended_action: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub suggested_patch: Option<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ReviewError {
    #[error("input failed schema validation: {}", errors.join("; "))]
    InputValidation { errors: Vec<String> },
    #[error("working_dir does not exist or is not a directory: {}", path.display())]
    WorkingDirInvalid { path: PathBuf },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    /// Agent session quota / rate-limit exceeded (`DK_AGENT_QUOTA`).
    #[error("agent quota exceeded{}", .raw_message.as_deref().map(|m| format!(": {m}")).unwrap_or_default())]
    AgentQuotaExceeded { raw_message: Option<String> },
    #[error("agent timed out")]
    AgentTimeout,
    #[error("configured agent not found: {agent}")]
    AgentNotFound { agent: String },
    #[error("template file not found: {path}")]
    TemplateMissing { path: String },
    #[error("invalid output schema: {message}")]
    InvalidSchema { message: String },
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
            ReviewError::InputValidation { .. } => "DK_INPUT_VALIDATION",
            ReviewError::WorkingDirInvalid { .. } => "DK_WORKING_DIR_INVALID",
            ReviewError::Config(c) => c.code(),
            ReviewError::AgentQuotaExceeded { .. } => "DK_AGENT_QUOTA",
            ReviewError::AgentTimeout => "DK_AGENT_TIMEOUT",
            ReviewError::AgentNotFound { .. } => "DK_AGENT_NOT_FOUND",
            ReviewError::TemplateMissing { .. } => "DK_TEMPLATE_NOT_FOUND",
            ReviewError::InvalidSchema { .. } | ReviewError::PipelineFailure { .. } => {
                "DK_PIPELINE_ERROR"
            }
            ReviewError::TemplateSlotsError { .. } => "DK_TEMPLATE_SLOT",
            ReviewError::Io(_) => "DK_IO_ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Run the review pipeline using the real agent from `config`.
pub fn run_review(
    input: ReviewInput,
    config: &DkConfig,
    template_dir: &Path,
    progress: &ProgressFn,
) -> Result<ReviewOutput, ReviewError> {
    let runner = build_agent_runner(config, &input);
    run_review_with_runner(input, config, template_dir, runner, progress)
}

/// Run the review pipeline against an injected `AgentRunner` (used by tests).
pub fn run_review_with_runner(
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

    // 1. Validate CLI-built input against the input schema.
    let input_value = serde_json::to_value(&input).map_err(|e| ReviewError::InputValidation {
        errors: vec![e.to_string()],
    })?;
    let input_schema = read_schema(&pack::input_schema_path(template_dir))?;
    validate_against(&input_schema, &input_value)
        .map_err(|errors| ReviewError::InputValidation { errors })?;

    // 2. Build the prompt slots.
    let prompt_slots = slots::build_prompt_slots(&input, config, template_dir)?;

    // 3. Load prompt template + output schema string.
    let prompt_template = read_template(&pack::prompt_path(template_dir))?;
    let schema_str = read_template(&pack::output_schema_path(template_dir))?;

    // 4. Run the aikit-sdk structured pipeline (render → agent → validate, retry).
    let slots_vec = slots::slots_as_pairs(&prompt_slots);
    progress(Progress::AgentRunning { attempt: 1, total: 1 });
    let result = Pipeline::new(&prompt_template, &schema_str)
        .max_retries(config.agent.max_retries.unwrap_or(2))
        .run(&slots_vec, runner)
        .map_err(map_pipeline_error)?;
    progress(Progress::Validating { attempt: 1, total: 1 });

    // 5. Deserialize into the typed output.
    let mut output: ReviewOutput =
        serde_json::from_value(result.data).map_err(|e| ReviewError::PipelineFailure {
            message: format!("output deserialize: {e}"),
        })?;

    // 6. Post-validation: V1 auto-reconcile; V2-V4 are warnings.
    reconcile_scores(&mut output);
    for warning in validation::validate_output(&output) {
        tracing::warn!(rule = %warning.rule, "{}", warning.message);
    }

    Ok(output)
}

fn reconcile_scores(output: &mut ReviewOutput) {
    if (output.summary.overall_score - output.overall_score).abs() > SCORE_TOLERANCE {
        let scored: Vec<f64> = output.grades.values().filter_map(|g| g.score()).collect();
        let canonical = if scored.is_empty() {
            output.overall_score
        } else {
            (scored.iter().sum::<f64>() / scored.len() as f64 * 10.0).round() / 10.0
        };
        tracing::warn!(
            rule = "V1",
            "score reconciled: summary={} top_level={} → {canonical}",
            output.summary.overall_score,
            output.overall_score
        );
        output.summary.overall_score = canonical;
        output.overall_score = canonical;
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
pub(crate) fn build_agent_runner(config: &DkConfig, input: &ReviewInput) -> AgentRunner {
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
            raw_message: Some(format!("{info:?}")),
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

fn read_schema(path: &Path) -> Result<Value, ReviewError> {
    let text = read_template(path)?;
    serde_json::from_str(&text).map_err(|e| ReviewError::InvalidSchema {
        message: format!("{}: {e}", path.display()),
    })
}

fn validate_against(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    validate_json(schema, instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_round_trips_minimal() {
        let json = r#"{"working_dir":".","target":"src/"}"#;
        let input: ReviewInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.working_dir, ".");
        assert_eq!(input.target.as_deref(), Some("src/"));
        assert_eq!(input.options.max_findings, 25);
        let v = serde_json::to_value(&input).unwrap();
        assert_eq!(v["options"]["max_findings"], 25);
        assert!(v.get("focus").is_none());
    }

    #[test]
    fn dimension_serializes_snake_case() {
        let v = serde_json::to_value(Dimension::OverallCodeHealth).unwrap();
        assert_eq!(v, Value::String("overall_code_health".into()));
    }

    #[test]
    fn verdict_pass_mapping() {
        assert!(Verdict::Approve.is_pass());
        assert!(Verdict::ApproveWithComments.is_pass());
        assert!(!Verdict::RequestChanges.is_pass());
        assert!(!Verdict::Reject.is_pass());
    }

    #[test]
    fn grade_entry_untagged_round_trip() {
        let scored: GradeEntry = serde_json::from_str(r#"{"score":8,"rationale":"ok"}"#).unwrap();
        assert_eq!(scored.score(), Some(8.0));
        let ne: GradeEntry =
            serde_json::from_str(r#"{"not_evaluated":true,"rationale":"n/a"}"#).unwrap();
        assert_eq!(ne.score(), None);
    }

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
        let err = run_review(input, &cfg, Path::new("/tmp"), &|_| {}).unwrap_err();
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
