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
2. Grade each in-scope dimension 0–10 with a short rationale.
3. Emit **specific, actionable** findings (see methodology). Respect max findings: **{{max_findings}}**.
4. List at least one **good thing** if any exist.
5. List **limitations** (what you could not verify).
6. List **suggested_next_steps** for the author (ordered, most important first).

## Output contract

Respond with **one** fenced JSON block labeled `json` containing an object that validates against this schema:

```json
{{output_schema}}
```

Rules:

- Do not wrap the JSON in commentary inside the fence.
- `summary.overall_score` and top-level `overall_score` must match (rounded mean of evaluated grades).
- Every finding must include `id`, `dimension`, `severity`, `location`, `observation`, `why_it_matters`, `recommended_action`.
- Use severity `nit` only for optional polish.
- If change context is empty, grade `cl_description` as not evaluated and explain in `limitations`.

After the JSON block you may add a brief human summary; the pipeline extracts only the first ` ```json ` block.
