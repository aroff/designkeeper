//! Slot-value construction for the prompt (8 slots) and report (9 slots).

use std::collections::HashMap;
use std::path::Path;

use crate::config::DkConfig;
use crate::discovery;
use crate::pack;
use crate::review::{ReviewInput, ReviewOutput};

/// Build the 9 required prompt-template slots (spec §4.4).
pub fn build_prompt_slots(
    input: &ReviewInput,
    config: &DkConfig,
    template_dir: &Path,
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

    let methodology = read_or_default(&pack::methodology_path(template_dir), pack::METHODOLOGY)?;
    let output_schema = minify_schema(&pack::output_schema_path(template_dir))?;

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

/// Build the 9 report-template slots from validated output (spec §4.4).
pub fn build_report_slots(output: &ReviewOutput) -> HashMap<String, String> {
    let mut slots = HashMap::new();
    slots.insert(
        "verdict".to_string(),
        format!("{:?}", output.summary.verdict),
    );
    slots.insert(
        "overall_score".to_string(),
        format!("{:.1}", output.summary.overall_score),
    );
    slots.insert(
        "one_paragraph".to_string(),
        output.summary.one_paragraph.clone(),
    );
    slots.insert("grades_table".to_string(), format_grades_table(output));
    slots.insert("findings_section".to_string(), format_findings(output));
    slots.insert(
        "good_things_section".to_string(),
        bullet_list(&output.good_things),
    );
    slots.insert(
        "limitations_section".to_string(),
        bullet_list(&output.limitations),
    );
    slots.insert(
        "suggested_next_steps_section".to_string(),
        numbered_list(&output.suggested_next_steps),
    );
    slots.insert(
        "report_body".to_string(),
        serde_json::to_string_pretty(output).unwrap_or_default(),
    );
    slots
}

// ---- prompt helpers -------------------------------------------------------

fn format_change_context(input: &ReviewInput) -> String {
    let Some(cc) = &input.change_context else {
        return "No PR/CL metadata supplied.".to_string();
    };
    if cc.is_empty() {
        return "No PR/CL metadata supplied.".to_string();
    }
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
        input
            .focus
            .iter()
            .map(|f| f.as_key())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn format_project_hints(input: &ReviewInput) -> String {
    let Some(hints) = &input.project_hints else {
        return "none".to_string();
    };
    if hints.is_empty() {
        return "none".to_string();
    }
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
            let list = dims
                .iter()
                .map(|d| d.as_key())
                .collect::<Vec<_>>()
                .join(", ");
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
    let text = read_or_default(path, pack::OUTPUT_SCHEMA)?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => Ok(v.to_string()),
        Err(_) => Ok(text),
    }
}

// ---- report helpers -------------------------------------------------------

fn format_grades_table(output: &ReviewOutput) -> String {
    if output.grades.is_empty() {
        return "| _none_ | | |".to_string();
    }
    output
        .grades
        .iter()
        .map(|(dim, entry)| {
            let score = match entry.score() {
                Some(s) => format!("{s:.1}"),
                None => "N/A".to_string(),
            };
            format!("| {} | {} | {} |", dim.as_key(), score, entry.rationale())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_findings(output: &ReviewOutput) -> String {
    if output.findings.is_empty() {
        return "None.".to_string();
    }
    let mut findings: Vec<&crate::review::Finding> = output.findings.iter().collect();
    // Severity derives Ord in blocker-first declaration order.
    findings.sort_by_key(|f| f.severity);
    findings
        .iter()
        .map(|f| {
            let mut s = format!(
                "- [{}] {}: {} ({})",
                f.severity.as_key(),
                f.id,
                f.observation,
                f.location
            );
            if let Some(patch) = &f.suggested_patch {
                s.push_str(&format!("\n  ```suggestion\n  {patch}\n  ```"));
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        return "None.".to_string();
    }
    items
        .iter()
        .map(|i| format!("- {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn numbered_list(items: &[String]) -> String {
    if items.is_empty() {
        return "None.".to_string();
    }
    items
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {s}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::review::{
        ChangeContext, Dimension, Finding, FocusArea, ReviewOptions, ReviewOutput, Severity,
        Summary, Verdict,
    };
    use std::collections::BTreeMap;
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
        pack::write_default_pack(dir.path()).unwrap();
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
        // output_schema is minified JSON (no newlines).
        assert!(!slots["output_schema"].contains('\n'));
        assert!(slots["output_schema"].contains("\"verdict\""));
        // dimensions_filter defaults to grading everything.
        assert_eq!(
            slots["dimensions_filter"],
            "Grade every in-scope dimension."
        );
    }

    #[test]
    fn change_context_formats_bullets() {
        let dir = tempdir().unwrap();
        pack::write_default_pack(dir.path()).unwrap();
        let wd = tempdir().unwrap();
        let mut input = input_with(wd.path().to_str().unwrap());
        input.change_context = Some(ChangeContext {
            title: Some("Add retry policy".to_string()),
            description: Some("Backoff for transient errors.".to_string()),
            base_ref: Some("main".to_string()),
            head_ref: Some("feature/x".to_string()),
            diff_stat: Some("4 files".to_string()),
        });
        input.focus = vec![FocusArea::Concurrency, FocusArea::Security];
        let slots = build_prompt_slots(&input, &default_config(), dir.path()).unwrap();
        assert!(slots["change_context"].contains("Title: Add retry policy"));
        assert!(slots["change_context"].contains("Base: main → Head: feature/x"));
        assert_eq!(slots["focus"], "concurrency, security");
    }

    #[test]
    fn discovery_used_when_target_absent() {
        let pack_dir = tempdir().unwrap();
        pack::write_default_pack(pack_dir.path()).unwrap();
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
        pack::write_default_pack(pack_dir.path()).unwrap();
        let wd = tempdir().unwrap();
        let mut input = input_with(wd.path().to_str().unwrap());
        input.options.include_dimensions = Some(vec![Dimension::Design, Dimension::Tests]);
        let slots = build_prompt_slots(&input, &default_config(), pack_dir.path()).unwrap();
        let filter = &slots["dimensions_filter"];
        assert!(filter.contains("ONLY"), "expected ONLY in: {filter}");
        assert!(filter.contains("design"), "expected 'design' in: {filter}");
        assert!(filter.contains("tests"), "expected 'tests' in: {filter}");
    }

    fn minimal_output_with_finding(finding: Finding) -> ReviewOutput {
        ReviewOutput {
            summary: Summary {
                verdict: Verdict::Approve,
                overall_score: 7.0,
                one_paragraph: "ok".to_string(),
            },
            grades: BTreeMap::new(),
            overall_score: 7.0,
            good_things: vec![],
            findings: vec![finding],
            limitations: vec![],
            suggested_next_steps: vec![],
        }
    }

    fn base_finding() -> Finding {
        Finding {
            id: "design-001".to_string(),
            dimension: Dimension::Design,
            severity: Severity::Minor,
            location: "src/main.rs:1".to_string(),
            observation: "obs".to_string(),
            why_it_matters: "why".to_string(),
            recommended_action: "action".to_string(),
            evidence: None,
            suggested_patch: None,
        }
    }

    #[test]
    fn format_findings_with_suggested_patch_includes_fenced_block() {
        let mut f = base_finding();
        f.suggested_patch = Some("patch text".to_string());
        let output = minimal_output_with_finding(f);
        let rendered = format_findings(&output);
        assert!(
            rendered.contains("patch text"),
            "expected 'patch text' in: {rendered}"
        );
        assert!(
            rendered.contains("```suggestion"),
            "expected fenced block in: {rendered}"
        );
    }

    #[test]
    fn format_findings_without_suggested_patch_has_no_fenced_block() {
        let output = minimal_output_with_finding(base_finding());
        let rendered = format_findings(&output);
        assert!(
            !rendered.contains("```suggestion"),
            "unexpected fenced block in: {rendered}"
        );
    }
}
