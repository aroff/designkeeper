//! Domain-specific post-validation (spec §4.5), run after JSON Schema passes.
//!
//! Score reconciliation is enforced as a hard overwrite in
//! [`crate::review::run_review`]. V3-V4 are non-blocking warnings returned here
//! and surfaced via `tracing::warn!`.

use crate::review::{ReviewOutput, Severity, Verdict};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationWarning {
    pub rule: String,
    pub message: String,
}

impl ValidationWarning {
    fn new(rule: &str, message: String) -> Self {
        Self {
            rule: rule.to_string(),
            message,
        }
    }
}

/// Apply non-blocking checks V3-V4 and return any warnings.
pub fn validate_output(output: &ReviewOutput) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    let scores: Vec<f64> = output.grades.values().filter_map(|g| g.score()).collect();
    if !scores.is_empty() {
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        if (mean - output.overall_score).abs() > crate::review::MEAN_DRIFT_TOLERANCE {
            warnings.push(ValidationWarning::new(
                "V2",
                format!(
                    "mean of scored dimensions ({mean:.2}) drifts from overall_score ({:.2}) by more than {}",
                    output.overall_score,
                    crate::review::MEAN_DRIFT_TOLERANCE
                ),
            ));
        }
    }

    // V3: reject with overall_score > 6 must document why in limitations.
    if output.summary.verdict == Verdict::Reject
        && output.overall_score > 6.0
        && output.limitations.is_empty()
    {
        warnings.push(ValidationWarning::new(
            "V3",
            format!(
                "verdict is reject with overall_score {:.2} (>6) but limitations is empty",
                output.overall_score
            ),
        ));
    }

    // V4: blocker findings should correlate with request_changes or reject.
    let blocker_ids: Vec<&str> = output
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Blocker)
        .map(|f| f.id.as_str())
        .collect();
    if !blocker_ids.is_empty() && output.summary.verdict.is_pass() {
        warnings.push(ValidationWarning::new(
            "V4",
            format!(
                "{} blocker finding(s) [{}] but verdict is {} (expected request_changes or reject)",
                blocker_ids.len(),
                blocker_ids.join(", "),
                output.summary.verdict.as_key()
            ),
        ));
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{Dimension, Finding, GradeEntry, Summary};
    use std::collections::BTreeMap;

    fn grade(score: f64) -> GradeEntry {
        GradeEntry::Scored {
            score,
            rationale: "r".to_string(),
        }
    }

    fn base_output(verdict: Verdict, overall: f64) -> ReviewOutput {
        let mut grades = BTreeMap::new();
        grades.insert(Dimension::Design, grade(overall));
        grades.insert(Dimension::Tests, grade(overall));
        ReviewOutput {
            summary: Summary {
                verdict,
                overall_score: overall,
                one_paragraph: "ok".to_string(),
            },
            grades,
            overall_score: overall,
            good_things: vec![],
            findings: vec![],
            limitations: vec![],
            suggested_next_steps: vec!["step".to_string()],
        }
    }

    fn blocker() -> Finding {
        Finding {
            id: "design-001".to_string(),
            dimension: Dimension::Design,
            severity: Severity::Blocker,
            location: "src/x.rs:1".to_string(),
            observation: "bad thing happens here".to_string(),
            why_it_matters: "it matters a lot".to_string(),
            recommended_action: "fix the thing now".to_string(),
            evidence: None,
            suggested_patch: None,
        }
    }

    #[test]
    fn v2_no_warning_when_consistent() {
        let out = base_output(Verdict::Approve, 8.0);
        assert!(validate_output(&out).iter().all(|w| w.rule != "V2"));
    }

    #[test]
    fn v2_warns_on_drift() {
        let mut out = base_output(Verdict::Approve, 8.0);
        // grades mean is 8.0, but claim overall 2.0 -> drift.
        out.overall_score = 2.0;
        out.summary.overall_score = 2.0;
        let w = validate_output(&out);
        assert!(w.iter().any(|w| w.rule == "V2"));
    }

    #[test]
    fn v3_warns_reject_high_score_empty_limitations() {
        let out = base_output(Verdict::Reject, 7.0);
        assert!(validate_output(&out).iter().any(|w| w.rule == "V3"));
    }

    #[test]
    fn v3_no_warning_when_limitations_present() {
        let mut out = base_output(Verdict::Reject, 7.0);
        out.limitations = vec!["could not run tests".to_string()];
        assert!(validate_output(&out).iter().all(|w| w.rule != "V3"));
    }

    #[test]
    fn v4_warns_blocker_with_approve() {
        let mut out = base_output(Verdict::Approve, 8.0);
        out.findings = vec![blocker()];
        assert!(validate_output(&out).iter().any(|w| w.rule == "V4"));
    }

    #[test]
    fn v4_no_warning_blocker_with_request_changes() {
        let mut out = base_output(Verdict::RequestChanges, 5.0);
        out.findings = vec![blocker()];
        assert!(validate_output(&out).iter().all(|w| w.rule != "V4"));
    }
}
