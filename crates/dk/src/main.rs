//! DesignKeeper CLI (`dk`) — a thin `cli-framework` shell over `dk-core`.
//!
//! Registers the `review` and `check` commands, maps flags onto
//! [`dk_core::ReviewInput`], and routes output. All domain logic lives in
//! `dk-core`; this crate only parses arguments and formats I/O.

mod doctor;
mod input;
mod progress;

use std::sync::Arc;

use cli_framework::doctor::DoctorModule;
use cli_framework::mcp::McpToolExportPolicy;
use cli_framework::prelude::*;
use cli_framework::spec::arg_spec::{ArgKind, ArgValueType, Cardinality};

use dk_core::config::{default_config, resolve_config, OutputFormat};
use dk_core::{pack_store, review, run_check};
use dk_core::{run_init, InitParams};
use dk_core::PackScope;

use input::{
    current_dir, emit, flag, output_format, prompt_or_default, resolve_common_args,
};
use progress::{fail, ProgressReporter};

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
        // Only commands flagged `expose_mcp` are surfaced as MCP tools by
        // the auto-registered `mcp serve`. Keeps `init`/`doctor` CLI-only.
        .with_mcp_export_policy(McpToolExportPolicy::ExposeMcpOnly)
        .register_command(review_command())?
        .register_command(check_command())?
        .register_command(init_command())?
        .register_command(install_command())?
        .register_module(DoctorModule::new(doctor::checks()))?
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

fn flag_spec(name: &'static str, short: Option<char>, help: &'static str) -> ArgSpec {
    ArgSpec {
        name,
        kind: ArgKind::Flag,
        short,
        long: None,
        value_type: ArgValueType::Bool,
        cardinality: Cardinality::Optional,
        default: None,
        conflicts_with: vec![],
        requires: vec![],
        help,
    }
}

fn int_opt(name: &'static str, help: &'static str) -> ArgSpec {
    ArgSpec {
        name,
        kind: ArgKind::Option,
        short: None,
        long: None,
        value_type: ArgValueType::Int,
        cardinality: Cardinality::Optional,
        default: None,
        conflicts_with: vec![],
        requires: vec![],
        help,
    }
}

fn positional_spec(name: &'static str, help: &'static str) -> ArgSpec {
    ArgSpec {
        name,
        kind: ArgKind::Positional,
        short: None,
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
        opt(
            "from-git",
            None,
            "Derive PR context from git using <base-ref> as the merge base",
        ),
        opt(
            "include-dimensions",
            None,
            "Comma-separated dimensions to grade (all others → not_evaluated)",
        ),
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
        int_opt("max-findings", "Maximum findings to emit (1-50, default 25)"),
        positional_spec("path", "Path/glob root within the repo to focus the review"),
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
        expose_mcp: true,
        execute: Arc::new(|_ctx, args| Box::pin(async move { run_review_cmd(args) })),
    }
}

