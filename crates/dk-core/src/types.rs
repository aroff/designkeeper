//! Domain type definitions for the review pipeline.
//!
//! These are the stable public API types that mirror `schemas/input.schema.json`
//! and `schemas/output.schema.json`. They have no dependency on orchestration code.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

fn parse_enum_from_str<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.trim().to_ascii_lowercase())).ok()
}

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

    pub fn parse(s: &str) -> Option<Self> { parse_enum_from_str(s) }
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

    pub fn parse(s: &str) -> Option<Self> { parse_enum_from_str(s) }
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
    pub fn as_key(&self) -> &'static str {
        match self {
            Verdict::Approve => "approve",
            Verdict::ApproveWithComments => "approve_with_comments",
            Verdict::RequestChanges => "request_changes",
            Verdict::Reject => "reject",
        }
    }

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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
    fn focus_area_parse_round_trips_all_variants() {
        let variants = [
            FocusArea::Security,
            FocusArea::Concurrency,
            FocusArea::Accessibility,
            FocusArea::Internationalization,
            FocusArea::Privacy,
            FocusArea::Performance,
            FocusArea::ApiDesign,
            FocusArea::Ui,
        ];
        for v in variants {
            assert_eq!(FocusArea::parse(v.as_key()), Some(v), "round-trip failed for {:?}", v);
        }
    }

    #[test]
    fn dimension_parse_round_trips_all_variants() {
        let variants = [
            Dimension::OverallCodeHealth,
            Dimension::ClDescription,
            Dimension::ChangeScope,
            Dimension::Design,
            Dimension::Functionality,
            Dimension::Complexity,
            Dimension::Tests,
            Dimension::Naming,
            Dimension::Comments,
            Dimension::Style,
            Dimension::Consistency,
            Dimension::Documentation,
            Dimension::ContextAndReviewDepth,
        ];
        for v in variants {
            assert_eq!(Dimension::parse(v.as_key()), Some(v), "round-trip failed for {:?}", v);
        }
    }
}
