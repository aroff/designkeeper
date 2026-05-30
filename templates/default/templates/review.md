# Task: Review code (structured rubric)

You are performing a structured code review for DesignKeeper. Work in the repository at **`{{working_dir}}`**. Read source files, tests, and docs from disk; do not invent paths or line numbers.

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
3. Grade each in-scope dimension 0–10 with a short rationale. If a focus area is listed in **Optional focus areas**, apply the corresponding sub-rubric from the methodology's "Focus area sub-rubrics" section.
4. Emit **specific, actionable** findings (see methodology). Respect max findings: **{{max_findings}}**.
5. List at least one **good thing** if any exist.
6. List **limitations** (what you could not verify).
7. List **suggested_next_steps** for the author (ordered, most important first).

**Edge cases:**
- **Partial changes:** grade only the dimensions visible in the diff; mark others `not_evaluated` with a note.
- **Generated code:** note it in `limitations`; do not grade generated files unless they contain business logic.
- **Empty change context:** grade `cl_description` as `not_evaluated` and explain in `limitations`.

## Output contract

Respond with **one** fenced JSON block labeled `json` containing an object that validates against this schema:

```json
{{output_schema}}
```

Rules:

- Do not wrap the JSON in commentary inside the fence.
- `summary.overall_score` and top-level `overall_score` must match (rounded mean of evaluated grades).
- Every finding must include `id`, `dimension`, `severity`, `location`, `observation`, `why_it_matters`, `recommended_action`.
- Use severity `nit` only for optional polish — cosmetic preferences, minor naming deviations, style inconsistencies with no correctness or readability impact, e.g. "prefer `snake_case` for variable name" or "trailing whitespace on line 12". A `nit` must never block merge.
- Optionally include `suggested_patch` in a finding when you can show the exact fix as a short diff or code snippet (max 2000 characters). Example: `"suggested_patch": "- old_line\n+ new_line"`.
- If change context is empty, grade `cl_description` as not evaluated and explain in `limitations`.

**Good finding example:**

```json
{
  "id": "design-001",
  "dimension": "design",
  "severity": "major",
  "location": "src/ordering/processor.rs:12-45",
  "observation": "OrderProcessor.process() mixes HTTP, retry, and persistence in one method.",
  "why_it_matters": "Mixed responsibilities make failure modes hard to reason about.",
  "recommended_action": "Extract HttpClient and RetryExecutor; keep OrderProcessor as orchestration only.",
  "suggested_patch": "- fn process(&self, order: Order) -> Result<()> {\n+ fn process(&self, order: Order) -> Result<()> {\n+     self.retry_executor.run(|| self.http_client.submit(&order))?;"
}
```

**Bad finding example (too vague — do not emit):**

```json
{
  "id": "style-001",
  "dimension": "style",
  "severity": "minor",
  "location": "src/",
  "observation": "Code is messy.",
  "why_it_matters": "Hard to read.",
  "recommended_action": "Clean it up."
}
```

After the JSON block you may add a brief human summary; the pipeline extracts only the first ` ```json ` block.
