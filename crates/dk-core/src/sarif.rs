//! SARIF 2.1.0 projection for `ReviewOutput`.

use std::collections::BTreeMap;

use serde_sarif::sarif::{
    ArtifactChange, ArtifactContent, ArtifactLocation, Fix, Invocation, Location, LogicalLocation,
    Message, MultiformatMessageString, PhysicalLocation, PropertyBag, Region, Replacement,
    ReportingDescriptor, Result as SarifResult, ResultKind, ResultLevel, Run, Sarif, Tool,
    ToolComponent,
};

use crate::types::{Dimension, Finding, ReviewOutput, Severity};

/// Metadata about the tool invocation; populated by the CLI crate.
#[derive(Debug, Clone)]
pub struct SarifRunMeta {
    pub tool_name: String,
    pub tool_version: String,
    pub agent_key: String,
    pub model: Option<String>,
}

/// Project a validated `ReviewOutput` into a SARIF 2.1.0 document.
///
/// Pure function — no I/O, no `Result`. Infallible given valid `ReviewOutput`.
pub fn to_sarif(output: &ReviewOutput, meta: &SarifRunMeta) -> Sarif {
    let rules = dimension_rules();
    let results: Vec<SarifResult> = output.findings.iter().map(finding_to_result).collect();

    // Build invocation with agent metadata in properties.
    let mut inv_props_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    inv_props_map.insert("dk/agent".to_string(), serde_json::json!(meta.agent_key));
    if let Some(model) = &meta.model {
        inv_props_map.insert("dk/model".to_string(), serde_json::json!(model));
    }
    let inv_props = PropertyBag::builder()
        .additional_properties(inv_props_map)
        .build();
    let invocation = Invocation::builder()
        .execution_successful(true)
        .properties(inv_props)
        .build();

    // Build run.properties with verdict/score/grades.
    let grades_json: serde_json::Map<String, serde_json::Value> = output
        .grades
        .iter()
        .map(|(dim, grade)| {
            (
                dim.as_key(),
                serde_json::to_value(grade).unwrap_or(serde_json::Value::Null),
            )
        })
        .collect();
    let mut run_props_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    run_props_map.insert(
        "dk/verdict".to_string(),
        serde_json::json!(output.summary.verdict.as_key()),
    );
    run_props_map.insert(
        "dk/overall_score".to_string(),
        serde_json::json!(output.overall_score),
    );
    run_props_map.insert(
        "dk/grades".to_string(),
        serde_json::Value::Object(grades_json),
    );
    let run_props = PropertyBag::builder()
        .additional_properties(run_props_map)
        .build();

    // Build tool.
    let mut driver = ToolComponent::builder()
        .name(meta.tool_name.clone())
        .rules(rules)
        .build();
    if !meta.tool_version.is_empty() {
        driver.version = Some(meta.tool_version.clone());
    }
    let tool = Tool::from(driver);

    // Build run.
    let run = Run::builder()
        .tool(tool)
        .invocations(vec![invocation])
        .results(results)
        .properties(run_props)
        .build();

    Sarif::builder()
        .version(serde_json::json!("2.1.0"))
        .schema(serde_sarif::sarif::SCHEMA_URL.to_string())
        .runs(vec![run])
        .build()
}

/// Build the `tool.driver.rules` array; one entry per Dimension variant (13 total).
fn dimension_rules() -> Vec<ReportingDescriptor> {
    use Dimension::*;

    static RULES: &[(Dimension, &str)] = &[
        (OverallCodeHealth, "Overall code health"),
        (ClDescription, "CL description"),
        (ChangeScope, "Change scope"),
        (Design, "Design"),
        (Functionality, "Functionality"),
        (Complexity, "Complexity"),
        (Tests, "Tests"),
        (Naming, "Naming"),
        (Comments, "Comments"),
        (Style, "Style"),
        (Consistency, "Consistency"),
        (Documentation, "Documentation"),
        (ContextAndReviewDepth, "Context and review depth"),
    ];

    let mut rule_props_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    rule_props_map.insert("dk/category".to_string(), serde_json::json!("code-review"));
    let rule_props = PropertyBag::builder()
        .additional_properties(rule_props_map)
        .build();

    RULES
        .iter()
        .map(|(dim, label)| {
            ReportingDescriptor::builder()
                .id(dim.as_key())
                .short_description(
                    MultiformatMessageString::builder()
                        .text(label.to_string())
                        .build(),
                )
                .properties(rule_props.clone())
                .build()
        })
        .collect()
}

