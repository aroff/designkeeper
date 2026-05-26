# `dk check`

Run a review and collapse the verdict to a **process exit code** — the
CI / pre-commit gate.

Handler: `crates/dk/src/main.rs` (`run_check_cmd`) →
`dk_core::run_check` (`crates/dk-core/src/check.rs`).

## Synopsis

```
dk check [<path>] [-a --agent <a>] [-m --model <m>]
         [--output-format markdown|json] [--output-file <path>] [-v --verbose]
```

Shares the review flags (agent/model/output/path); see
[cli-review.md](cli-review.md). Adds:

| Flag | Meaning |
|------|---------|
| `-v, --verbose` | Also print the full scored report to stdout. |

## Behavior

Runs the same pipeline as `review`, then maps the verdict:

| Verdict | Exit |
|---------|------|
| `approve`, `approve_with_comments` | `0` (pass) |
| `request_changes`, `reject` | `1` (fail) |
| pipeline error (agent missing, invalid output, …) | `1` |

`Verdict::is_pass()` (`crates/dk-core/src/review.rs`) defines the pass set.

## Output

- **Pass**: nothing on stdout, exit 0.
- **Fail**: a findings summary on **stderr** — verdict + score, then findings
  grouped by severity (blockers first), each as `id: observation (location)`
  (`findings_summary`, `check.rs`). `fail_code` is `DK_CHECK_FAILED` for a
  failing verdict, or the underlying review error code.
- `-v/--verbose`: the full markdown report is also written to stdout (or
  `--output-file`).

The `CheckResult` struct (`exit_code`, `passed`, `report`, `findings_summary`,
`fail_code`) is what the SDK returns; the CLI maps it to stdout/stderr + exit.

## Examples

```sh
dk check && echo "design OK"          # gate in a shell
dk check || exit 1                    # in a script
dk check crates/dk -v                 # print the report too
dk check -a claude -m sonnet          # override agent/model
```

CI usage: a non-zero exit fails the step. Pair with `dk doctor` to fail fast
when the agent isn't reachable.

## Gotchas

- Quiet on success by design — don't expect stdout when it passes.
- Same agent/pipeline prerequisites as `review` (agent on `PATH`, slow,
  buffered).
- `check` is intentionally **not** exposed over MCP (its whole output is the
  exit code).