fn check_command() -> Command {
    let mut args = common_args();
    args.push(flag_spec("verbose", Some('v'), "Print the full scored report to stdout"));
    args.push(opt(
        "from-git",
        None,
        "Derive PR context from git using <base-ref> as the merge base",
    ));
    args.push(positional_spec("path", "Path/glob root within the repo to focus the review"));
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

fn init_command() -> Command {
    let args = vec![
        opt("agent", Some('a'), "Default agent key (e.g. claude, codex)"),
        opt("model", Some('m'), "Default model override (optional)"),
    ];
    let spec = CommandSpec {
        summary: "Install packs from dk-templates.toml and write dk.toml",
        args,
        ..Default::default()
    };
    Command {
        id: "init",
        summary: "Install template packs and scaffold dk.toml",
        syntax: Some("init [--agent <a>] [--model <m>]"),
        category: Some("setup"),
        spec: Some(Arc::new(spec)),
        validator: None,
        expose_mcp: false,
        execute: Arc::new(|_ctx, args| Box::pin(async move { run_init_cmd(args) })),
    }
}

fn install_command() -> Command {
    let args = vec![
        flag_spec("global", Some('g'), "Install packs to ~/.dk/packs/ (user-global) instead of .dk/packs/"),
        positional_spec("source", "Pack source: owner/repo, URL, or local path. Omit to install all official packs."),
    ];
    let spec = CommandSpec {
        summary: "Install template packs from GitHub, a URL, or a local path",
        args,
        ..Default::default()
    };
    Command {
        id: "install",
        summary: "Install template packs",
        syntax: Some("install [--global] [<source>]"),
        category: Some("setup"),
        spec: Some(Arc::new(spec)),
        validator: None,
        expose_mcp: false,
        execute: Arc::new(|_ctx, args| Box::pin(async move { run_install_cmd(args) })),
    }
}

/// Flags shared by `review` and `check`.
fn common_args() -> Vec<ArgSpec> {
    vec![
        opt("template", Some('t'), "Template pack name (required, e.g. default, structural)"),
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
        int_opt("timeout", "Agent timeout in seconds (0 = no timeout)"),
        int_opt("max-retries", "Max retry attempts after first failure (default 2)"),
    ]
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

fn print_installed(p: &dk_core::InstalledPack) {
    println!("✓ installed {} → {}", p.name, p.path.display());
}

fn run_review_cmd(args: CommandArgs) -> anyhow::Result<()> {
    let c = resolve_common_args(&args);

    let reporter = ProgressReporter::new(&c.config.agent.agent);
    let runner = review::build_agent_runner(&c.config, &c.input);
    let result = review::run_review(c.input, &c.config, &c.template_dir, runner, &|e| reporter.handle(e));
    reporter.finish();
    let output = match result {
        Ok(o) => o,
        Err(e) => fail(e.code(), &e.to_string()),
    };

    let format = output_format(&args, &c.config);
    let rendered = match format {
        OutputFormat::Json => serde_json::to_string_pretty(&output)
            .unwrap_or_else(|e| fail("DK_IO_ERROR", &e.to_string())),
        OutputFormat::Markdown => match review::render_report(&output, &c.template_dir) {
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
    let c = resolve_common_args(&args);
    let verbose = flag(&args, "verbose");

    let reporter = ProgressReporter::new(&c.config.agent.agent);
    let runner = review::build_agent_runner(&c.config, &c.input);
    let result = run_check(c.input, &c.config, &c.template_dir, verbose, runner, &|e| {
        reporter.handle(e)
    });
    reporter.finish();
    if let Some(report) = &result.report {
        if let Err(e) = emit(&args, report) {
            fail("DK_IO_ERROR", &e.to_string());
        }
    }
    if let Some(summary) = &result.findings_summary {
        eprintln!("{summary}");
    }
    std::process::exit(result.exit_code());
}

fn run_init_cmd(args: CommandArgs) -> anyhow::Result<()> {
    let cwd = current_dir();
    let existing = resolve_config(&cwd).unwrap_or_else(|_| default_config());

    let agent = prompt_or_default(args.named.get("agent"), "Agent", &existing.agent.agent);
    let model_default = existing.agent.model.as_deref().unwrap_or("");
    let model_raw = prompt_or_default(
        args.named.get("model"),
        "Model (blank for none)",
        model_default,
    );
    let model = Some(model_raw).filter(|m| !m.trim().is_empty());

    let params = InitParams { agent, model };
    let outcome = match run_init(&cwd, &params) {
        Ok(o) => o,
        Err(e) => fail(e.code(), &e.to_string()),
    };

    let verb = if outcome.updated_existing { "Updated" } else { "Created" };
    println!("{verb} {}", outcome.config_path.display());
    if outcome.installed_packs.is_empty() {
        println!("No packs installed (check dk-templates.toml sources).");
    } else {
        for p in &outcome.installed_packs {
            print_installed(p);
        }
    }
    Ok(())
}

fn run_install_cmd(args: CommandArgs) -> anyhow::Result<()> {
    let cwd = current_dir();
    let global = flag(&args, "global");
    let scope = if global { PackScope::Global } else { PackScope::Project };

    let dest_base = if global {
        pack_store::global_packs_dir()
            .unwrap_or_else(|| fail("DK_IO_ERROR", "cannot determine home directory for --global install"))
    } else {
        cwd.join(".dk").join("packs")
    };

    if let Some(source) = args.named.get("source") {
        match pack_store::install_pack(source, &dest_base, scope) {
            Ok(p) => print_installed(&p),
            Err(e) => fail(e.code(), &e.to_string()),
        }
    } else {
        let manifest = dk_core::DkTemplatesManifest::resolve(&cwd);
        let mut any = false;
        for entry in &manifest.packs {
            match pack_store::install_pack_or_embedded_fallback(entry, &dest_base, scope.clone()) {
                Ok(Some(p)) => {
                    print_installed(&p);
                    any = true;
                }
                Ok(None) => {
                    eprintln!("  ! skipped {} (no source or fallback)", entry.name);
                }
                Err(e) => {
                    fail(e.code(), &e.to_string());
                }
            }
        }
        if !any {
            eprintln!("No packs could be installed. Check dk-templates.toml sources.");
        }
    }
    Ok(())
}