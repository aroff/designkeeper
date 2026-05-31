//! CLI flag → domain type mapping and config resolution helpers.

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use cli_framework::prelude::CommandArgs;

use dk_core::config::{resolve_config, DkConfig, OutputFormat};
use dk_core::pack_store;
use dk_core::{
    ChangeContext, Dimension, FocusArea, ReviewInput, ReviewOptions, DEFAULT_MAX_FINDINGS,
};

pub fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub struct CommonArgs {
    #[allow(dead_code)]
    pub cwd: PathBuf,
    pub config: DkConfig,
    pub input: ReviewInput,
    pub template_dir: PathBuf,
    pub output_format: OutputFormat,
}

pub fn resolve_common_args(args: &CommandArgs) -> CommonArgs {
    let cwd = current_dir();
    let config = resolved_config(args, &cwd)
        .unwrap_or_else(|msg| crate::progress::fail("DK_CONFIG_PARSE", &msg));
    let input = map_input(args, &cwd, &config)
        .unwrap_or_else(|msg| crate::progress::fail("DK_INPUT_VALIDATION", &msg));
    let fmt = output_format(args, &config)
        .unwrap_or_else(|msg| crate::progress::fail("DK_INPUT_VALIDATION", &msg));
    let template_name = args.named.get("template").cloned().unwrap_or_else(|| {
        crate::progress::fail(
            "DK_INPUT_VALIDATION",
            "--template is required. Run `dk install` to see available packs.",
        )
    });
    let template_dir = pack_store::resolve_pack(&template_name, &cwd)
        .unwrap_or_else(|e| crate::progress::fail(e.code(), &e.to_string()));
    CommonArgs {
        cwd,
        config,
        input,
        template_dir,
        output_format: fmt,
    }
}

/// Resolve config from `dk.toml`, then apply CLI agent/model overrides
/// (CLI > dk.toml > built-in defaults).
pub fn resolved_config(args: &CommandArgs, cwd: &Path) -> Result<DkConfig, String> {
    let mut config = resolve_config(cwd).map_err(|e| e.to_string())?;
    if let Some(agent) = args.named.get("agent") {
        config.agent.agent = agent.clone();
    }
    if let Some(model) = args.named.get("model") {
        config.agent.model = Some(model.clone());
    }
    if let Some(s) = args.named.get("timeout") {
        let n: u64 = s
            .parse::<u64>()
            .map_err(|_| format!("invalid --timeout: {s}"))?;
        config.agent.timeout_secs = if n == 0 { None } else { Some(n) };
    }
    if let Some(s) = args.named.get("max-retries") {
        let n: u32 = s
            .parse::<u32>()
            .map_err(|_| format!("invalid --max-retries: {s}"))?;
        config.agent.max_retries = Some(n);
    }
    Ok(config)
}

pub fn output_format(args: &CommandArgs, config: &DkConfig) -> Result<OutputFormat, String> {
    match args.named.get("output-format") {
        Some(s) => OutputFormat::parse(s).ok_or_else(|| format!("invalid --output-format: {s}")),
        None => Ok(config.output.format),
    }
}

pub fn build_change_context(
    args: &CommandArgs,
    cwd: &Path,
    config: &DkConfig,
) -> (Option<ChangeContext>, Option<String>) {
    let (git_ctx, git_target) = match args.named.get("from-git") {
        Some(base) => {
            let (ctx, target) =
                dk_core::git::change_context_from_git(cwd, base, &config.scan.extensions);
            (Some(ctx), target)
        }
        None => (None, None),
    };

    let title = args
        .named
        .get("title")
        .cloned()
        .or_else(|| git_ctx.as_ref()?.title.clone());
    let description = args
        .named
        .get("description")
        .map(|d| read_file_or_text(d))
        .or_else(|| git_ctx.as_ref()?.description.clone());
    let base_ref = args
        .named
        .get("base-ref")
        .cloned()
        .or_else(|| git_ctx.as_ref()?.base_ref.clone());
    let head_ref = args
        .named
        .get("head-ref")
        .cloned()
        .or_else(|| git_ctx.as_ref()?.head_ref.clone());
    let git_diff_stat = git_ctx.and_then(|c| c.diff_stat);

    let diff_stat = if let (Some(base), Some(head)) = (&base_ref, &head_ref) {
        git_diff_stat.or_else(|| dk_core::git::diff_stat(cwd, base, head))
    } else {
        git_diff_stat
    };

    let cc = ChangeContext {
        title,
        description,
        base_ref,
        head_ref,
        diff_stat,
    };
    let change_context = if !cc.is_empty() { Some(cc) } else { None };

    (change_context, git_target)
}

