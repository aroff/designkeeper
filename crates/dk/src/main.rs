//! DesignKeeper CLI (`dk`) — a thin `cli-framework` shell over `dk-core`.
//!
//! Registers the `review` and `check` commands, maps flags onto
//! [`dk_core::ReviewInput`], and routes output. All domain logic lives in
//! `dk-core`; this crate only parses arguments and formats I/O.

use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::Arc;

use cli_framework::prelude::*;
use cli_framework::spec::arg_spec::{ArgKind, ArgValueType, Cardinality};

use dk_core::config::{resolve_config, DkConfig, OutputFormat};
use dk_core::{pack, review, run_check, ReviewInput, ReviewOptions};
use dk_core::{ChangeContext, FocusArea};

struct DkContext;
impl AppContext for DkContext {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let app = AppBuilder::new()
        .with_version("dk", env!("CARGO_PKG_VERSION"))
        .register_command(review_command())?
        .register_command(check_command())?
        .build(DkContext)?;
    let mut app = app;
    app.run().await
}

// ---------------------------------------------------------------------------
// Command specs
// ---------------------------------------------------------------------------

fn opt(name: &'static str, short: Option<char>, help: &'static str) -> ArgSpec {
    ArgSpec {
        name,
        kind: ArgKind::Option,
        short,
        long: None,
        value_type: ArgValueType::String,
        cardinality: Cardinality::Optional,
        default: None,
        conflicts_with: vec![],
        requires: vec![],
        help,
    }
}

fn review_command() -> Command {
    let mut args = common_args();
    args.extend([
        opt("title", None, "PR/CL title"),
        opt(
            "description",
            None,
            "PR/CL description (file path or raw text)",
        ),
        opt("base-ref", None, "Base git ref, e.g. main"),
        opt("head-ref", None, "Head git ref, e.g. HEAD"),
        ArgSpec {
            name: "focus",
            kind: ArgKind::Option,
            short: None,
            long: None,
            value_type: ArgValueType::Enum(vec![
                "security",
                "concurrency",
                "accessibility",
                "internationalization",
                "privacy",
                "performance",
                "api_design",
                "ui",
            ]),
            cardinality: Cardinality::Repeated,
            default: None,
            conflicts_with: vec![],
            requires: vec![],
            help: "Focus area (repeatable)",
        },
        ArgSpec {
            name: "max-findings",
            kind: ArgKind::Option,
            short: None,
            long: None,
            value_type: ArgValueType::Int,
            cardinality: Cardinality::Optional,
            default: None,
            conflicts_with: vec![],
            requires: vec![],
            help: "Maximum findings to emit (1-50, default 25)",
        },
        positional_path(),
    ]);
    let spec = CommandSpec {
        summary: "Structured, agent-driven code review",
        args,
        ..Default::default()
    };
    Command {
        id: "review",
        summary: "Structured, agent-driven code review",
        syntax: Some("review [<path>] [--agent <a>] [--focus <area>]... [--output-format json]"),
        category: Some("analysis"),
        spec: Some(Arc::new(spec)),
        validator: None,
        expose_mcp: false,
        execute: Arc::new(|_ctx, args| Box::pin(async move { run_review_cmd(args) })),
    }
}

fn check_command() -> Command {
    let mut args = common_args();
    args.push(ArgSpec {
        name: "verbose",
        kind: ArgKind::Flag,
        short: Some('v'),
        long: None,
        value_type: ArgValueType::Bool,
        cardinality: Cardinality::Optional,
        default: None,
        conflicts_with: vec![],
        requires: vec![],
        help: "Print the full scored report to stdout",
    });
    args.push(positional_path());
    let spec = CommandSpec {
        summary: "Pass/fail review gate (verdict -> exit code)",
        args,
        ..Default::default()
    };
    Command {
        id: "check",
        summary: "Pass/fail review gate (verdict -> exit code)",
        syntax: Some("check [<path>] [--agent <a>] [--verbose]"),
        category: Some("analysis"),
        spec: Some(Arc::new(spec)),
        validator: None,
        expose_mcp: false,
        execute: Arc::new(|_ctx, args| Box::pin(async move { run_check_cmd(args) })),
    }
}

/// Flags shared by `review` and `check`.
fn common_args() -> Vec<ArgSpec> {
    vec![
        opt("agent", Some('a'), "Agent key (overrides dk.toml)"),
        opt("model", Some('m'), "Model override (overrides dk.toml)"),
        opt(
            "output-format",
            None,
            "Output format: markdown (default) or json",
        ),
        opt(
            "output-file",
            None,
            "Write output to this file instead of stdout",
        ),
    ]
}

