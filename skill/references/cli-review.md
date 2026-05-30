# `dk review`

Run a structured, agent-driven review and emit a scored report.

## Synopsis

```
dk review --template <name> [<path>]
          [-a --agent <a>] [-m --model <m>]
          [--output-format markdown|json] [--output-file <path>]
          [--title <text>] [--description <file|text>]
          [--base-ref <ref>] [--head-ref <ref>]
          [--from-git <base-ref>]
          [--focus <area>]...
          [--max-findings <1-50>]
          [--include-dimensions <dim,...>]
```

## Flags

| Flag | Meaning |
|------|---------|
| `-t, --template <name>` | **Required.** Template pack to use (e.g. `default`, `structural`). |
| `<path>` (positional) | Path/glob to focus the review. Omit to auto-discover (see below). |
| `-a, --agent <a>` | Agent key; overrides `dk.toml [agent].agent`. |
| `-m, --model <m>` | Model override; overrides `dk.toml [agent].model`. |
| `--output-format` | `markdown` (default) or `json`. Overrides `dk.toml [output].format`. |
| `--output-file <path>` | Write output here instead of stdout. |
| `--title <text>` | PR/CL title → change-context block. |
| `--description <file\|text>` | PR/CL description. If the value is an existing file path its contents are read; otherwise used as raw text. |
| `--base-ref` / `--head-ref` | Git refs for the change under review (e.g. `main` / `HEAD`). |
| `--from-git <base-ref>` | Derive PR context from git: title from last commit, diff stat, changed files. |
| `--focus <area>` | Repeatable. One of: `security`, `concurrency`, `accessibility`, `internationalization`, `privacy`, `performance`, `api_design`, `ui`. |
| `--max-findings <n>` | 1–50, default 25. Out-of-range is rejected with `DK_INPUT_VALIDATION`. |
| `--include-dimensions <dims>` | Comma-separated dimensions to grade; others are `not_evaluated`. |

## Which files get reviewed

- **With `<path>`**: the value is used verbatim as the `{{target}}` slot.
- **Without `<path>`**: `dk` auto-discovers source files under the working dir —
  those matching `[scan].extensions`, not gitignored, not hidden, and not
  excluded by `[scan].ignore_patterns`. If none match, the whole repository is
  reviewed.

`dk` never sends file *contents* — the agent reads files itself. See
[configuration.md](configuration.md) for discovery + prompt details.

## How a review runs

1. `dk` resolves the named template pack (project-local → global → embedded).
2. Builds a prompt: methodology, target path(s), optional change context / focus
   areas, and the expected output schema.
3. Runs the agent; the agent reads source files itself.
4. Extracts the JSON block from the agent's reply and validates it against the
   pack's `schemas/review.json`.
5. On validation failure, re-prompts with errors appended — up to **2 retries**
   (3 attempts total).
6. If the agent's reported scores are internally inconsistent, the review is
   rejected (`DK_SCORE_MISMATCH`).

## Output

A report with a verdict (`approve` / `approve_with_comments` /
`request_changes` / `reject`), an overall 0–10 score, per-dimension grades,
findings, good practices, limitations, and suggested next steps. Markdown by
default; `--output-format json` emits the validated `ReviewOutput`.

The exact dimensions and scoring depend on the template pack used:
- `default`: 13 Google eng-practices dimensions, severity `blocker/major/minor/nit`.
- `structural`: 9 structural sub-dimensions across 3 groups, severity `critical/high/medium/low`.

## Progress

During the (slow) agent call, stderr shows a spinner with elapsed seconds on a
TTY, or plain stage lines when piped. stdout stays clean.

## Examples

```sh
dk review --template default                          # whole repo, markdown to stdout
dk review --template structural src/                  # structural review of src/
dk review --template default src/ --output-format json
dk review --template default --focus security --focus privacy --max-findings 10 src/api
dk review --template default --title "Add cache" --description PR_BODY.md --base-ref main --head-ref HEAD
dk review --template default --from-git main
dk review --template default -a codex -m o3 --output-file review.md
```

## Errors / exit

- `DK_INPUT_VALIDATION` — missing `--template`, invalid flag value.
- `DK_PACK_NOT_FOUND` — named pack not installed; run `dk install`.
- `DK_AGENT_NOT_FOUND` — agent binary missing.
- `DK_PIPELINE_ERROR` — agent failed / exhausted retries.
- `DK_SCORE_MISMATCH` — agent output internally inconsistent.
- `DK_IO_ERROR` — filesystem error.

## Gotchas

- `--template` is required — omitting it is an error.
- Needs an agent CLI on `PATH`. Default `claude`; verify with `dk doctor`.
- Output is buffered (no streaming) — a real review can take 1–2 min.
- `--focus` values are a closed enum; an unknown value is rejected.