pub fn map_input(args: &CommandArgs, cwd: &Path, config: &DkConfig) -> Result<ReviewInput, String> {
    let (change_context, git_target) = build_change_context(args, cwd, config);

    let target = args.named.get("path").cloned().or(git_target);

    let focus = parse_comma_list(args.named.get("focus"), "--focus", FocusArea::parse)?;

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
        None => DEFAULT_MAX_FINDINGS,
    };

    let include_dimensions = {
        let dims = parse_comma_list(
            args.named.get("include-dimensions"),
            "--include-dimensions",
            Dimension::parse,
        )?;
        if dims.is_empty() {
            None
        } else {
            Some(dims)
        }
    };

    Ok(ReviewInput {
        working_dir: cwd.to_string_lossy().into_owned(),
        target,
        change_context,
        focus,
        project_hints: None,
        options: ReviewOptions {
            max_findings,
            include_dimensions,
        },
    })
}

fn parse_comma_list<T, F>(arg: Option<&String>, flag_name: &str, parse: F) -> Result<Vec<T>, String>
where
    F: Fn(&str) -> Option<T>,
{
    match arg {
        None => Ok(vec![]),
        Some(s) => s
            .split(',')
            .filter(|x| !x.is_empty())
            .map(|x| parse(x).ok_or_else(|| format!("invalid {flag_name} value: {x}")))
            .collect(),
    }
}

/// `--description` accepts a file path (if it exists) or raw text (AC #19).
pub fn read_file_or_text(value: &str) -> String {
    let path = Path::new(value);
    if path.is_file() {
        if let Ok(contents) = std::fs::read_to_string(path) {
            return contents;
        }
    }
    value.to_string()
}

pub fn flag(args: &CommandArgs, name: &str) -> bool {
    args.named.get(name).map(|v| v == "true").unwrap_or(false)
}

/// Return the effective SARIF output path: CLI flag takes precedence over config.
pub fn effective_sarif_path(args: &CommandArgs, config: &DkConfig) -> Option<String> {
    args.named
        .get("sarif")
        .cloned()
        .or_else(|| config.output.sarif_path.clone())
}

/// Write `content` to `--output-file` if set, otherwise to stdout.
pub fn emit(args: &CommandArgs, content: &str) -> Result<(), std::io::Error> {
    match args.named.get("output-file") {
        Some(path) => {
            if let Some(parent) = Path::new(path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, content)
        }
        None => {
            println!("{content}");
            Ok(())
        }
    }
}

