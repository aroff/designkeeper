//! Fixture-driven acceptance tests (spec §7, AC #14-#18 + smoke test).
//!
//! Consumes the spec fixtures under `specs/review/examples/` and the schemas
//! under `templates/default/schemas/`.

use std::path::{Path, PathBuf};

use aikit_sdk::AgentRunner;
use dk_core::config::default_config;
use dk_core::pipeline::{extract_json_block, validate_json};
use dk_core::{pack, review, ReviewInput, ReviewOutput, Verdict};
use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn read_fixture(rel: &str) -> String {
    let path = repo_root().join("specs/review").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_json(rel: &str) -> Value {
    serde_json::from_str(&read_fixture(rel)).expect("valid json fixture")
}

fn input_schema() -> Value {
    serde_json::from_str(pack::INPUT_SCHEMA).unwrap()
}

fn output_schema() -> Value {
    serde_json::from_str(pack::OUTPUT_SCHEMA).unwrap()
}

// ---- AC #14 / #15: input fixtures pass input schema ----------------------

#[test]
fn minimal_input_passes_schema() {
    let instance = read_json("examples/input/minimal.json");
    validate_json(&input_schema(), &instance).expect("minimal.json should validate");
}

#[test]
fn pr_context_input_passes_schema() {
    let instance = read_json("examples/input/with-pr-context.json");
    validate_json(&input_schema(), &instance).expect("with-pr-context.json should validate");
}

#[test]
fn invalid_input_fails_schema() {
    let instance: Value = serde_json::json!({ "target": "src/" });
    assert!(validate_json(&input_schema(), &instance).is_err());
}

// ---- AC-P2: suggested_patch > 2000 chars fails output schema ---------------

#[test]
fn suggested_patch_too_long_fails_output_schema() {
    let mut instance = read_json("examples/output/approve.json");
    let long_patch = "x".repeat(2001);
    instance["findings"][0]["suggested_patch"] = serde_json::Value::String(long_patch);
    assert!(
        validate_json(&output_schema(), &instance).is_err(),
        "expected schema validation to fail for suggested_patch > 2000 chars"
    );
}

// ---- AC #16 / #17: output fixtures pass output schema --------------------

#[test]
fn approve_output_passes_schema() {
    let instance = read_json("examples/output/approve.json");
    validate_json(&output_schema(), &instance).expect("approve.json should validate");
    let typed: ReviewOutput = serde_json::from_value(instance).unwrap();
    assert_eq!(typed.summary.verdict, Verdict::Approve);
}

#[test]
fn request_changes_output_passes_schema() {
    let instance = read_json("examples/output/request-changes.json");
    validate_json(&output_schema(), &instance).expect("request-changes.json should validate");
    let typed: ReviewOutput = serde_json::from_value(instance).unwrap();
    assert_eq!(typed.summary.verdict, Verdict::RequestChanges);
    assert!(typed.findings.iter().any(|f| f.id == "design-001"));
}

// ---- AC #18: extraction from raw agent response --------------------------

#[test]
fn extracts_and_parses_agent_response() {
    let raw = read_fixture("examples/agent-response/valid.md");
    let block = extract_json_block(&raw).expect("first ```json block");
    let typed: ReviewOutput = serde_json::from_str(&block).expect("parses to ReviewOutput");
    assert_eq!(typed.summary.verdict, Verdict::ApproveWithComments);
    assert!((typed.overall_score - 7.0).abs() < f64::EPSILON);
    let value: Value = serde_json::from_str(&block).unwrap();
    validate_json(&output_schema(), &value).expect("extracted block validates");
}

// ---- Full pipeline smoke test with a mock agent response -----------------

fn pack_and_workdir() -> (tempfile::TempDir, tempfile::TempDir) {
    let pack_dir = tempfile::tempdir().unwrap();
    pack::write_default_pack(pack_dir.path()).unwrap();
    let wd = tempfile::tempdir().unwrap();
    std::fs::write(wd.path().join("lib.rs"), "pub fn x() {}").unwrap();
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
fn end_to_end_run_review_with_recorded_response() {
    let (pack_dir, wd) = pack_and_workdir();
    let raw = read_fixture("examples/agent-response/valid.md");
    let (runner, _) = AgentRunner::with_mock(vec![Ok(raw)]);
    let output = review::run_review_with_runner(
        input_for(wd.path()),
        &default_config(),
        pack_dir.path(),
        runner,
        &|_| {},
    )
    .expect("review succeeds");
    assert_eq!(output.summary.verdict, Verdict::ApproveWithComments);

    let report = review::render_report(&output, pack_dir.path()).unwrap();
    assert!(report.contains("Code review grade report"));
    assert!(report.contains("documentation"));
    assert!(!report.contains("{{verdict}}"));
    assert!(!report.contains("{{grades_table}}"));
}

#[test]
fn run_review_reconciles_score_mismatch() {
    let (pack_dir, wd) = pack_and_workdir();
    let mut value = read_json("examples/output/approve.json");
    value["overall_score"] = serde_json::json!(2.0);
    let raw = format!("```json\n{value}\n```");
    let (runner, _) = AgentRunner::with_mock(vec![Ok(raw)]);
    let output = review::run_review_with_runner(
        input_for(wd.path()),
        &default_config(),
        pack_dir.path(),
        runner,
        &|_| {},
    )
    .expect("reconciliation should not fail");
    assert!(
        (output.summary.overall_score - output.overall_score).abs() < 1e-9,
        "scores must match after reconciliation: summary={} top_level={}",
        output.summary.overall_score,
        output.overall_score
    );
    let scored: Vec<f64> = output.grades.values().filter_map(|g| g.score()).collect();
    assert!(!scored.is_empty());
    let expected = (scored.iter().sum::<f64>() / scored.len() as f64 * 10.0).round() / 10.0;
    assert!(
        (output.summary.overall_score - expected).abs() < 0.01,
        "reconciled score {:.1} should be close to mean {:.1}",
        output.summary.overall_score,
        expected
    );
}

#[test]
fn run_review_input_validation_error() {
    let (pack_dir, wd) = pack_and_workdir();
    let mut input = input_for(wd.path());
    input.target = Some(String::new());
    let (runner, _) = AgentRunner::with_mock(vec![]);
    let err = review::run_review_with_runner(input, &default_config(), pack_dir.path(), runner, &|_| {})
        .unwrap_err();
    assert_eq!(err.code(), "DK_INPUT_VALIDATION");
}

#[test]
fn run_review_template_not_found() {
    let empty = tempfile::tempdir().unwrap();
    let wd = tempfile::tempdir().unwrap();
    let (runner, _) = AgentRunner::with_mock(vec![]);
    let err = review::run_review_with_runner(
        input_for(wd.path()),
        &default_config(),
        empty.path(),
        runner,
        &|_| {},
    )
    .unwrap_err();
    assert_eq!(err.code(), "DK_TEMPLATE_NOT_FOUND");
}
