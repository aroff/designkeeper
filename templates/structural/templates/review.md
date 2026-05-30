# Task: Structural code quality review

You are performing a structural code quality review for DesignKeeper. Work in the repository at **`{{working_dir}}`**. Read source files, tests, and docs from disk; do not invent paths or line numbers.

## Methodology

{{methodology}}

## Change under review

**Working directory:** `{{working_dir}}`

**Target (focus paths):** {{target}}

**Change context:**

{{change_context}}

**Optional focus areas:** {{focus}}

**Project hints (consult if present):**

{{project_hints}}

## Instructions

1. Follow the review sequence in the methodology.
2. **Dimensions filter:** {{dimensions_filter}}
3. Grade each in-scope sub-dimension 0–10 with a short rationale. Apply the score anchors from the methodology: scores below 4 are **critical** and act as quality penalties.
4. Compute group scores (`structure_score`, `complexity_score`, `expressiveness_score`) as the mean of their three sub-dimensions.
5. Compute `overall_score` as the mean of all nine evaluated sub-dimensions, then apply the critical penalty rule (−0.5 per sub-dimension scored < 4, floor at 0).
6. Emit **specific, actionable** findings (see methodology). Respect max findings: **{{max_findings}}**. Prioritize critical and high findings; do not flood with medium/low when structural blockers exist.
7. List at least one **good thing** if any exist — acknowledge structural choices that genuinely improve health.
8. List **limitations** (what you could not verify).
9. List **suggested_next_steps** for the author ordered by severity (critical first).

**Edge cases:**
- **Partial changes:** grade only dimensions visible in the diff; mark others `not_evaluated` with a note.
- **Generated code:** note it in `limitations`; do not grade generated files unless they contain business logic.
- **Empty change context:** grade only structural dimensions derivable from file content; note absence of diff context in `limitations`.

## Output contract

Respond with **one** fenced JSON block labeled `json` containing an object that validates against this schema:

```json
{{output_schema}}
```

Rules:

- Do not wrap the JSON in commentary inside the fence.
- `summary.overall_score` and top-level `overall_score` must match (post-penalty rounded value).
- `summary.structure_score`, `summary.complexity_score`, and `summary.expressiveness_score` must be the unpenalized means of their respective sub-dimension groups.
- Every finding must include `id`, `dimension`, `severity`, `location`, `observation`, `why_it_matters`, `recommended_action`.
- `severity` must be `"critical"` when the dimension scores 0–3, `"high"` for 4–5, `"medium"` for 6–7, `"low"` for 8.
- Optionally include `suggested_patch` in a finding when you can show the exact fix as a short diff or code snippet (max 2000 characters).

**Good finding example:**

```json
{
  "id": "layer-integrity-001",
  "dimension": "layer_integrity",
  "severity": "critical",
  "location": "src/api/handler.rs:88-140",
  "observation": "EmailNotificationService is instantiated and called directly inside the HTTP handler, bypassing the service layer.",
  "why_it_matters": "Feature logic leaking into the API layer makes the handler untestable without a real SMTP server and couples HTTP concerns to domain logic.",
  "recommended_action": "Move notification logic to OrderService; inject it as a dependency; handler should call service.complete_order() only."
}
```

**Bad finding example (too vague — do not emit):**

```json
{
  "id": "structure-001",
  "dimension": "file_decomposition",
  "severity": "medium",
  "location": "src/",
  "observation": "Files are too big.",
  "why_it_matters": "Hard to maintain.",
  "recommended_action": "Split them up."
}
```

After the JSON block you may add a brief human summary; the pipeline extracts only the first ` ```json ` block.
