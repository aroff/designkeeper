//! Slot-value construction for the prompt (9 slots) and report (generic).

pub use prompt::{build_prompt_slots, slots_as_pairs};
pub use report::build_report_slots;

// ---------------------------------------------------------------------------
// Prompt slots
// ---------------------------------------------------------------------------

mod prompt {
    use std::collections::HashMap;
    use std::path::Path;

    use crate::config::DkConfig;
    use crate::discovery;
    use crate::pack;
    use crate::types::ReviewInput;

    /// Build the 9 required prompt-template slots.
    pub fn build_prompt_slots(
        input: &ReviewInput,
        config: &DkConfig,
        pack_dir: &Path,
    ) -> Result<HashMap<String, String>, std::io::Error> {
        let working_dir = Path::new(&input.working_dir);
        let working_dir_abs = std::fs::canonicalize(working_dir)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| input.working_dir.clone());

        let target = match &input.target {
            Some(t) => t.clone(),
            None => {
                let discovered = discovery::discover_paths(&config.scan, working_dir)?;
                if discovered.is_empty() {
                    "entire repository".to_string()
                } else {
                    discovered.join("\n")
                }
            }
        };

        let methodology = read_or_default(&pack::methodology_path(pack_dir), "")?;
        let output_schema = minify_schema(&pack::output_schema_path(pack_dir))?;