fn positional_path() -> ArgSpec {
    ArgSpec {
        name: "path",
        kind: ArgKind::Positional,
        short: None,
        long: None,
        value_type: ArgValueType::String,
        cardinality: Cardinality::Optional,
        default: None,
        conflicts_with: vec![],
        requires: vec![],
        help: "Path/glob root within the repo to focus the review",
    }
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

fn run_review_cmd(args: CommandArgs) -> anyhow::Result<()> {
    let cwd = current_dir();
    let config = match resolved_config(&args, &cwd) {
        Ok(c) => c,
        Err(msg) => fail("DK_CONFIG_PARSE", &msg),
    };
    let input = match map_input(&args, &cwd) {
        Ok(i) => i,
        Err(msg) => fail("DK_INPUT_VALIDATION", &msg),
    };
    let template_dir = match ensure_template_dir(&cwd) {
        Ok(d) => d,
        Err(e) => fail("DK_IO_ERROR", &e.to_string()),
    };

    let output = match review::run_review(input, &config, &template_dir) {
        Ok(o) => o,
        Err(e) => fail(e.code(), &e.to_string()),
    };

    let format = output_format(&args, &config);
    let rendered = match format {
        OutputFormat::Json => serde_json::to_string_pretty(&output)
            .unwrap_or_else(|e| fail("DK_IO_ERROR", &e.to_string())),
        OutputFormat::Markdown => match review::render_report(&output, &template_dir) {
            Ok(r) => r,
            Err(e) => fail(e.code(), &e.to_string()),
        },
    };

    if let Err(e) = emit(&args, &rendered) {
        fail("DK_IO_ERROR", &e.to_string());
    }
    Ok(())
}

fn run_check_cmd(args: CommandArgs) -> anyhow::Result<()> {
    let cwd = current_dir();
    let config = match resolved_config(&args, &cwd) {
        Ok(c) => c,
        Err(msg) => fail("DK_CONFIG_PARSE", &msg),
    };
    let input = match map_input(&args, &cwd) {
        Ok(i) => i,
        Err(msg) => fail("DK_INPUT_VALIDATION", &msg),
    };
    let template_dir = match ensure_template_dir(&cwd) {
        Ok(d) => d,
        Err(e) => fail("DK_IO_ERROR", &e.to_string()),
    };
    let verbose = flag(&args, "verbose");

    let result = run_check(input, &config, &template_dir, verbose);
    if let Some(report) = &result.report {
        if let Err(e) = emit(&args, report) {
            fail("DK_IO_ERROR", &e.to_string());
        }
    }
    if let Some(summary) = &result.findings_summary {
        eprintln!("{summary}");
    }
    exit(if result.passed { 0 } else { 1 });
}

// ---------------------------------------------------------------------------
// Flag -> input mapping and helpers
// ---------------------------------------------------------------------------

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolve config from `dk.toml`, then apply CLI agent/model overrides
/// (CLI > dk.toml > built-in defaults).
fn resolved_config(args: &CommandArgs, cwd: &Path) -> Result<DkConfig, String> {
    let mut config = resolve_config(cwd).map_err(|e| e.to_string())?;
    if let Some(agent) = args.named.get("agent") {
        config.agent.agent = agent.clone();
    }
    if let Some(model) = args.named.get("model") {
        config.agent.model = Some(model.clone());
    }
    Ok(config)
}

fn output_format(args: &CommandArgs, config: &DkConfig) -> OutputFormat {
    match args.named.get("output-format") {
        Some(s) => OutputFormat::parse(s).unwrap_or_else(|| {
            fail(
                "DK_INPUT_VALIDATION",
                &format!("invalid --output-format: {s}"),
            )
        }),
        None => config.output.format,
    }
}

fn map_input(args: &CommandArgs, cwd: &Path) -> Result<ReviewInput, String> {
    let target = args.named.get("path").cloned();

    let title = args.named.get("title").cloned();
    let description = args.named.get("description").map(|d| read_file_or_text(d));
    let base_ref = args.named.get("base-ref").cloned();
    let head_ref = args.named.get("head-ref").cloned();
    let change_context =
        if title.is_some() || description.is_some() || base_ref.is_some() || head_ref.is_some() {
            Some(ChangeContext {
                title,
                description,
                base_ref,
                head_ref,
                diff_stat: None,
            })
        } else {
            None
        };

    let focus = match args.named.get("focus") {
        Some(s) => s
            .split(',')
            .filter(|x| !x.is_empty())
            .map(|x| FocusArea::parse(x).ok_or_else(|| format!("invalid --focus value: {x}")))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };

    let max_findings = match args.named.get("max-findings") {
        Some(s) => {
            let n: u8 = s
                .parse()
                .map_err(|_| format!("invalid --max-findings: {s}"))?;
            if !(1..=50).contains(&n) {
                return Err(format!("--max-findings must be 1-50, got {n}"));
            }
            n
        }
        None => 25,
    };

    Ok(ReviewInput {
        working_dir: cwd.to_string_lossy().into_owned(),
        target,
        change_context,
        focus,
        project_hints: None,
        options: ReviewOptions {
            max_findings,
            include_dimensions: None,
        },
    })
}

/// `--description` accepts a file path (if it exists) or raw text (AC #19).
fn read_file_or_text(value: &str) -> String {
    let path = Path::new(value);
    if path.is_file() {
        if let Ok(contents) = std::fs::read_to_string(path) {
            return contents;
        }
    }
    value.to_string()
}

fn flag(args: &CommandArgs, name: &str) -> bool {
    args.named.get(name).map(|v| v == "true").unwrap_or(false)
}

/// Use `.dk/` if present (walking up from cwd); otherwise materialize the
/// embedded default template pack to a temp dir (spec decision #6).
fn ensure_template_dir(cwd: &Path) -> Result<PathBuf, std::io::Error> {
    let mut dir = Some(cwd);
    while let Some(current) = dir {
        let dk = current.join(".dk");
        if pack::prompt_path(&dk).is_file() {
            return Ok(dk);
        }
        dir = current.parent();
    }
    let base = std::env::temp_dir().join(format!("dk-pack-{}", std::process::id()));
    pack::write_default_pack(&base)?;
    Ok(base)
}

/// Write `content` to `--output-file` if set, otherwise to stdout.
fn emit(args: &CommandArgs, content: &str) -> Result<(), std::io::Error> {
    match args.named.get("output-file") {
        Some(path) => std::fs::write(path, content),
        None => {
            println!("{content}");
            Ok(())
        }
    }
}

/// Print `error [CODE]: message` to stderr and exit with status 1.
fn fail(code: &str, message: &str) -> ! {
    eprintln!("error [{code}]: {message}");
    exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(named: &[(&str, &str)], positional: &[&str]) -> CommandArgs {
        CommandArgs {
            positional: positional.iter().map(|s| s.to_string()).collect(),
            named: named
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn description_reads_file_when_path_exists() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("body.md");
        std::fs::write(&file, "from file").unwrap();
        assert_eq!(read_file_or_text(file.to_str().unwrap()), "from file");
    }

    #[test]
    fn description_uses_raw_text_when_not_a_file() {
        assert_eq!(read_file_or_text("just some text"), "just some text");
    }

    #[test]
    fn map_input_parses_flags() {
        let a = args(
            &[
                ("title", "T"),
                ("description", "raw body"),
                ("base-ref", "main"),
                ("head-ref", "HEAD"),
                ("focus", "security,concurrency"),
                ("max-findings", "10"),
            ],
            &[],
        );
        let cwd = std::env::temp_dir();
        let input = map_input(&a, &cwd).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("T"));
        assert_eq!(cc.description.as_deref(), Some("raw body"));
        assert_eq!(input.focus.len(), 2);
        assert_eq!(input.options.max_findings, 10);
    }

    #[test]
    fn map_input_rejects_bad_focus_and_range() {
        let cwd = std::env::temp_dir();
        assert!(map_input(&args(&[("focus", "nope")], &[]), &cwd).is_err());
        assert!(map_input(&args(&[("max-findings", "99")], &[]), &cwd).is_err());
    }

    #[test]
    fn agent_model_precedence_cli_over_config() {
        // No dk.toml in a fresh dir -> defaults (agent="claude", model=None).
        let dir = tempfile::tempdir().unwrap();
        let a = args(&[("agent", "codex"), ("model", "gpt-5")], &[]);
        let cfg = resolved_config(&a, dir.path()).unwrap();
        assert_eq!(cfg.agent.agent, "codex");
        assert_eq!(cfg.agent.model.as_deref(), Some("gpt-5"));

        // Absent CLI flags -> built-in defaults.
        let cfg2 = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        assert_eq!(cfg2.agent.agent, "claude");
        assert_eq!(cfg2.agent.model, None);
    }

    #[test]
    fn output_format_defaults_to_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        assert_eq!(output_format(&args(&[], &[]), &cfg), OutputFormat::Markdown);
        assert_eq!(
            output_format(&args(&[("output-format", "json")], &[]), &cfg),
            OutputFormat::Json
        );
    }

    #[test]
    fn flag_detection() {
        assert!(flag(&args(&[("verbose", "true")], &[]), "verbose"));
        assert!(!flag(&args(&[], &[]), "verbose"));
    }
}
