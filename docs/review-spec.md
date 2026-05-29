# Review — implementation spec

Structured pipeline artifact for **`dk review`**: structured review rubric (default: Google eng-practices), 0–10 dimension scores, actionable findings. Consumed by `dk-core` and the aikit-sdk structured pipeline (template → agent → schema validation → report).

## Implementation reliance

Implementation MUST rely on the latest versions of `cli-framework` and `aikit-sdk` for as much of the work as possible:

- **cli-framework**: command registration, argument parsing, flag definitions, MCP server (`dk mcp`).
- **aikit-sdk**: structured pipeline — `TemplateRenderer`, `AgentRunner`, `ResponseValidator`, `ReportRenderer`, `Pipeline` composition, agent detection.

Custom logic in `dk-core` is limited to: file discovery, config resolution (`dk.toml` walk-up), init scaffolding, and domain-specific post-validation checks (Section: Validation rules).

## Command

```
dk review [<path>] [-a --agent] [-m --model] [--output-format] [--output-file]
                   [--title <text>] [--description <file|text>] [--base-ref <ref>] [--head-ref <ref>]
                   [--focus <area>]... [--max-findings <n>]
```

| Flag | Maps to `input.schema.json` |
|------|-----------------------------|
| `<path>` | `target` (optional); default from file discovery |
| cwd | `working_dir` |
| `--title`, `--description` | `change_context.title`, `change_context.description` |
| `--base-ref`, `--head-ref` | `change_context.base_ref`, `change_context.head_ref` |
| `--focus` (repeatable) | `focus[]` |
| `--max-findings` | `options.max_findings` |

## `dk check` relationship

`dk check` runs `review` internally and maps the verdict to exit codes:

- `approve` or `approve_with_comments` → exit 0
- `request_changes` or `reject` → exit 1

No separate pipeline or schema. `--verbose` on `check` produces the full scored report.

## Template pack layout

Installed under `.dk/` (source of truth in this spec folder until `dk init` packs it):

```
.dk/
├── templates/
│   ├── review.md           # copy of review/templates/prompt.md
│   └── methodology.md      # rubric (default: Google eng-practices); user-editable
├── schemas/
│   ├── review-input.json   # validate CLI-built input before render
│   └── review.json         # copy of review/schemas/output.schema.json
└── reports/
    └── review.md           # copy of review/templates/report.md
```

## Pipeline steps

1. **Build input** — Parse CLI flags into JSON; validate against `schemas/review-input.json`.
2. **Render prompt** — `TemplateRenderer::render(templates/review.md, slots)`.
3. **Run agent** — `AgentRunner` with `working_dir = input.working_dir`.
4. **Extract + validate** — First ` ```json ` block → `schemas/review.json`; retry up to 2 on failure.
5. **Render report** — Fill `templates/report.md` slots from validated JSON (see below); `--output-format json` skips report template.

## Prompt template slots

| Slot | Required | How `dk-core` fills it |
|------|----------|-------------------------|
| `working_dir` | yes | `input.working_dir` (absolute path) |
| `target` | yes | `input.target` or discovered paths string; use `entire repository` if empty |
| `change_context` | yes | Formatted markdown from `change_context` or `No PR/CL metadata supplied.` |
| `focus` | yes | Join `input.focus` or `none` |
| `project_hints` | yes | Format hints or `none` |
| `methodology` | yes | Read `templates/methodology.md` verbatim |
| `max_findings` | yes | `input.options.max_findings` default `25` |
| `output_schema` | yes | Minified `schemas/review.json` |

Unknown slots in the template MUST error (aikit-sdk R1).

## Report template slots

`dk-core` builds these from validated output before `ReportRenderer::render`:

| Slot | Source field |
|------|----------------|
| `verdict` | `summary.verdict` |
| `overall_score` | `summary.overall_score` |
| `one_paragraph` | `summary.one_paragraph` |
| `grades_table` | Markdown table from `grades` |
| `findings_section` | Bullet list grouped by severity |
| `good_things_section` | Bullet list |
| `limitations_section` | Bullet list or `None.` |
| `suggested_next_steps_section` | Numbered list |
| `report_body` | Optional pretty-printed JSON for debugging |

## Output extraction

- **Input to validator:** raw agent `String`.
- **Extraction rule:** first fenced block with info string `json` (case-insensitive), per aikit-sdk R3.
- **Example:** `examples/agent-response/valid.md`.

## Validation rules (beyond JSON Schema)

Implement in `dk-core` as warnings or post-validation checks:

1. `summary.overall_score` equals `overall_score` (tolerance 0.01).
2. Mean of graded dimensions (where `score` present) ≈ `overall_score` (±0.5); warn if drift.
3. `reject` verdict must not have `overall_score` > 6 unless documented in `limitations`.
4. Every `blocker` finding should correlate with `request_changes` or `reject` verdict.

## MCP

Tool name: `dk_review` — same input shape as HTTP.

## Examples (fixtures)

| Path | Purpose |
|------|---------|
| `examples/input/minimal.json` | Smallest valid CLI input |
| `examples/input/with-pr-context.json` | Full change metadata |
| `examples/output/approve.json` | Valid high-score output |
| `examples/output/request-changes.json` | Valid low-score output with blockers |
| `examples/agent-response/valid.md` | Raw agent text for extraction tests |
| `examples/slots/rendered-prompt-fragment.md` | Slot mapping documentation |

## Related docs

- Rubric source: Google [eng-practices](https://github.com/google/eng-practices) (`looking-for.md`, `standard.md`, …)
- Product vision: `specs/vision.md` — `dk review` command
