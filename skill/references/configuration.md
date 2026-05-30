# Configuration & template packs

Two optional, user-editable inputs; both default sensibly when absent. `dk`
walks up parent directories to find each.

## `dk.toml` (control file)

`dk` walks up from the working dir to the first `dk.toml`; if none is found it
uses built-in defaults. Unknown keys are a hard error (`DK_CONFIG_PARSE`).

```toml
[scan]
extensions = [".rs", ".ts"]          # file types auto-discovery considers
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"                  # or "json"

[agent]
agent = "claude"                     # default agent key
model = ""                           # optional model override ("" = unset)
```

All sections/fields are optional and merged over defaults. An empty `model =
""` is treated as unset. The `[templates]` section has been removed — template
pack selection is now per-command via `--template`.

**Defaults**: `extensions` = ~19 languages (`.rs .ts .tsx .js .jsx .py .go
.java .kt .c .cpp .h .hpp .rb .ex .exs .scala .swift .cs`), `ignore_patterns`
empty, `format` markdown, `agent` `claude`, `model` none.

**Precedence**: CLI flags (`--agent`/`--model`/`--output-format`) > `dk.toml` >
built-in defaults.

## Template packs

Each pack is a self-contained directory with a fixed layout. Packs are
installed by `dk init` or `dk install`.

### Pack directory layout

```
{pack-root}/
├── aikit.toml           # pack manifest ([package] name, version, description)
├── templates/
│   ├── review.md        # prompt template with {{slots}}
│   └── methodology.md   # the review rubric — edit to customize
├── reports/
│   └── review.md        # report layout
└── schemas/
    ├── review-input.json
    └── review.json      # output schema the agent must satisfy
```

### Pack resolution order

When `dk review --template <name>` is invoked:

1. Walk up from CWD looking for `.dk/packs/{name}/templates/review.md` →
   project-local pack.
2. Check `~/.dk/packs/{name}/templates/review.md` → user-global pack.
3. For `default` and `structural`: write the embedded copy to a temp dir →
   embedded fallback (no install required).
4. `DK_PACK_NOT_FOUND` for any other name.

Project-local always wins over global.

### Built-in packs

| Pack | Rubric | Dimensions |
|------|--------|-----------|
| `default` | Google Engineering Practices | 13: overall_code_health, cl_description, change_scope, design, functionality, complexity, tests, naming, comments, style, consistency, documentation, context_and_review_depth |
| `structural` | Structure · Complexity · Expressiveness | 9: file_decomposition, layer_integrity, helper_reuse, structural_simplicity, branching_complexity, orchestration_quality, abstraction_quality, type_contract_clarity, legibility |

### Customizing a pack

After installing, edit `.dk/packs/{name}/templates/methodology.md` to tune the
rubric for your team. The source templates live in [`templates/`](../../../templates/).

### Score thresholds (structural pack)

The structural pack uses different severity labels and score anchors than the
default pack:

| Score | Severity | Meaning |
|-------|----------|---------|
| 9–10 | — | Good |
| 8 | low | Minor concern |
| 6–7 | medium | Approve with comments |
| 4–5 | high | Request changes |
| 0–3 | **critical** | Blocks merge; −0.5 penalty per dimension to overall score |

### `dk-templates.toml`

The official pack manifest listing pack sources. `dk install` (with no
arguments) and `dk init` read this. `dk` walks up from CWD for a
`dk-templates.toml`; falls back to the embedded copy bundled in the binary.

```toml
[[packs]]
name = "default"
description = "Google engineering practices code review rubric"
source = "owner/dk-template-default"   # GitHub shorthand

[[packs]]
name = "structural"
description = "Structural code quality review — Structure, Complexity, Expressiveness"
source = "owner/dk-template-structural"
```

## Prompt slot assembly

`dk` fills the template's `{{slots}}`:

| Slot | Source |
|------|--------|
| `{{working_dir}}` | Canonicalized working dir. |
| `{{target}}` | The `<path>` arg, or newline-joined discovered files, or `"entire repository"`. |
| `{{change_context}}` | `--title/--description/--base-ref/--head-ref`, or "No PR/CL metadata supplied." |
| `{{focus}}` | `--focus` areas. |
| `{{project_hints}}` | Optional hints. |
| `{{methodology}}` | `templates/methodology.md` from the resolved pack. |
| `{{max_findings}}` | `--max-findings` (default 25). |
| `{{output_schema}}` | Minified `schemas/review.json` from the resolved pack. |
| `{{dimensions_filter}}` | Instruction derived from `--include-dimensions`. |

The prompt is a **task directive** — methodology + target + schema — not a
context payload. The agent reads source files from disk itself.

## File discovery

Used only when `dk review` is run without a `<path>`. `dk` walks the working
dir — honoring `.gitignore` even outside a git repo, and skipping hidden
files/dirs — and keeps files that:

1. match an extension in `[scan].extensions`,
2. are not gitignored,
3. are not under a `[scan].ignore_patterns` entry (a trailing `/` like
   `vendor/` matches everything under it).

Returns repo-relative, forward-slashed, sorted paths.
