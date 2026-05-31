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
use dk_core::PackScope;
use dk_core::{pack_store, review, run_check};
use dk_core::{run_init, InitParams};

use input::{
    current_dir, effective_sarif_path, emit, flag, prompt_or_default, resolve_common_args,
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
        int_opt(
            "max-findings",
            "Maximum findings to emit (1-50, default 25)",
        ),
        opt(
            "sarif",
            None,
            "Also write a SARIF 2.1.0 report to this file",
        ),
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
    args.push(flag_spec(
        "verbose",
        Some('v'),
        "Print the full scored report to stdout",
    ));
    args.push(opt(
        "from-git",
        None,
        "Derive PR context from git using <base-ref> as the merge base",
    ));
    args.push(positional_spec(
        "path",
        "Path/glob root within the repo to focus the review",
    ));
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
        execute: Arc::new(|_ctx, args| {
            Box::pin(async move { tokio::task::block_in_place(|| run_init_cmd(args)) })
        }),
    }
}

fn install_command() -> Command {
    let args = vec![
        flag_spec(
            "global",
            Some('g'),
            "Install packs to ~/.dk/packs/ (user-global) instead of .dk/packs/",
        ),
        positional_spec(
            "source",
            "Pack source: owner/repo, URL, or local path. Omit to install all official packs.",
        ),
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
        execute: Arc::new(|_ctx, args| {
            Box::pin(async move { tokio::task::block_in_place(|| run_install_cmd(args)) })
        }),
    }
}

/// Flags shared by `review` and `check`.
fn common_args() -> Vec<ArgSpec> {
    vec![
        opt(
            "template",
            Some('t'),
            "Template pack name (required, e.g. default, structural)",
        ),
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
        int_opt(
            "max-retries",
            "Max retry attempts after first failure (default 2)",
        ),
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
    let result = review::run_review(c.input, &c.config, &c.template_dir, runner, &|e| {
        reporter.handle(e)
    });
    reporter.finish();
    let output = match result {
        Ok(o) => o,
        Err(e) => fail(e.code(), &e.to_string()),
    };

    let format = c.output_format;

    // Build SARIF before the primary emit so we can reuse output.
    let sarif_path = effective_sarif_path(&args, &c.config);
    let sarif_rendered: Option<String> =
        if sarif_path.is_some() || matches!(format, OutputFormat::Sarif) {
            let meta = dk_core::SarifRunMeta {
                tool_name: "dk".into(),
                tool_version: env!("CARGO_PKG_VERSION").into(),
                agent_key: c.config.agent.agent.clone(),
                model: c.config.agent.model.clone(),
            };
            let sarif = dk_core::sarif::to_sarif(&output, &meta);
            Some(
                serde_json::to_string_pretty(&sarif)
                    .unwrap_or_else(|e| fail("DK_IO_ERROR", &e.to_string())),
            )
        } else {
            None
        };

    let rendered = match format {
        OutputFormat::Json => serde_json::to_string_pretty(&output)
            .unwrap_or_else(|e| fail("DK_IO_ERROR", &e.to_string())),
        OutputFormat::Markdown => match review::render_report(&output, &c.template_dir) {
            Ok(r) => r,
            Err(e) => fail(e.code(), &e.to_string()),
        },
        OutputFormat::Sarif => sarif_rendered
            .clone()
            .unwrap_or_else(|| fail("DK_IO_ERROR", "SARIF render failed")),
    };

    if let Err(e) = emit(&args, &rendered) {
        fail("DK_IO_ERROR", &e.to_string());
    }

    // Write SARIF side-channel if requested.
    if let Some(path) = sarif_path {
        let content =
            sarif_rendered.unwrap_or_else(|| fail("DK_IO_ERROR", "SARIF render unavailable"));
        if let Err(e) = std::fs::write(&path, &content) {
            fail("DK_IO_ERROR", &e.to_string());
        }
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

    let verb = if outcome.updated_existing {
        "Updated"
    } else {
        "Created"
    };
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
    let scope = if global {
        PackScope::Global
    } else {
        PackScope::Project
    };

    let dest_base = if global {
        pack_store::global_packs_dir().unwrap_or_else(|| {
            fail(
                "DK_IO_ERROR",
                "cannot determine home directory for --global install",
            )
        })
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
        for entry in &manifest.packs {
            match pack_store::install_pack(&entry.source, &dest_base, scope.clone()) {
                Ok(p) => print_installed(&p),
                Err(e) => fail(e.code(), &e.to_string()),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_names(cmd: &Command) -> Vec<&'static str> {
        cmd.spec
            .as_ref()
            .expect("command has a spec")
            .args
            .iter()
            .map(|a| a.name)
            .collect()
    }

    #[test]
    fn review_is_mcp_exposed_with_expected_args() {
        let cmd = review_command();
        assert_eq!(cmd.id, "review");
        assert!(cmd.expose_mcp, "review must be surfaced as an MCP tool");
        assert_eq!(cmd.category, Some("analysis"));
        assert!(
            cmd.syntax.is_some(),
            "review should advertise a syntax line"
        );
        let names = arg_names(&cmd);
        for expected in [
            "template",
            "focus",
            "sarif",
            "max-findings",
            "include-dimensions",
            "path",
        ] {
            assert!(
                names.contains(&expected),
                "review missing {expected}: {names:?}"
            );
        }
    }

    #[test]
    fn review_focus_enumerates_every_focus_area() {
        let cmd = review_command();
        let spec = cmd.spec.as_ref().unwrap();
        let focus = spec
            .args
            .iter()
            .find(|a| a.name == "focus")
            .expect("review has a --focus arg");
        match &focus.value_type {
            ArgValueType::Enum(values) => {
                for v in [
                    "security",
                    "concurrency",
                    "accessibility",
                    "internationalization",
                    "privacy",
                    "performance",
                    "api_design",
                    "ui",
                ] {
                    assert!(values.contains(&v), "focus enum missing {v}");
                }
            }
            _ => panic!("--focus should be an Enum value type"),
        }
        assert!(
            matches!(focus.cardinality, Cardinality::Repeated),
            "--focus must be repeatable"
        );
    }

    #[test]
    fn check_is_not_mcp_exposed_and_has_verbose() {
        let cmd = check_command();
        assert_eq!(cmd.id, "check");
        assert!(!cmd.expose_mcp, "check must not become an MCP tool");
        assert!(arg_names(&cmd).contains(&"verbose"));
    }

    #[test]
    fn init_and_install_are_unexposed_setup_commands() {
        let init = init_command();
        assert_eq!(init.id, "init");
        assert!(!init.expose_mcp);
        assert_eq!(init.category, Some("setup"));

        let install = install_command();
        assert_eq!(install.id, "install");
        assert!(!install.expose_mcp);
        let names = arg_names(&install);
        assert!(names.contains(&"global"), "install should have --global");
        assert!(
            names.contains(&"source"),
            "install should have a source positional"
        );
    }

    #[test]
    fn common_args_carry_the_shared_flags() {
        let names: Vec<&str> = common_args().iter().map(|a| a.name).collect();
        for expected in [
            "template",
            "agent",
            "model",
            "output-format",
            "output-file",
            "timeout",
            "max-retries",
        ] {
            assert!(names.contains(&expected), "common_args missing {expected}");
        }
    }
}
