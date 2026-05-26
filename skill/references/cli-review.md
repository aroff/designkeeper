# `dk review`

Run a structured, agent-driven review and emit a scored report.

## Synopsis

```
dk review [<path>]
          [-a --agent <a>] [-m --model <m>]
          [--output-format markdown|json] [--output-file <path>]
          [--title <text>] [--description <file|text>]
          [--base-ref <ref>] [--head-ref <ref>]
          [--focus <area>]...
          [--max-findings <1-50>]
```

## Flags

| Flag | Meaning |
|------|---------|
| `<path>` (positional) | Path/glob to focus the review. Omit to auto-discover (see below). |
| `-a, --agent <a>` | Agent key; overrides `dk.toml [agent].agent`. |
| `-m, --model <m>` | Model override; overrides `dk.toml [agent].model`. |
| `--output-format` | `markdown` (default) or `json`. Overrides `dk.toml [output].format`. |
| `--output-file <path>` | Write output here instead of stdout. |
| `--title <text>` | PR/CL title → change-context block. |
| `--description <file\|text>` | PR/CL description. If the value is an existing file path its contents are read; otherwise it is used as raw text. |
| `--base-ref` / `--head-ref` | Git refs for the change under review (e.g. `main` / `HEAD`). |
| `--focus <area>` | Repeatable. One of: `security`, `concurrency`, `accessibility`, `internationalization`, `privacy`, `performance`, `api_design`, `ui`. |
| `--max-findings <n>` | 1–50, default 25. Out-of-range is rejected with `DK_INPUT_VALIDATION`. |

## Which files get reviewed

- **With `<path>`**: the value is used verbatim as the `{{target}}` slot — no
  filtering; the agent honors it.
- **Without `<path>`**: `dk` auto-discovers source files under the working dir —
  those matching `[scan].extensions`, not gitignored (`.gitignore` honored even
  outside git), not hidden, and not excluded by `[scan].ignore_patterns`. The
  matched, sorted, repo-relative paths become the target; if none match, the
  whole repository is reviewed.

`dk` never sends file *contents* — the agent reads files itself. See
[configuration.md](configuration.md) for discovery + prompt details.

## How a review runs

1. `dk` builds a prompt from your template pack: the methodology, the target
   path(s), optional change context / focus areas, and the expected output
   schema.
2. It runs the agent on that prompt; the agent reads the source files itself.
3. It extracts the JSON block from the agent's reply and validates it against
   the pack's output schema (`.dk/schemas/review.json`).
4. On a validation failure it re-prompts with the errors appended — up to **2
   retries** (3 attempts total).
5. If the agent's reported scores are internally inconsistent, the review is
   rejected (`DK_SCORE_MISMATCH`).

## Output

A report with a verdict (`approve` / `approve_with_comments` /
`request_changes` / `reject`), an overall 0–10 score, per-dimension grades,
findings (severity `blocker`/`major`/`minor`/`nit`), good practices,
limitations, and suggested next steps. Markdown by default; `--output-format
json` emits the validated `ReviewOutput`.

## Progress

During the (slow) agent call, stderr shows a spinner with elapsed seconds and
the attempt number on a TTY, or plain stage lines when piped
(`dk: Reviewing with claude…`, `dk: validating response…`, retry notices).
stdout stays clean.

## Examples

```sh
dk review                                   # whole repo, markdown to stdout
dk review src/ --output-format json         # JSON for a subtree
dk review --focus security --focus privacy --max-findings 10 src/api
dk review --title "Add cache" --description PR_BODY.md --base-ref main --head-ref HEAD
dk review -a codex -m o3 --output-file review.md
```

## Errors / exit

- Non-zero exit on any pipeline error: `DK_AGENT_NOT_FOUND` (agent binary
  missing), `DK_PIPELINE_ERROR` (agent failed / exhausted retries),
  `DK_INPUT_VALIDATION`, `DK_TEMPLATE_NOT_FOUND`, `DK_SCORE_MISMATCH`,
  `DK_IO_ERROR`. Errors print `error [CODE]: message` to stderr.

## Gotchas

- Needs an agent CLI on `PATH`. Default `claude`; verify with `dk doctor`.
- Output is buffered (no streaming) — a real review can take 1–2 min.
- `--focus` values are a closed enum; an unknown value is rejected.
