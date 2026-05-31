//! Domain type definitions for the review pipeline.
//!
//! These are the stable public API types that mirror `schemas/input.schema.json`
//! and `schemas/output.schema.json`. They have no dependency on orchestration code.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

fn parse_enum_from_str<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.trim().to_ascii_lowercase())).ok()
}

fn serde_key<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|val| val.as_str().map(String::from))
        .unwrap_or_default()
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
        *self == Self::default()
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
    pub fn as_key(&self) -> String {
        serde_key(self)
    }

    pub fn parse(s: &str) -> Option<Self> {
        parse_enum_from_str(s)
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
        *self == Self::default()
    }
}

pub const DEFAULT_MAX_FINDINGS: u8 = 25;

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
            max_findings: DEFAULT_MAX_FINDINGS,
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
    pub fn as_key(&self) -> String {
        serde_key(self)
    }

    pub fn parse(s: &str) -> Option<Self> {
        parse_enum_from_str(s)
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
    pub fn as_key(&self) -> String {
        serde_key(self)
    }

    pub fn all() -> &'static [Severity] {
        &[
            Severity::Blocker,
            Severity::Major,
            Severity::Minor,
            Severity::Nit,
        ]
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
    pub fn as_key(&self) -> String {
        serde_key(self)
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

impl ReviewOutput {
    /// Returns the unrounded mean of all scored grade entries, or `None` if no grades are scored.
    pub fn mean_grade_score(&self) -> Option<f64> {
        let scores: Vec<f64> = self.grades.values().filter_map(|g| g.score()).collect();
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().sum::<f64>() / scores.len() as f64)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub verdict: Verdict,
    pub overall_score: f64,
    pub one_paragraph: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScoredGrade {
    pub score: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NotEvaluatedGrade {
    pub not_evaluated: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GradeEntry {
    Scored(ScoredGrade),
    NotEvaluated(NotEvaluatedGrade),
}

impl GradeEntry {
    pub fn score(&self) -> Option<f64> {
        match self {
            GradeEntry::Scored(g) => Some(g.score),
            GradeEntry::NotEvaluated(_) => None,
        }
    }

    pub fn rationale(&self) -> &str {
        match self {
            GradeEntry::Scored(g) => &g.rationale,
            GradeEntry::NotEvaluated(g) => &g.rationale,
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
    fn grade_entry_rejects_ambiguous_scored_and_not_evaluated() {
        let ambiguous = r#"{"score":7,"not_evaluated":true,"rationale":"x"}"#;
        let result: Result<GradeEntry, _> = serde_json::from_str(ambiguous);
        assert!(
            result.is_err(),
            "should reject JSON with both score and not_evaluated fields"
        );
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
            assert_eq!(
                FocusArea::parse(&v.as_key()),
                Some(v),
                "round-trip failed for {:?}",
                v
            );
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
            assert_eq!(
                Dimension::parse(&v.as_key()),
                Some(v),
                "round-trip failed for {:?}",
                v
            );
        }
    }

    #[test]
    fn severity_as_key_matches_serde() {
        let cases = [
            (Severity::Blocker, "blocker"),
            (Severity::Major, "major"),
            (Severity::Minor, "minor"),
            (Severity::Nit, "nit"),
        ];
        for (variant, expected) in cases {
            assert_eq!(variant.as_key(), expected);
            assert_eq!(
                serde_json::to_value(variant).unwrap(),
                Value::String(expected.into()),
                "serde/as_key mismatch for {expected}"
            );
        }
    }

    #[test]
    fn verdict_as_key_matches_serde() {
        let cases = [
            (Verdict::Approve, "approve"),
            (Verdict::ApproveWithComments, "approve_with_comments"),
            (Verdict::RequestChanges, "request_changes"),
            (Verdict::Reject, "reject"),
        ];
        for (variant, expected) in cases {
            assert_eq!(variant.as_key(), expected);
            assert_eq!(
                serde_json::to_value(variant).unwrap(),
                Value::String(expected.into()),
                "serde/as_key mismatch for {expected}"
            );
        }
    }
}