        let mut slots = HashMap::new();
        slots.insert("working_dir".to_string(), working_dir_abs);
        slots.insert("target".to_string(), target);
        slots.insert("change_context".to_string(), format_change_context(input));
        slots.insert("focus".to_string(), format_focus(input));
        slots.insert("project_hints".to_string(), format_project_hints(input));
        slots.insert("methodology".to_string(), methodology);
        slots.insert(
            "max_findings".to_string(),
            input.options.max_findings.to_string(),
        );
        slots.insert("output_schema".to_string(), output_schema);
        slots.insert(
            "dimensions_filter".to_string(),
            format_dimensions_filter(input),
        );
        Ok(slots)
    }

    /// Convert a slot map to a `Vec<(&str, &str)>` suitable for `aikit_sdk::TemplateRenderer::render`.
    pub fn slots_as_pairs(slots: &HashMap<String, String>) -> Vec<(&str, &str)> {
        slots
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    fn format_change_context(input: &ReviewInput) -> String {
        let Some(cc) = input.change_context.as_ref().filter(|cc| !cc.is_empty()) else {
            return "No PR/CL metadata supplied.".to_string();
        };
        let mut lines = Vec::new();
        if let Some(title) = &cc.title {
            lines.push(format!("Title: {title}"));
        }
        if let Some(desc) = &cc.description {
            lines.push("Description:".to_string());
            lines.push(desc.clone());
        }
        match (&cc.base_ref, &cc.head_ref) {
            (Some(base), Some(head)) => lines.push(format!("Base: {base} → Head: {head}")),
            (Some(base), None) => lines.push(format!("Base: {base}")),
            (None, Some(head)) => lines.push(format!("Head: {head}")),
            (None, None) => {}
        }
        if let Some(diff) = &cc.diff_stat {
            lines.push(format!("Diff stat: {diff}"));
        }
        lines.join("\n")
    }

    fn format_focus(input: &ReviewInput) -> String {
        if input.focus.is_empty() {
            "none".to_string()
        } else {
            input.focus.join(", ")
        }
    }

    fn format_project_hints(input: &ReviewInput) -> String {
        let Some(hints) = input.project_hints.as_ref().filter(|h| !h.is_empty()) else {
            return "none".to_string();
        };
        let mut lines = Vec::new();
        if let Some(sg) = &hints.style_guide {
            lines.push(format!("Style guide: {sg}"));
        }
        if let Some(c) = &hints.contributing {
            lines.push(format!("Contributing: {c}"));
        }
        if let Some(docs) = &hints.architecture_docs {
            if !docs.is_empty() {
                lines.push(format!("Architecture docs: {}", docs.join(", ")));
            }
        }
        lines.join("\n")
    }

    fn format_dimensions_filter(input: &ReviewInput) -> String {
        match &input.options.include_dimensions {
            Some(dims) if !dims.is_empty() => {
                let list = dims.join(", ");
                format!("Grade ONLY these dimensions: {list}; mark all others not_evaluated.")
            }
            _ => "Grade every in-scope dimension.".to_string(),
        }
    }

    fn read_or_default(path: &Path, fallback: &str) -> Result<String, std::io::Error> {
        match std::fs::read_to_string(path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(fallback.to_string()),
            Err(e) => Err(e),
        }
    }

    fn minify_schema(path: &Path) -> Result<String, std::io::Error> {
        let text = read_or_default(path, "")?;
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => Ok(v.to_string()),
            Err(e) => {
                tracing::warn!("failed to minify schema at {}: {e}", path.display());
                Ok(text)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::config::default_config;
        use crate::types::{ChangeContext, ReviewOptions};
        use tempfile::tempdir;

        fn input_with(working_dir: &str) -> ReviewInput {
            ReviewInput {
                working_dir: working_dir.to_string(),
                target: Some("src/".to_string()),
                change_context: None,
                focus: vec![],
                project_hints: None,
                options: ReviewOptions::default(),
            }
        }

        #[test]
        fn builds_all_nine_prompt_slots() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let wd = tempdir().unwrap();
            let slots = build_prompt_slots(
                &input_with(wd.path().to_str().unwrap()),
                &default_config(),
                dir.path(),
            )
            .unwrap();
            for key in [
                "working_dir",
                "target",
                "change_context",
                "focus",
                "project_hints",
                "methodology",
                "max_findings",
                "output_schema",
                "dimensions_filter",
            ] {
                assert!(slots.contains_key(key), "missing slot {key}");
            }
            assert_eq!(slots["target"], "src/");
            assert_eq!(slots["focus"], "none");
            assert_eq!(slots["project_hints"], "none");
            assert_eq!(slots["change_context"], "No PR/CL metadata supplied.");
            assert_eq!(slots["max_findings"], "25");
            assert!(!slots["output_schema"].contains('\n'));
            assert!(slots["output_schema"].contains("\"verdict\""));
            assert_eq!(
                slots["dimensions_filter"],
                "Grade every in-scope dimension."
            );
        }

        #[test]
        fn change_context_formats_bullets() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let wd = tempdir().unwrap();
            let mut input = input_with(wd.path().to_str().unwrap());
            input.change_context = Some(ChangeContext {
                title: Some("Add retry policy".to_string()),
                description: Some("Backoff for transient errors.".to_string()),
                base_ref: Some("main".to_string()),
                head_ref: Some("feature/x".to_string()),
                diff_stat: Some("4 files".to_string()),
            });
            input.focus = vec!["concurrency".to_string(), "security".to_string()];
            let slots = build_prompt_slots(&input, &default_config(), dir.path()).unwrap();
            assert!(slots["change_context"].contains("Title: Add retry policy"));
            assert!(slots["change_context"].contains("Base: main → Head: feature/x"));
            assert_eq!(slots["focus"], "concurrency, security");
        }

        #[test]
        fn discovery_used_when_target_absent() {
            let pack_dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", pack_dir.path());
            let wd = tempdir().unwrap();
            std::fs::write(wd.path().join("a.rs"), "fn a() {}").unwrap();
            let mut input = input_with(wd.path().to_str().unwrap());
            input.target = None;
            let slots = build_prompt_slots(&input, &default_config(), pack_dir.path()).unwrap();
            assert_eq!(slots["target"], "a.rs");
        }

        #[test]
        fn dimensions_filter_with_some_emits_only_clause() {
            let pack_dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", pack_dir.path());
            let wd = tempdir().unwrap();
            let mut input = input_with(wd.path().to_str().unwrap());
            input.options.include_dimensions = Some(vec!["alpha".to_string(), "beta".to_string()]);
            let slots = build_prompt_slots(&input, &default_config(), pack_dir.path()).unwrap();
            let filter = &slots["dimensions_filter"];
            assert!(filter.contains("ONLY"), "expected ONLY in: {filter}");
            assert!(filter.contains("alpha"), "expected 'alpha' in: {filter}");
            assert!(filter.contains("beta"), "expected 'beta' in: {filter}");
        }
    }
}

// ---------------------------------------------------------------------------
// Report slots
// ---------------------------------------------------------------------------

mod report {
    use std::collections::HashMap;
    use std::path::Path;

    use crate::contract::ContractValidator;
    use crate::pack;
    use crate::types::ReviewDocument;