/// Project a single `Finding` into a SARIF `result`.
fn finding_to_result(f: &Finding) -> SarifResult {
    let level = match f.severity {
        Severity::Blocker | Severity::Major => ResultLevel::Error,
        Severity::Minor => ResultLevel::Warning,
        Severity::Nit => ResultLevel::Note,
    };

    let message_text = format!("{}. {}", f.observation, f.why_it_matters);
    let message = Message::builder()
        .text(message_text)
        .markdown(f.recommended_action.clone())
        .build();

    // Stable fingerprint.
    let stable_fp = stable_fingerprint(&f.dimension.as_key(), &f.location, &f.observation);

    let mut fingerprints: BTreeMap<String, String> = BTreeMap::new();
    fingerprints.insert("dk/v1".to_string(), f.id.clone());
    fingerprints.insert("dk/stable-v1".to_string(), stable_fp);

    // Result properties.
    let mut props_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    props_map.insert(
        "dk/severity".to_string(),
        serde_json::json!(f.severity.as_key()),
    );

    // Location handling.
    let (locations_vec, extra_props) = build_locations(f);
    for (k, v) in extra_props {
        props_map.insert(k, v);
    }
    let result_props = PropertyBag::builder()
        .additional_properties(props_map)
        .build();

    // Pre-compute optional collections to avoid conditional TypedBuilder calls.
    let locations_opt = if locations_vec.is_empty() {
        None
    } else {
        Some(locations_vec)
    };

    let fixes_opt = f.suggested_patch.as_ref().map(|patch| {
        let inserted = ArtifactContent::builder().text(patch.clone()).build();
        let deleted_region = Region::builder().build();
        let replacement = Replacement::builder()
            .deleted_region(deleted_region)
            .inserted_content(inserted)
            .build();
        let artifact_location = ArtifactLocation::builder().build();
        let artifact_change = ArtifactChange::builder()
            .artifact_location(artifact_location)
            .replacements(vec![replacement])
            .build();
        vec![Fix::builder()
            .artifact_changes(vec![artifact_change])
            .build()]
    });

    // Build base result then set optional fields directly on the struct.
    let mut result = SarifResult::builder()
        .rule_id(f.dimension.as_key())
        .level(level)
        .kind(ResultKind::Open)
        .message(message)
        .partial_fingerprints(fingerprints)
        .properties(result_props)
        .build();

    result.locations = locations_opt;
    result.fixes = fixes_opt;
    result
}

/// Build SARIF locations and extra property hints for a finding.
fn build_locations(f: &Finding) -> (Vec<Location>, Vec<(String, serde_json::Value)>) {
    if let Some((uri, start_line, end_line)) = parse_physical_location(&f.location) {
        let region = Region::builder()
            .start_line(start_line as i64)
            .end_line(end_line as i64)
            .build();
        let artifact_location = ArtifactLocation::builder().uri(uri).build();
        let phys = PhysicalLocation::builder()
            .artifact_location(artifact_location)
            .region(region)
            .build();
        let loc = Location::builder().physical_location(phys).build();
        (vec![loc], vec![])
    } else {
        let logical = LogicalLocation::builder().name(f.location.clone()).build();
        let loc = Location::builder().logical_locations(vec![logical]).build();
        let extra = vec![("dk/location-kind".to_string(), serde_json::json!("logical"))];
        (vec![loc], extra)
    }
}

/// Parse a physical location string (e.g. "src/foo.rs:42-48") into (uri, startLine, endLine).
/// Returns `None` if the string does not match the physical location pattern.
fn parse_physical_location(s: &str) -> Option<(String, u64, u64)> {
    // Pattern: ^([^:]+):(\d+)(?:-(\d+))?$
    // Must have exactly one ':' separating the file from the line part.
    let colon_pos = s.rfind(':')?;
    let file = &s[..colon_pos];
    let line_part = &s[colon_pos + 1..];

    if file.is_empty() || line_part.is_empty() {
        return None;
    }

    // line_part is either "NN" or "NN-MM"
    if let Some(dash_pos) = line_part.find('-') {
        let start_str = &line_part[..dash_pos];
        let end_str = &line_part[dash_pos + 1..];
        let start: u64 = start_str.parse().ok()?;
        let end: u64 = end_str.parse().ok()?;
        Some((file.to_string(), start, end))
    } else {
        let line: u64 = line_part.parse().ok()?;
        Some((file.to_string(), line, line))
    }
}

