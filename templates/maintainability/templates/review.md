# Task: Maintainability audit

You are performing a strict maintainability audit for DesignKeeper. Work in the repository at **`{{working_dir}}`**. Read source files, tests, and docs from disk; do not invent paths or line numbers.

Be **ambitious**: actively hunt for "code judo" moves that delete whole categories of complexity. Do not rubber-stamp working-but-messy code.

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
3. Grade each in-scope dimension 0–10 with a short rationale. Apply the score anchors: scores below 4 are **critical** and act as quality penalties.
4. Compute `overall_score` as the mean of all evaluated dimensions, then apply the critical penalty rule (−0.5 per dimension scored < 4, floor at 0).
5. Emit **specific, actionable** findings (see methodology). Respect max findings: **{{max_findings}}**. Prioritize critical and high; do not flood with medium/low when blockers exist. A finding's `dimension` must be a dimension you actually scored.
6. List at least one **good thing** if any exist — acknowledge choices that genuinely improve maintainability.
7. List **limitations** (what you could not verify).
8. List **suggested_next_steps** for the author ordered by severity (critical first).

**Edge cases:**
- **Partial changes:** grade only dimensions visible in the diff; mark others `not_evaluated` with a note. Do not emit findings against `not_evaluated` dimensions.
- **Generated code:** note it in `limitations`; do not grade generated files unless they contain business logic.
- **Empty change context:** grade only dimensions derivable from file content; note the absence of diff context in `limitations`.

## Output contract

Respond with **one** fenced JSON block labeled `json` containing an object that validates against this schema:

```json
{{output_schema}}
```

Rules:

- Do not wrap the JSON in commentary inside the fence.
- `summary.overall_score` and top-level `overall_score` must match (post-penalty rounded value).
- Every finding must include `id`, `dimension`, `severity`, `location`, `observation`, `why_it_matters`, `recommended_action`.
- `severity` must be `"critical"` when the dimension scores 0–3, `"high"` for 4–5, `"medium"` for 6–7, `"low"` for 8.
- A finding's `dimension` must appear in `grades` with a numeric score (not `not_evaluated`).
- For Newton compatibility, write `location` as `path/to/file.ext:START-END` when the critique is local, so it parses into a `{path, range}` object.
- Optionally include `suggested_patch` when you can show the exact fix as a short diff or snippet (max 2000 characters).

**Good finding example:**

```json
{
  "id": "simplification-ambition-001",
  "dimension": "simplification_ambition",
  "severity": "high",
  "location": "src/sync/reconciler.rs:120-260",
  "observation": "reconcile() reimplements a three-state machine via nested booleans (is_dirty, was_seen, needs_flush) threaded through five helpers.",
  "why_it_matters": "The boolean soup hides the real state model; every new case adds another flag and another branch, so the function grows spaghetti instead of staying legible.",
  "recommended_action": "Replace the three booleans with an explicit `enum SyncState`, drive transitions through a single match, and delete the helper indirection — the five helpers collapse into one dispatcher."
}
```

**Bad finding example (too vague — do not emit):**

```json
{
  "id": "abstraction-economy-001",
  "dimension": "abstraction_economy",
  "severity": "medium",
  "location": "src/",
  "observation": "Too many abstractions.",
  "why_it_matters": "Hard to maintain.",
  "recommended_action": "Simplify."
}
```

After the JSON block you may add a brief human summary; the pipeline extracts only the first ` ```json ` block.
