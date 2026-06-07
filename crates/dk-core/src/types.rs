//! Domain type definitions for the review pipeline.
//!
//! Pack-agnostic: no rubric dimension, severity, or focus-area enums.
//! Only `Verdict` is a shared enum (part of the core contract and `dk check` exit semantics).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Input types
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
    pub focus: Vec<String>,
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
    pub include_dimensions: Option<Vec<String>>,
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
// Shared enum — core contract only
// ---------------------------------------------------------------------------

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
// Output types — Pack-agnostic wrappers
// ---------------------------------------------------------------------------

/// Lossless wrapper around validated agent output.
/// Contract fields are accessible via typed helpers; all Pack-specific fields
/// remain in the raw JSON value.
#[derive(Debug, Clone)]
pub struct ReviewDocument {
    value: serde_json::Value,
}

impl ReviewDocument {
    pub fn from_value(value: serde_json::Value) -> Self {
        Self { value }
    }

    /// Parse the contract `verdict` field from `summary.verdict`.
    pub fn verdict(&self) -> Option<Verdict> {
        let s = self.value["summary"]["verdict"].as_str()?;
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }

    /// Read `summary.overall_score`, falling back to top-level `overall_score`.
    pub fn overall_score(&self) -> Option<f64> {
        self.value["summary"]["overall_score"]
            .as_f64()
            .or_else(|| self.value["overall_score"].as_f64())
    }

    /// Deserialize `findings` array into `Vec<Finding>`.
    pub fn findings(&self) -> Vec<Finding> {
        self.value["findings"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Access the full raw JSON value.
    pub fn raw(&self) -> &serde_json::Value {
        &self.value
    }
}

/// A finding with Pack-defined dimension and severity strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub dimension: String,
    pub severity: String,
    pub location: String,
    pub observation: String,
    pub why_it_matters: String,
    pub recommended_action: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub suggested_patch: Option<String>,
}

/// Grade entry for a single dimension — shape is Pack-agnostic.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn input_focus_serializes_as_strings() {
        let input = ReviewInput {
            working_dir: ".".to_string(),
            target: None,
            change_context: None,
            focus: vec!["security".to_string(), "concurrency".to_string()],
            project_hints: None,
            options: ReviewOptions::default(),
        };
        let v = serde_json::to_value(&input).unwrap();
        assert_eq!(v["focus"][0], "security");
        assert_eq!(v["focus"][1], "concurrency");
    }

    #[test]
    fn verdict_pass_mapping() {
        assert!(Verdict::Approve.is_pass());
        assert!(Verdict::ApproveWithComments.is_pass());
        assert!(!Verdict::RequestChanges.is_pass());
        assert!(!Verdict::Reject.is_pass());
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
                serde_json::Value::String(expected.into()),
                "serde/as_key mismatch for {expected}"
            );
        }
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
    fn review_document_verdict_accessor() {
        let value = serde_json::json!({
            "summary": { "verdict": "approve", "overall_score": 8, "one_paragraph": "Good." },
            "grades": { "alpha": { "score": 8, "rationale": "ok" } },
            "overall_score": 8,
            "good_things": [],
            "findings": [],
            "limitations": [],
            "suggested_next_steps": ["step"]
        });
        let doc = ReviewDocument::from_value(value);
        assert_eq!(doc.verdict(), Some(Verdict::Approve));
        assert!((doc.overall_score().unwrap() - 8.0).abs() < f64::EPSILON);
        assert_eq!(doc.findings().len(), 0);
    }

    #[test]
    fn review_document_findings_accessor() {
        let value = serde_json::json!({
            "summary": { "verdict": "request_changes", "overall_score": 5, "one_paragraph": "Issues." },
            "grades": { "beta": { "score": 5, "rationale": "needs work" } },
            "overall_score": 5,
            "good_things": [],
            "findings": [
                {
                    "id": "alpha-001",
                    "dimension": "alpha",
                    "severity": "high",
                    "location": "src/main.rs:1",
                    "observation": "obs",
                    "why_it_matters": "matters",
                    "recommended_action": "fix"
                }
            ],
            "limitations": [],
            "suggested_next_steps": ["step"]
        });
        let doc = ReviewDocument::from_value(value);
        let findings = doc.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].dimension, "alpha");
        assert_eq!(findings[0].severity, "high");
    }
}
