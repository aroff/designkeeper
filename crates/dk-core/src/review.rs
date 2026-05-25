//! Review domain types and `run_review` orchestration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::DkConfig;
use crate::pack;
use crate::pipeline::{
    AgentRunner, DefaultRenderer, JsonResponseValidator, Pipeline, PipelineError, TemplateRenderer,
};
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
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error("score mismatch: summary.overall_score={summary_score} top-level overall_score={top_level_score}")]
    ScoreMismatch {
        summary_score: f64,
        top_level_score: f64,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ReviewError {
    pub fn code(&self) -> &'static str {
        match self {
            ReviewError::InputValidation { .. } => "DK_INPUT_VALIDATION",
            ReviewError::WorkingDirInvalid { .. } => "DK_WORKING_DIR_INVALID",
            ReviewError::Config(c) => c.code(),
            ReviewError::Pipeline(p) => p.code(),
            ReviewError::ScoreMismatch { .. } => "DK_SCORE_MISMATCH",
            ReviewError::Io(_) => "DK_IO_ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Run the review pipeline using the real subprocess agent from `config`.
pub fn run_review(
    input: ReviewInput,
    config: &DkConfig,
    template_dir: &Path,
) -> Result<ReviewOutput, ReviewError> {
    let agent = crate::pipeline::SubprocessAgent {
        agent: config.agent.agent.clone(),
        model: config.agent.model.clone(),
    };
    run_review_with_agent(input, config, template_dir, &agent)
}

/// Run the review pipeline against an injected agent (used by tests).
pub fn run_review_with_agent(
    input: ReviewInput,
    config: &DkConfig,
    template_dir: &Path,
    agent: &dyn AgentRunner,
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

    // 2. Build the 8 prompt slots.
    let prompt_slots = slots::build_prompt_slots(&input, config, template_dir)?;

    // 3. Load prompt template + output schema.
    let prompt_template = read_template(&pack::prompt_path(template_dir))?;
    let output_schema = read_schema(&pack::output_schema_path(template_dir))?;

    // 4. Run the structured pipeline (render -> agent -> extract + validate, retry).
    let renderer = DefaultRenderer;
    let validator = JsonResponseValidator;
    let pipeline = Pipeline::new(&renderer, agent, &validator);
    let value = pipeline.run(&prompt_template, &prompt_slots, working_dir, &output_schema)?;

    // The pipeline already schema-validated; deserialize into the typed output.
    let output: ReviewOutput =
        serde_json::from_value(value).map_err(|e| PipelineError::JsonParse {
            message: e.to_string(),
        })?;

    // 5. Post-validation. V1 is a hard error; V2-V4 are warnings.
    if (output.summary.overall_score - output.overall_score).abs() > SCORE_TOLERANCE {
        return Err(ReviewError::ScoreMismatch {
            summary_score: output.summary.overall_score,
            top_level_score: output.overall_score,
        });
    }
    for warning in validation::validate_output(&output) {
        tracing::warn!(rule = %warning.rule, "{}", warning.message);
    }

    Ok(output)
}

/// Render the markdown report for a validated review output.
pub fn render_report(output: &ReviewOutput, template_dir: &Path) -> Result<String, ReviewError> {
    let template = read_template(&pack::report_path(template_dir))?;
    let report_slots = slots::build_report_slots(output);
    let rendered = DefaultRenderer.render(&template, &report_slots)?;
    Ok(rendered)
}

fn read_template(path: &Path) -> Result<String, ReviewError> {
    std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ReviewError::Pipeline(PipelineError::TemplateNotFound {
                path: path.display().to_string(),
            })
        } else {
            ReviewError::Io(e)
        }
    })
}

fn read_schema(path: &Path) -> Result<Value, ReviewError> {
    let text = read_template(path)?;
    serde_json::from_str(&text).map_err(|e| {
        ReviewError::Pipeline(PipelineError::InvalidSchema {
            message: format!("{}: {e}", path.display()),
        })
    })
}

fn validate_against(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    crate::pipeline::validate_json(schema, instance)
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
        // Serialized form fills in default options.
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
        let err = run_review(input, &cfg, Path::new("/tmp")).unwrap_err();
        assert_eq!(err.code(), "DK_WORKING_DIR_INVALID");
    }
}
