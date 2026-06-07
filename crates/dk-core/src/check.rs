//! `dk check` — runs a review and maps the verdict to a process exit code.

use std::path::Path;

use aikit_sdk::AgentRunner;

use crate::config::DkConfig;
use crate::contract::ContractValidator;
use crate::pack;
use crate::review::{self, ProgressFn};
use crate::types::{ReviewDocument, ReviewInput};

/// Result of a `dk check` run.
pub struct CheckResult {
    /// True when the verdict passed (approve / approve_with_comments).
    pub passed: bool,
    /// Full scored report (markdown), populated when `verbose` is set and the
    /// review succeeded.
    pub report: Option<String>,
    /// Findings summary (grouped by severity, schema-order first) for stderr,
    /// populated when the check fails.
    pub findings_summary: Option<String>,
    /// Error code when the check did not pass: `DK_CHECK_FAILED` for a failing
    /// verdict, or the underlying [`crate::ReviewError::code`] if the review
    /// itself errored. `None` when the check passed.
    pub fail_code: Option<&'static str>,
}

impl CheckResult {
    pub fn exit_code(&self) -> i32 {
        match self.fail_code {
            None => 0,
            Some("DK_CHECK_FAILED") => 1,
            Some(_) => 2,
        }
    }
}

/// Run `check` against an injected `AgentRunner`.
pub fn run_check(
    input: ReviewInput,
    config: &DkConfig,
    pack_dir: &Path,
    verbose: bool,
    runner: AgentRunner,
    progress: &ProgressFn,
) -> CheckResult {
    match review::run_review(input, config, pack_dir, runner, progress) {
        Ok(doc) => {
            let passed = doc.verdict().map(|v| v.is_pass()).unwrap_or(false);
            let report = if verbose {
                review::render_report(&doc, pack_dir).ok()
            } else {
                None
            };
            let findings_summary = if passed {
                None
            } else {
                Some(findings_summary(&doc, pack_dir))
            };
            CheckResult {
                passed,
                report,
                findings_summary,
                fail_code: if passed {
                    None
                } else {
                    Some("DK_CHECK_FAILED")
                },
            }
        }
        Err(err) => CheckResult {
            passed: false,
            report: None,
            findings_summary: Some(format!("review failed [{}]: {err}", err.code())),
            fail_code: Some(err.code()),
        },
    }
}

/// Build a findings summary grouped by schema severity order (highest first).
fn findings_summary(doc: &ReviewDocument, pack_dir: &Path) -> String {
    let severity_order = read_severity_order(pack_dir);

    let verdict = doc.verdict().map(|v| v.as_key()).unwrap_or("unknown");
    let score = doc.overall_score().unwrap_or(0.0);

    let mut lines = vec![format!("Verdict: {} (score {:.1}/10)", verdict, score)];

    let findings = doc.findings();

    let order: Vec<String> = if let Some(ord) = severity_order {
        ord
    } else {
        let mut sevs: Vec<String> = findings
            .iter()
            .map(|f| f.severity.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        sevs.sort();
        sevs.reverse();
        sevs
    };

    for sev in &order {
        let group: Vec<&crate::types::Finding> =
            findings.iter().filter(|f| &f.severity == sev).collect();
        if group.is_empty() {
            continue;
        }
        lines.push(format!("{} ({}):", sev, group.len()));
        for f in group {
            lines.push(format!("  - {}: {} ({})", f.id, f.observation, f.location));
        }
    }

    // Append any findings whose severity wasn't in the order list.
    let ordered: std::collections::HashSet<_> = order.iter().collect();
    let mut remainder: Vec<&crate::types::Finding> = findings
        .iter()
        .filter(|f| !ordered.contains(&f.severity))
        .collect();
    remainder.sort_by_key(|f| f.id.as_str());
    for f in remainder {
        lines.push(format!("  - {}: {} ({})", f.id, f.observation, f.location));
    }

    lines.join("\n")
}

fn read_severity_order(pack_dir: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(pack::output_schema_path(pack_dir)).ok()?;
    let schema: serde_json::Value = serde_json::from_str(&text).ok()?;
    ContractValidator::severity_order(&schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::testutil::copy_fixture_pack;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn fixture(name: &str) -> String {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/examples/output")
            .join(name);
        std::fs::read_to_string(root).unwrap()
    }

    fn setup() -> (tempfile::TempDir, tempfile::TempDir) {
        let pack_dir = tempdir().unwrap();
        copy_fixture_pack("minimal", pack_dir.path());
        let wd = tempdir().unwrap();
        (pack_dir, wd)
    }

    fn input_for(wd: &Path) -> ReviewInput {
        ReviewInput {
            working_dir: wd.to_str().unwrap().to_string(),
            target: Some("src/".to_string()),
            change_context: None,
            focus: vec![],
            project_hints: None,
            options: Default::default(),
        }
    }

    #[test]
    fn approve_exits_pass() {
        let (pack_dir, wd) = setup();
        let raw = fixture("approve.json");
        let (runner, _) = AgentRunner::with_mock(vec![Ok(format!("```json\n{raw}\n```"))]);
        let res = run_check(
            input_for(wd.path()),
            &default_config(),
            pack_dir.path(),
            false,
            runner,
            &|_| {},
        );
        assert!(res.passed);
        assert!(res.findings_summary.is_none());
    }

    #[test]
    fn request_changes_exits_fail_with_summary() {
        let (pack_dir, wd) = setup();
        let raw = fixture("request-changes.json");
        let (runner, _) = AgentRunner::with_mock(vec![Ok(format!("```json\n{raw}\n```"))]);
        let res = run_check(
            input_for(wd.path()),
            &default_config(),
            pack_dir.path(),
            false,
            runner,
            &|_| {},
        );
        assert!(!res.passed);
        assert_eq!(res.fail_code, Some("DK_CHECK_FAILED"));
        let summary = res.findings_summary.unwrap();
        let high_pos = summary.find("high").unwrap();
        let low_pos = summary.find("low").unwrap();
        assert!(
            high_pos < low_pos,
            "high severity must appear before low in summary"
        );
    }

    #[test]
    fn verbose_produces_report() {
        let (pack_dir, wd) = setup();
        let raw = fixture("approve.json");
        let (runner, _) = AgentRunner::with_mock(vec![Ok(format!("```json\n{raw}\n```"))]);
        let res = run_check(
            input_for(wd.path()),
            &default_config(),
            pack_dir.path(),
            true,
            runner,
            &|_| {},
        );
        assert!(res.report.is_some());
        assert!(res.report.unwrap().contains("Code review grade report"));
    }
}