    /// Build report-template slots from a validated `ReviewDocument`.
    ///
    /// All scalar keys in `summary` are automatically exposed as slots.
    /// Computed slots (`grades_table`, `findings_section`, etc.) are always present.
    pub fn build_report_slots(doc: &ReviewDocument, pack_dir: &Path) -> HashMap<String, String> {
        let mut slots = HashMap::new();

        // Flatten all summary scalar fields into slots.
        if let Some(summary) = doc.raw()["summary"].as_object() {
            for (key, val) in summary {
                match val {
                    serde_json::Value::String(s) => {
                        slots.insert(key.clone(), s.clone());
                    }
                    serde_json::Value::Number(n) => {
                        if let Some(f) = n.as_f64() {
                            slots.insert(key.clone(), format!("{f:.1}"));
                        }
                    }
                    other => {
                        let kind = if other.is_null() {
                            "null"
                        } else {
                            "complex type"
                        };
                        tracing::warn!(
                            "summary.{key} is not a string or number ({kind}); skipping slot"
                        );
                    }
                }
            }
        }

        // Severity order from Pack output schema for ordering findings.
        let severity_order = read_severity_order(pack_dir);

        // Computed slots.
        slots.insert("grades_table".to_string(), format_grades_table(doc));
        slots.insert(
            "findings_section".to_string(),
            format_findings(doc, &severity_order),
        );
        slots.insert(
            "good_things_section".to_string(),
            bullet_list_from_value(&doc.raw()["good_things"]),
        );
        slots.insert(
            "limitations_section".to_string(),
            bullet_list_from_value(&doc.raw()["limitations"]),
        );
        slots.insert(
            "suggested_next_steps_section".to_string(),
            numbered_list_from_value(&doc.raw()["suggested_next_steps"]),
        );
        slots.insert(
            "report_body".to_string(),
            serde_json::to_string_pretty(doc.raw()).unwrap_or_default(),
        );

        slots
    }

    fn read_severity_order(pack_dir: &Path) -> Option<Vec<String>> {
        let text = std::fs::read_to_string(pack::output_schema_path(pack_dir)).ok()?;
        let schema: serde_json::Value = serde_json::from_str(&text).ok()?;
        ContractValidator::severity_order(&schema)
    }

    fn format_grades_table(doc: &ReviewDocument) -> String {
        let Some(grades) = doc.raw()["grades"].as_object() else {
            return "| _none_ | | |".to_string();
        };
        if grades.is_empty() {
            return "| _none_ | | |".to_string();
        }
        let mut rows: Vec<String> = grades
            .iter()
            .map(|(key, entry)| {
                let score = if let Some(s) = entry["score"].as_f64() {
                    format!("{s:.1}")
                } else {
                    "N/A".to_string()
                };
                let rationale = entry["rationale"].as_str().unwrap_or("");
                format!("| {key} | {score} | {rationale} |")
            })
            .collect();
        rows.sort(); // stable, deterministic output
        rows.join("\n")
    }

    fn format_findings(doc: &ReviewDocument, severity_order: &Option<Vec<String>>) -> String {
        let findings = doc.findings();
        if findings.is_empty() {
            return "None.".to_string();
        }

        // Determine severity ordering.
        let order: Vec<String> = if let Some(ord) = severity_order {
            ord.clone()
        } else {
            // Fallback: collect unique severities, sort lexicographically descending.
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

        // Group findings by severity in schema order, within group sort by id.
        let mut lines: Vec<String> = Vec::new();
        for sev in &order {
            let mut group: Vec<&crate::types::Finding> =
                findings.iter().filter(|f| &f.severity == sev).collect();
            if group.is_empty() {
                continue;
            }
            group.sort_by_key(|f| f.id.as_str());
            for f in group {
                let mut s = format!(
                    "- [{}] {}: {} ({})",
                    f.severity, f.id, f.observation, f.location
                );
                if let Some(patch) = &f.suggested_patch {
                    s.push_str(&format!("\n  ```suggestion\n  {patch}\n  ```"));
                }
                lines.push(s);
            }
        }

        // Append any findings whose severity wasn't in the order list.
        let ordered_sevs: std::collections::HashSet<_> = order.iter().collect();
        let mut remainder: Vec<&crate::types::Finding> = findings
            .iter()
            .filter(|f| !ordered_sevs.contains(&f.severity))
            .collect();
        remainder.sort_by_key(|f| f.id.as_str());
        for f in remainder {
            let s = format!(
                "- [{}] {}: {} ({})",
                f.severity, f.id, f.observation, f.location
            );
            lines.push(s);
        }

        if lines.is_empty() {
            "None.".to_string()
        } else {
            lines.join("\n")
        }
    }

    fn bullet_list_from_value(val: &serde_json::Value) -> String {
        match val.as_array() {
            Some(arr) if !arr.is_empty() => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("- {s}"))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => "None.".to_string(),
        }
    }

