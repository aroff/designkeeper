# `dk-core` SDK

The domain crate behind `dk`. All review logic lives here; the `dk` binary is a
thin shell. Embed it to run reviews from Rust, inject a custom agent, or build
another front-end (`serve`, `mcp`).

```toml
[dependencies]
dk-core = { path = "crates/dk-core" }   # or git/version once published
```

## Modules (`crates/dk-core/src/lib.rs`)

| Module | Purpose |
|--------|---------|
| `config` | `dk.toml` resolution + defaults. |
| `discovery` | Default-target file discovery (`ignore` + `globset`). |
| `pack` | Embedded default template pack + path layout. |
| `slots` | Build prompt/report `{{slot}}` maps. |
| `pipeline` | `render → agent → extract+validate`, retry; traits + `SubprocessAgent`. |
| `review` | Review orchestration + typed output model. |
| `check` | Verdict → exit-code gate. |
| `init` | `.dk/` + `dk.toml` scaffolding. |
| `validation` | Post-validation warnings/errors on the typed output. |

## Entry points

```rust
// Real subprocess agent from config:
pub fn run_review(input: ReviewInput, config: &DkConfig, template_dir: &Path,
                  progress: &ProgressFn) -> Result<ReviewOutput, ReviewError>;
pub fn run_check(input: ReviewInput, config: &DkConfig, template_dir: &Path,
                 verbose: bool, progress: &ProgressFn) -> CheckResult;

// Inject your own agent (tests / embedding):
pub fn run_review_with_agent(input, config, template_dir, agent: &dyn AgentRunner,
                             progress: &ProgressFn) -> Result<ReviewOutput, ReviewError>;
pub fn run_check_with_agent(input, config, template_dir, verbose,
                            agent: &dyn AgentRunner, progress: &ProgressFn) -> CheckResult;

// Scaffolding:
pub fn run_init(working_dir: &Path, params: &InitParams) -> Result<InitOutcome, InitError>;
```

`ProgressFn = dyn Fn(Progress)`. Pass `&|_| {}` for none. See
[agent-invocation.md](agent-invocation.md) for `Progress`.

## Key types (re-exported at crate root)

- **Input**: `ReviewInput { working_dir, target, change_context, focus,
  project_hints, options }`, `ReviewOptions`, `ChangeContext`, `FocusArea`,
  `ProjectHints`.
- **Output**: `ReviewOutput`, `Summary`, `Verdict` (`Approve`,
  `ApproveWithComments`, `RequestChanges`, `Reject`; `Verdict::is_pass()`),
  `Dimension`, `GradeEntry`, `Finding`, `Severity` (`Blocker`/`Major`/`Minor`/`Nit`).
- **Config**: `DkConfig`, `ScanConfig`, `OutputConfig`, `OutputFormat`,
  `AgentConfig`, `TemplatesConfig`; `resolve_config(&Path)`, `default_config()`.
- **Check**: `CheckResult { exit_code, passed, report, findings_summary, fail_code }`.
- **Init**: `InitParams { agent, model, pack }`, `InitOutcome`, `PackSource`.
- **Pipeline**: `Pipeline`, `AgentRunner`, `PipelineError`, `validate_json`.
- **Validation**: `validate_output`, `ValidationWarning`.

Error enums (`ReviewError`, `PipelineError`, `ConfigError`, `InitError`) each
expose `.code() -> &'static str` returning the `DK_*` code the CLI prints.

## Inject a custom agent

```rust
use dk_core::{run_review_with_agent, ReviewInput, ReviewOptions};
use dk_core::config::default_config;
use dk_core::pipeline::{AgentRunner, PipelineError};
use std::path::Path;

struct CannedAgent(String);
impl AgentRunner for CannedAgent {
    fn run(&self, _prompt: &str, _wd: &Path) -> Result<String, PipelineError> {
        Ok(format!("```json\n{}\n```", self.0))   // must validate against the output schema
    }
}

let input = ReviewInput {
    working_dir: ".".into(), target: Some("src/".into()),
    change_context: None, focus: vec![], project_hints: None,
    options: ReviewOptions::default(),
};
let out = run_review_with_agent(input, &default_config(), template_dir,
                                &CannedAgent(json), &|_| {})?;
assert!(out.summary.verdict.is_pass());
```

`template_dir` must contain `templates/`, `schemas/`, `reports/`. Use
`pack::write_default_pack(dir)` to materialize the embedded default pack.

## The pipeline directly

```rust
use dk_core::pipeline::{Pipeline, DefaultRenderer, JsonResponseValidator};
let pipe = Pipeline::new(&DefaultRenderer, &agent, &JsonResponseValidator); // max_retries = 2
let value = pipe.run(&prompt_template, &slots, working_dir, &output_schema, &|_| {})?;
```

`Pipeline::run` emits `Progress` events and retries on validation failure,
appending the errors to the prompt each attempt.
