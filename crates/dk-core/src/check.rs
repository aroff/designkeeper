//! `dk check` — runs a review and maps the verdict to a process exit code.

use std::path::Path;
use std::process::ExitCode;

use aikit_sdk::AgentRunner;

use crate::config::DkConfig;
use crate::review::{self, build_agent_runner, ReviewInput, ReviewOutput, Severity};

/// Result of a `dk check` run.
pub struct CheckResult {
    /// Process exit code: SUCCESS for approve/approve_with_comments, else FAILURE.
    pub exit_code: ExitCode,
    /// True when the verdict passed (approve / approve_with_comments).
    pub passed: bool,
    /// Full scored report (markdown), populated when `verbose` is set and the
    /// review succeeded.
    pub report: Option<String>,
    /// Findings summary (grouped by severity, blockers first) for stderr,
    /// populated when the check fails.
    pub findings_summary: Option<String>,
    /// Error code when the check did not pass: `DK_CHECK_FAILED` for a failing
    /// verdict, or the underlying [`crate::ReviewError::code`] if the review
    /// itself errored. `None` when the check passed.
    pub fail_code: Option<&'static str>,
}

/// Run `check` using the real agent from `config`.
pub fn run_check(
    input: ReviewInput,
    config: &DkConfig,
    template_dir: &Path,
    verbose: bool,
    progress: &crate::pipeline::ProgressFn,
) -> CheckResult {
    let runner = build_agent_runner(config, &input);
    run_check_with_runner(input, config, template_dir, verbose, runner, progress)
}

/// Run `check` against an injected `AgentRunner` (used by tests).
pub fn run_check_with_runner(
    input: ReviewInput,
    config: &DkConfig,
    template_dir: &Path,
    verbose: bool,
    runner: AgentRunner,
    progress: &crate::pipeline::ProgressFn,
) -> CheckResult {
    match review::run_review_with_runner(input, config, template_dir, runner, progress) {
        Ok(output) => {
            let passed = output.summary.verdict.is_pass();
            let report = if verbose {
                review::render_report(&output, template_dir).ok()
            } else {
                None
            };
            let findings_summary = if passed {
                None
            } else {
                Some(findings_summary(&output))
            };
            CheckResult {
                exit_code: if passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                },
                passed,
                report,
                findings_summary,
                fail_code: if passed { None } else { Some("DK_CHECK_FAILED") },
            }
        }
        Err(err) => CheckResult {
            exit_code: ExitCode::FAILURE,
            passed: false,
            report: None,
            findings_summary: Some(format!("review failed [{}]: {err}", err.code())),
            fail_code: Some(err.code()),
        },
    }
}

/// Build a findings summary grouped by severity (blockers first).
fn findings_summary(output: &ReviewOutput) -> String {
    let mut lines = vec![format!(
        "Verdict: {:?} (score {:.1}/10)",
        output.summary.verdict, output.summary.overall_score
    )];
    let order = [
        Severity::Blocker,
        Severity::Major,
        Severity::Minor,
        Severity::Nit,
    ];
    for sev in order {
        let group: Vec<&crate::review::Finding> = output
            .findings
            .iter()
            .filter(|f| f.severity == sev)
            .collect();
        if group.is_empty() {
            continue;
        }
        lines.push(format!("{} ({}):", sev.as_key(), group.len()));
        for f in group {
            lines.push(format!("  - {}: {} ({})", f.id, f.observation, f.location));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::pack;
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
        pack::write_default_pack(pack_dir.path()).unwrap();
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
        let res = run_check_with_runner(
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
        let res = run_check_with_runner(
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
        let blocker_pos = summary.find("blocker").unwrap();
        let major_pos = summary.find("major").unwrap();
        assert!(blocker_pos < major_pos);
    }

    #[test]
    fn verbose_produces_report() {
        let (pack_dir, wd) = setup();
        let raw = fixture("approve.json");
        let (runner, _) = AgentRunner::with_mock(vec![Ok(format!("```json\n{raw}\n```"))]);
        let res = run_check_with_runner(
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