    fn numbered_list_from_value(val: &serde_json::Value) -> String {
        match val.as_array() {
            Some(arr) if !arr.is_empty() => arr
                .iter()
                .filter_map(|v| v.as_str())
                .enumerate()
                .map(|(i, s)| format!("{}. {s}", i + 1))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => "None.".to_string(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::tempdir;

        fn minimal_doc() -> ReviewDocument {
            ReviewDocument::from_value(serde_json::json!({
                "summary": {
                    "verdict": "approve",
                    "overall_score": 8,
                    "one_paragraph": "Looks good."
                },
                "grades": {
                    "alpha": { "score": 8, "rationale": "solid" },
                    "beta": { "score": 7, "rationale": "ok" }
                },
                "overall_score": 8,
                "good_things": ["clean code"],
                "findings": [
                    {
                        "id": "alpha-001",
                        "dimension": "alpha",
                        "severity": "low",
                        "location": "src/main.rs:1",
                        "observation": "minor nit",
                        "why_it_matters": "style",
                        "recommended_action": "fix it"
                    }
                ],
                "limitations": ["could not run tests"],
                "suggested_next_steps": ["address nit"]
            }))
        }

        #[test]
        fn summary_scalars_become_slots() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let slots = build_report_slots(&minimal_doc(), dir.path());
            assert_eq!(slots.get("verdict").map(|s| s.as_str()), Some("approve"));
            assert_eq!(slots.get("overall_score").map(|s| s.as_str()), Some("8.0"));
            assert!(slots.contains_key("one_paragraph"));
        }

        #[test]
        fn extra_summary_field_auto_exposed() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let doc = ReviewDocument::from_value(serde_json::json!({
                "summary": {
                    "verdict": "approve",
                    "overall_score": 8,
                    "one_paragraph": "ok",
                    "structure_score": 8.5
                },
                "grades": { "alpha": { "score": 8, "rationale": "ok" } },
                "overall_score": 8,
                "good_things": [],
                "findings": [],
                "limitations": [],
                "suggested_next_steps": ["step"]
            }));
            let slots = build_report_slots(&doc, dir.path());
            assert_eq!(
                slots.get("structure_score").map(|s| s.as_str()),
                Some("8.5")
            );
        }

        #[test]
        fn grades_table_contains_dimension_key() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let slots = build_report_slots(&minimal_doc(), dir.path());
            let table = slots.get("grades_table").unwrap();
            assert!(table.contains("alpha"), "table: {table}");
            assert!(table.contains("beta"), "table: {table}");
        }

        #[test]
        fn findings_with_patch_includes_fenced_block() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let doc = ReviewDocument::from_value(serde_json::json!({
                "summary": { "verdict": "approve", "overall_score": 8, "one_paragraph": "ok" },
                "grades": { "alpha": { "score": 8, "rationale": "ok" } },
                "overall_score": 8,
                "good_things": [],
                "findings": [{
                    "id": "f1",
                    "dimension": "alpha",
                    "severity": "low",
                    "location": "src/x.rs:1",
                    "observation": "obs",
                    "why_it_matters": "why",
                    "recommended_action": "fix",
                    "suggested_patch": "patch text"
                }],
                "limitations": [],
                "suggested_next_steps": ["step"]
            }));
            let slots = build_report_slots(&doc, dir.path());
            let findings = slots.get("findings_section").unwrap();
            assert!(findings.contains("patch text"), "findings: {findings}");
            assert!(findings.contains("```suggestion"), "findings: {findings}");
        }

        #[test]
        fn all_computed_slots_present() {
            let dir = tempdir().unwrap();
            crate::testutil::copy_fixture_pack("minimal", dir.path());
            let slots = build_report_slots(&minimal_doc(), dir.path());
            for key in [
                "grades_table",
                "findings_section",
                "good_things_section",
                "limitations_section",
                "suggested_next_steps_section",
                "report_body",
            ] {
                assert!(slots.contains_key(key), "missing computed slot: {key}");
            }
        }
    }
}
