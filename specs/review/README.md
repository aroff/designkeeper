# Review spec pack

Artifacts for **`dk review`**: review code against a structured rubric (default: Google eng-practices, 13 dimensions, 0–10 scores), with actionable findings. Used with the aikit-sdk structured pipeline (prompt interpolation, JSON extraction, schema validation, report render). The rubric ships as `templates/methodology.md` and is user-editable via the template pack.

## Contents

| Path | Role |
|------|------|
| [spec.md](./spec.md) | Command mapping, pipeline, slot contract |
| [schemas/input.schema.json](./schemas/input.schema.json) | CLI/API input validated before prompt render |
| [schemas/output.schema.json](./schemas/output.schema.json) | Agent JSON validated after extraction |
| [templates/prompt.md](./templates/prompt.md) | Agent prompt (`{{slots}}`) |
| [templates/methodology.md](./templates/methodology.md) | Rubric injected as `{{methodology}}` |
| [templates/report.md](./templates/report.md) | Markdown report layout |
| [examples/](./examples/) | Fixture input, output, and agent response |

## Quick test (manual)

1. Render `templates/prompt.md` with slots from `examples/input/with-pr-context.json` and `schemas/output.schema.json` inlined as `output_schema`.
2. Validate `examples/output/approve.json` against `schemas/output.schema.json`.
3. Parse `examples/agent-response/valid.md` and validate the extracted JSON the same way.

Implementation tracking: [specs/vision.md](../vision.md).
