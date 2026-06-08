//! Fixture-driven acceptance tests for the Pack-agnostic review pipeline.
//!
//! All fixture packs use the synthetic `minimal` Pack (tests/fixtures/packs/minimal/).
//! The default/structural templates are NOT referenced here.

use std::path::{Path, PathBuf};

use aikit_sdk::AgentRunner;
use dk_core::config::default_config;
use dk_core::testutil::copy_fixture_pack;
use dk_core::{review, ReviewDocument, ReviewInput, Verdict};
use serde_json::Value;

fn validate_json(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let validator =
        jsonschema::validator_for(schema).map_err(|e| vec![format!("invalid schema: {e}")])?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn extract_json_block(raw: &str) -> Option<String> {
    let mut lines = raw.lines();
    let mut collecting = false;
    let mut buf: Vec<&str> = Vec::new();
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if !collecting {
            if let Some(rest) = trimmed.strip_prefix("```") {
                if rest.trim().eq_ignore_ascii_case("json") {
                    collecting = true;
                }
            }
        } else if trimmed.starts_with("```") {
            return Some(buf.join("\n"));
        } else {
            buf.push(line);
        }
    }
    None
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_fixture(rel: &str) -> String {
    let path = fixture_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_json(rel: &str) -> Value {
    serde_json::from_str(&read_fixture(rel)).expect("valid json fixture")
}

fn minimal_pack_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packs/minimal")
}

fn input_schema() -> Value {
    let path = minimal_pack_dir().join("schemas/review-input.json");
    let s =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&s).expect("valid json schema")
}

fn output_schema() -> Value {
    let path = minimal_pack_dir().join("schemas/review.json");
    let s =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&s).expect("valid json schema")
}

// ---- Input fixtures pass minimal Pack input schema -----------------------

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

// ---- suggested_patch > 2000 chars fails output schema --------------------

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

// ---- Output fixtures pass minimal Pack output schema --------------------

#[test]
fn approve_output_passes_schema() {
    let instance = read_json("examples/output/approve.json");
    validate_json(&output_schema(), &instance).expect("approve.json should validate");
    let doc = ReviewDocument::from_value(instance);
    assert_eq!(doc.verdict(), Some(Verdict::Approve));
}

#[test]
fn request_changes_output_passes_schema() {
    let instance = read_json("examples/output/request-changes.json");
    validate_json(&output_schema(), &instance).expect("request-changes.json should validate");
    let doc = ReviewDocument::from_value(instance);
    assert_eq!(doc.verdict(), Some(Verdict::RequestChanges));
    assert_eq!(doc.findings().len(), 6);
    // All findings use alpha/beta dimensions.
    assert!(doc
        .findings()
        .iter()
        .all(|f| f.dimension == "alpha" || f.dimension == "beta"));
}

// ---- Extraction from raw agent response ----------------------------------

#[test]
fn extracts_and_parses_agent_response() {
    let raw = read_fixture("examples/agent-response/valid.md");
    let block = extract_json_block(&raw).expect("first ```json block");
    let value: Value = serde_json::from_str(&block).expect("parses to JSON");
    let doc = ReviewDocument::from_value(value.clone());
    assert_eq!(doc.verdict(), Some(Verdict::ApproveWithComments));
    assert!((doc.overall_score().unwrap() - 7.0).abs() < f64::EPSILON);
    validate_json(&output_schema(), &value).expect("extracted block validates");
}

// ---- Full pipeline smoke test with a mock agent response -----------------

fn pack_and_workdir() -> (tempfile::TempDir, tempfile::TempDir) {
    let pack_dir = tempfile::tempdir().unwrap();
    copy_fixture_pack("minimal", pack_dir.path());
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
    let doc = review::run_review(
        input_for(wd.path()),
        &default_config(),
        pack_dir.path(),
        runner,
        &|_| {},
    )
    .expect("review succeeds");
    assert_eq!(doc.verdict(), Some(Verdict::ApproveWithComments));

    let report = review::render_report(&doc, pack_dir.path()).unwrap();
    assert!(report.contains("Code review grade report"));
    assert!(report.contains("documentation"));
    assert!(!report.contains("{{verdict}}"));
    assert!(!report.contains("{{grades_table}}"));
}

// ---- AC-5: a third Pack with custom vocabulary works with no Rust change ----

/// A mock agent response for the `custom` Pack: dimensions x/y/z, severities
/// critical/info. None of these strings ever existed in the deleted Rust enums.
fn custom_agent_response() -> String {
    let json = r#"{
  "summary": {
    "verdict": "request_changes",
    "overall_score": 5.0,
    "structure_health": "needs work",
    "one_paragraph": "Custom-pack review: dimension z regressed and needs attention before merge."
  },
  "grades": {
    "x": { "score": 7, "rationale": "X dimension is acceptable." },
    "y": { "score": 6, "rationale": "Y dimension has minor issues." },
    "z": { "not_evaluated": true, "rationale": "Z could not be assessed from the diff." }
  },
  "overall_score": 5.0,
  "good_things": ["X dimension is well structured."],
  "findings": [
    {
      "id": "x-001",
      "dimension": "x",
      "severity": "critical",
      "location": "src/lib.rs:10",
      "observation": "A critical-severity problem in dimension x.",
      "why_it_matters": "It blocks merge under the custom rubric.",
      "recommended_action": "Address before merging."
    },
    {
      "id": "y-001",
      "dimension": "y",
      "severity": "info",
      "location": "src/lib.rs:20",
      "observation": "An informational note in dimension y.",
      "why_it_matters": "Worth knowing but not blocking.",
      "recommended_action": "Consider when convenient."
    }
  ],
  "limitations": ["Did not run the full suite."],
  "suggested_next_steps": ["Fix the critical x finding first."]
}"#;
    format!("Here is the custom review.\n\n```json\n{json}\n```\n")
}

