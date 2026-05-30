# `dk check`

Run a review and collapse the verdict to a **process exit code** — the
CI / pre-commit gate.

## Synopsis

```
dk check --template <name> [<path>]
         [-a --agent <a>] [-m --model <m>]
         [--output-format markdown|json] [--output-file <path>]
         [--from-git <base-ref>]
         [-v --verbose]
```

Shares the review flags (agent/model/output/path/from-git/template); see
[cli-review.md](cli-review.md). Adds:

| Flag | Meaning |
|------|---------|
| `-t, --template <name>` | **Required.** Template pack to use (e.g. `default`, `structural`). |
| `-v, --verbose` | Also print the full scored report to stdout. |

## Behavior

Runs the same pipeline as `review`, then maps the verdict:

| Verdict | Exit |
|---------|------|
| `approve`, `approve_with_comments` | `0` (pass) |
| `request_changes`, `reject` | `1` (fail) |
| pipeline error (agent missing, invalid output, …) | `2` |

## Output

- **Pass**: nothing on stdout, exit 0.
- **Fail**: a findings summary on **stderr** — verdict + score, then findings
  grouped by severity (blockers/criticals first), each as `id: observation (location)`.
- `-v/--verbose`: the full markdown report is also written to stdout (or
  `--output-file`).

## Examples

```sh
dk check --template default && echo "design OK"       # gate in a shell
dk check --template structural src/ -v                 # structural check + full report
dk check --template default -a claude -m sonnet        # override agent/model
dk check --template default --from-git main            # PR context from git
```

CI usage:
```yaml
- run: dk check --template default
```

## Gotchas

- `--template` is required — omitting it is an error.
- Quiet on success by design — don't expect stdout when it passes.
- Same agent/pipeline prerequisites as `review` (agent on `PATH`, slow, buffered).
- `check` is intentionally **not** exposed over MCP (its whole output is the exit code).