/// Compute the stable SHA-256 fingerprint from (dimension_key, location, observation).
/// Returns lowercase hex (64 chars).
fn stable_fingerprint(dimension_key: &str, location: &str, observation: &str) -> String {
    use sha2::Digest;
    let input = format!(
        "{}|{}|{}",
        normalize(dimension_key),
        normalize(location),
        normalize(observation)
    );
    let hash = sha2::Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Trim and collapse interior whitespace runs to a single ASCII space.
fn normalize(s: &str) -> String {
    let trimmed = s.trim();
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta() -> SarifRunMeta {
        SarifRunMeta {
            tool_name: "dk".to_string(),
            tool_version: "0.1.11".to_string(),
            agent_key: "claude".to_string(),
            model: Some("claude-opus-4-8".to_string()),
        }
    }

    fn load_fixture(name: &str) -> ReviewOutput {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/examples/output")
            .join(name);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {name}: {e}"));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {name}: {e}"))
    }

    #[test]
    fn test_dimension_rules_count() {
        assert_eq!(dimension_rules().len(), 13);
    }

    #[test]
    fn test_dimension_rules_ids_stable() {
        let rules = dimension_rules();
        let expected_ids = [
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
        for (rule, dim) in rules.iter().zip(expected_ids.iter()) {
            assert_eq!(rule.id, dim.as_key());
        }
    }

    #[test]
    fn test_severity_level_blocker() {
        let f = Finding {
            id: "x".to_string(),
            dimension: Dimension::Design,
            severity: Severity::Blocker,
            location: "src/foo.rs:1".to_string(),
            observation: "obs".to_string(),
            why_it_matters: "why".to_string(),
            recommended_action: "action".to_string(),
            evidence: None,
            suggested_patch: None,
        };
        let result = finding_to_result(&f);
        assert_eq!(result.level, Some(ResultLevel::Error));
    }

    #[test]
    fn test_severity_level_major() {
        let f = Finding {
            id: "x".to_string(),
            dimension: Dimension::Design,
            severity: Severity::Major,
            location: "src/foo.rs:1".to_string(),
            observation: "obs".to_string(),
            why_it_matters: "why".to_string(),
            recommended_action: "action".to_string(),
            evidence: None,
            suggested_patch: None,
        };
        let result = finding_to_result(&f);
        assert_eq!(result.level, Some(ResultLevel::Error));
    }

    #[test]
    fn test_severity_level_minor() {
        let f = Finding {
            id: "x".to_string(),
            dimension: Dimension::Design,
            severity: Severity::Minor,
            location: "src/foo.rs:1".to_string(),
            observation: "obs".to_string(),
            why_it_matters: "why".to_string(),
            recommended_action: "action".to_string(),
            evidence: None,
            suggested_patch: None,
        };
        let result = finding_to_result(&f);
        assert_eq!(result.level, Some(ResultLevel::Warning));
    }

    #[test]
    fn test_severity_level_nit() {
        let f = Finding {
            id: "x".to_string(),
            dimension: Dimension::Comments,
            severity: Severity::Nit,
            location: "src/foo.rs:1".to_string(),
            observation: "obs".to_string(),
            why_it_matters: "why".to_string(),
            recommended_action: "action".to_string(),
            evidence: None,
            suggested_patch: None,
        };
        let result = finding_to_result(&f);
        assert_eq!(result.level, Some(ResultLevel::Note));
    }

    #[test]
    fn test_physical_location_single_line() {
        let result = parse_physical_location("src/ordering/processor.rs:44");
        assert_eq!(
            result,
            Some(("src/ordering/processor.rs".to_string(), 44, 44))
        );
    }

    #[test]
    fn test_physical_location_range() {
        let result = parse_physical_location("src/ordering/processor.rs:12-180");
        assert_eq!(
            result,
            Some(("src/ordering/processor.rs".to_string(), 12, 180))
        );
    }

    #[test]
    fn test_logical_location_passthrough() {
        let f = Finding {
            id: "x".to_string(),
            dimension: Dimension::ClDescription,
            severity: Severity::Major,
            location: "PR description".to_string(),
            observation: "obs".to_string(),
            why_it_matters: "why".to_string(),
            recommended_action: "action".to_string(),
            evidence: None,
            suggested_patch: None,
        };
        let result = finding_to_result(&f);
        let locs = result.locations.as_ref().unwrap();
        let loc = &locs[0];
        // Must have logicalLocations, not physicalLocation.
        assert!(loc.physical_location.is_none());
        let ll = loc.logical_locations.as_ref().unwrap();
        assert_eq!(ll[0].name.as_deref(), Some("PR description"));
    }

    #[test]
    fn test_fingerprint_stability() {
        let fp1 = stable_fingerprint("design", "src/foo.rs:1", "some observation");
        let fp2 = stable_fingerprint("design", "src/foo.rs:1", "some observation");
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64);
        // Different inputs produce different fingerprints.
        let fp3 = stable_fingerprint("tests", "src/foo.rs:1", "some observation");
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn test_to_sarif_approve_fixture() {
        let output = load_fixture("approve.json");
        let sarif = to_sarif(&output, &test_meta());
        let run = &sarif.runs[0];
        let results = run.results.as_ref().unwrap();
        assert_eq!(results.len(), 1, "approve.json has exactly 1 finding");
    }

    #[test]
    fn test_to_sarif_request_changes_fixture() {
        let output = load_fixture("request-changes.json");
        let sarif = to_sarif(&output, &test_meta());
        let run = &sarif.runs[0];
        let results = run.results.as_ref().unwrap();
        assert_eq!(
            results.len(),
            6,
            "request-changes.json has exactly 6 findings"
        );
    }

    #[test]
    fn test_sarif_schema_valid() {
        let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sarif-schema/sarif-2.1.0.json");
        let schema_text = match std::fs::read_to_string(&schema_path) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("SARIF schema not found, skipping schema validation test");
                return;
            }
        };
        let schema_value: serde_json::Value = serde_json::from_str(&schema_text).unwrap();
        let output = load_fixture("approve.json");
        let sarif = to_sarif(&output, &test_meta());
        let sarif_json = serde_json::to_value(&sarif).unwrap();
        let validator = jsonschema::validator_for(&schema_value).unwrap();
        let errors: Vec<_> = validator.iter_errors(&sarif_json).collect();
        assert!(
            errors.is_empty(),
            "SARIF schema validation errors:\n{}",
            errors
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