#[test]
fn end_to_end_run_review_with_custom_pack() {
    let pack_dir = tempfile::tempdir().unwrap();
    copy_fixture_pack("custom", pack_dir.path());
    let wd = tempfile::tempdir().unwrap();
    std::fs::write(wd.path().join("lib.rs"), "pub fn x() {}").unwrap();

    let (runner, _) = AgentRunner::with_mock(vec![Ok(custom_agent_response())]);
    let doc = review::run_review(
        input_for(wd.path()),
        &default_config(),
        pack_dir.path(),
        runner,
        &|_| {},
    )
    .expect("custom-pack review succeeds with no Rust change");

    assert_eq!(doc.verdict(), Some(Verdict::RequestChanges));
    // Custom severities/dimensions survive the lossless document path.
    let findings = doc.findings();
    assert_eq!(findings.len(), 2);
    assert!(findings
        .iter()
        .any(|f| f.dimension == "x" && f.severity == "critical"));
    assert!(findings
        .iter()
        .any(|f| f.dimension == "y" && f.severity == "info"));

    // Report renders Pack vocabulary and the auto-exposed extra summary field.
    let report = review::render_report(&doc, pack_dir.path()).unwrap();
    assert!(report.contains("Custom review grade report"));
    assert!(!report.contains("{{grades_table}}"));
}

#[test]
fn run_review_rejects_input_failing_pack_schema() {
    // AC-8: include_dimensions value not allowed by the Pack input schema fails
    // with DK_INPUT_VALIDATION before the agent is ever invoked.
    let pack_dir = tempfile::tempdir().unwrap();
    copy_fixture_pack("custom", pack_dir.path());
    let wd = tempfile::tempdir().unwrap();
    std::fs::write(wd.path().join("lib.rs"), "pub fn x() {}").unwrap();

    let mut input = input_for(wd.path());
    input.options.include_dimensions = Some(vec!["alpha".to_string()]); // not in x/y/z

    // Empty mock queue: if the agent were invoked, run() would panic/fail —
    // proving validation happens before invocation.
    let (runner, _) = AgentRunner::with_mock(vec![]);
    let err =
        review::run_review(input, &default_config(), pack_dir.path(), runner, &|_| {}).unwrap_err();
    assert_eq!(err.code(), "DK_INPUT_VALIDATION");
}

#[test]
fn run_review_rejects_output_missing_contract_field() {
    // AC-9: output passes the Pack schema (summary.verdict is optional there for
    // this fixture) but violates the embedded core contract → DK_CONTRACT_VIOLATION.
    let pack_dir = tempfile::tempdir().unwrap();
    copy_fixture_pack("custom", pack_dir.path());
    let wd = tempfile::tempdir().unwrap();
    std::fs::write(wd.path().join("lib.rs"), "pub fn x() {}").unwrap();

    // Loosen the Pack output schema so summary.verdict is NOT required, letting a
    // verdict-less document pass the Pack gate and reach the core contract gate.
    let schema_path = pack_dir.path().join("schemas/review.json");
    let mut schema: Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["properties"]["summary"]["required"] =
        serde_json::json!(["overall_score", "one_paragraph"]);
    std::fs::write(&schema_path, serde_json::to_string_pretty(&schema).unwrap()).unwrap();

    let response = r#"```json
{
  "summary": { "overall_score": 8.0, "one_paragraph": "Looks fine but has no verdict." },
  "grades": { "x": { "score": 8, "rationale": "Good." } },
  "overall_score": 8.0,
  "good_things": [],
  "findings": [],
  "limitations": [],
  "suggested_next_steps": ["Ship it."]
}
```"#;
    let (runner, _) = AgentRunner::with_mock(vec![Ok(response.to_string())]);
    let err = review::run_review(
        input_for(wd.path()),
        &default_config(),
        pack_dir.path(),
        runner,
        &|_| {},
    )
    .unwrap_err();
    assert_eq!(err.code(), "DK_CONTRACT_VIOLATION");
}

#[test]
fn run_review_template_not_found() {
    let empty = tempfile::tempdir().unwrap();
    let wd = tempfile::tempdir().unwrap();
    let (runner, _) = AgentRunner::with_mock(vec![]);
    let err = review::run_review(
        input_for(wd.path()),
        &default_config(),
        empty.path(),
        runner,
        &|_| {},
    )
    .unwrap_err();
    assert_eq!(err.code(), "DK_TEMPLATE_NOT_FOUND");
}
