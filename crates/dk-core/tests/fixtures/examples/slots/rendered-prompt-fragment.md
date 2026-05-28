# Example: slots after dk + aikit-sdk TemplateRenderer

Illustrates how `examples/input/with-pr-context.json` maps into `templates/prompt.md` slots (values truncated).

| Slot | Source |
|------|--------|
| `working_dir` | `input.working_dir` |
| `target` | `input.target` or file-discovery summary |
| `change_context` | YAML or markdown bullet list from `input.change_context` |
| `focus` | comma-separated `input.focus` or `none` |
| `project_hints` | formatted `input.project_hints` or `none` |
| `methodology` | full text of `templates/methodology.md` |
| `max_findings` | `input.options.max_findings` (default 25) |
| `output_schema` | compact JSON of `schemas/output.schema.json` |

`change_context` rendered example:

```
Title: Add retry policy to OrderProcessor
Description:
Introduce exponential backoff for transient DB errors...
Base: main → Head: feature/order-retry
Diff stat: 4 files changed, 186 insertions(+), 12 deletions(-)
```