/// Resolve a parameter from a CLI flag, an interactive prompt (TTY only), or
/// the supplied default. Non-interactive invocations silently take the default.
pub fn prompt_or_default(flag: Option<&String>, label: &str, default: &str) -> String {
    if let Some(value) = flag {
        return value.clone();
    }
    if io::stdin().is_terminal() {
        print!("{label} [{default}]: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dk_core::config::default_config;
    use dk_core::testutil::{git_available, init_repo_with_commit};

    fn args(named: &[(&str, &str)], positional: &[&str]) -> CommandArgs {
        CommandArgs {
            positional: positional.iter().map(|s| s.to_string()).collect(),
            named: named
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            named_typed: std::collections::HashMap::new(),
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
        let input = map_input(&a, &cwd, &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("T"));
        assert_eq!(cc.description.as_deref(), Some("raw body"));
        assert_eq!(input.focus.len(), 2);
        assert_eq!(input.options.max_findings, 10);
    }

    #[test]
    fn map_input_rejects_bad_focus_and_range() {
        let cwd = std::env::temp_dir();
        assert!(map_input(&args(&[("focus", "nope")], &[]), &cwd, &default_config()).is_err());
        assert!(map_input(
            &args(&[("max-findings", "99")], &[]),
            &cwd,
            &default_config()
        )
        .is_err());
    }

    #[test]
    fn map_input_include_dimensions_rejects_invalid() {
        let cwd = std::env::temp_dir();
        assert!(map_input(
            &args(&[("include-dimensions", "nope")], &[]),
            &cwd,
            &default_config()
        )
        .is_err());
    }

    #[test]
    fn map_input_include_dimensions_valid() {
        let cwd = std::env::temp_dir();
        let input = map_input(
            &args(&[("include-dimensions", "design,tests")], &[]),
            &cwd,
            &default_config(),
        )
        .unwrap();
        let dims = input.options.include_dimensions.unwrap();
        assert_eq!(dims.len(), 2);
    }

    #[test]
    fn agent_model_precedence_cli_over_config() {
        let dir = tempfile::tempdir().unwrap();
        let a = args(&[("agent", "codex"), ("model", "gpt-5")], &[]);
        let cfg = resolved_config(&a, dir.path()).unwrap();
        assert_eq!(cfg.agent.agent, "codex");
        assert_eq!(cfg.agent.model.as_deref(), Some("gpt-5"));

        let cfg2 = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        assert_eq!(cfg2.agent.agent, "claude");
        assert_eq!(cfg2.agent.model, None);
    }

    #[test]
    fn output_format_defaults_to_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        assert_eq!(
            output_format(&args(&[], &[]), &cfg).unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            output_format(&args(&[("output-format", "json")], &[]), &cfg).unwrap(),
            OutputFormat::Json
        );
    }

    #[test]
    fn output_format_rejects_invalid_value() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        let err = output_format(&args(&[("output-format", "xml")], &[]), &cfg);
        assert!(err.is_err(), "xml is not a valid output format");
        let msg = err.unwrap_err();
        assert!(
            msg.contains("xml"),
            "error message should mention the invalid value"
        );
    }

    #[test]
    fn flag_detection() {
        assert!(flag(&args(&[("verbose", "true")], &[]), "verbose"));
        assert!(!flag(&args(&[], &[]), "verbose"));
    }

    #[test]
    fn emit_writes_to_output_file_creating_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("nested").join("deep").join("report.md");
        let a = args(&[("output-file", out.to_str().unwrap())], &[]);
        emit(&a, "hello world").unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "hello world",
            "emit should write content and create missing parent dirs"
        );
    }

    #[test]
    fn emit_without_output_file_returns_ok() {
        // No --output-file: content goes to stdout, call still succeeds.
        assert!(emit(&args(&[], &[]), "to stdout").is_ok());
    }

    #[test]
    fn test_sarif_flag_overrides_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("dk.toml"),
            "[output]\nsarif_path = \"default.sarif\"\n",
        )
        .unwrap();
        let cfg = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        let a = args(&[("sarif", "override.sarif")], &[]);
        assert_eq!(
            effective_sarif_path(&a, &cfg).as_deref(),
            Some("override.sarif")
        );
    }

    #[test]
    fn test_sarif_config_used_when_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("dk.toml"),
            "[output]\nsarif_path = \"default.sarif\"\n",
        )
        .unwrap();
        let cfg = resolved_config(&args(&[], &[]), dir.path()).unwrap();
        assert_eq!(
            effective_sarif_path(&args(&[], &[]), &cfg).as_deref(),
            Some("default.sarif")
        );
    }

    // ---- AC-FG: --from-git tests -------------------------------------------

    #[test]
    fn map_input_from_git_populates_title_in_git_repo() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "my pr title") {
            return;
        }
        let a = args(&[("from-git", "HEAD")], &[]);
        let input = map_input(&a, dir.path(), &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(
            cc.title.as_deref(),
            Some("my pr title"),
            "title should come from git log"
        );
    }

    #[test]
    fn map_input_from_git_explicit_title_overrides_git() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "git title") {
            return;
        }
        let a = args(&[("from-git", "HEAD"), ("title", "Override")], &[]);
        let input = map_input(&a, dir.path(), &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(
            cc.title.as_deref(),
            Some("Override"),
            "explicit --title should override git-derived title"
        );
    }

    #[test]
    fn map_input_from_git_non_git_dir_does_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let a = args(&[("from-git", "main")], &[]);
        let result = map_input(&a, dir.path(), &default_config());
        assert!(result.is_ok(), "from-git in non-git dir must not fail");
        let input = result.unwrap();
        let cc = input.change_context.unwrap();
        assert!(cc.title.is_none(), "title should be None in non-git dir");
        assert!(
            cc.diff_stat.is_none(),
            "diff_stat should be None in non-git dir"
        );
    }

    #[test]
    fn map_input_from_git_works_for_check_command_path() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "check title") {
            return;
        }
        let a = args(&[("from-git", "HEAD")], &[]);
        let input = map_input(&a, dir.path(), &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("check title"));
        assert_eq!(cc.base_ref.as_deref(), Some("HEAD"));
        assert_eq!(cc.head_ref.as_deref(), Some("HEAD"));
    }
}
