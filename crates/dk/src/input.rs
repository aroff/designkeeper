//! CLI flag → domain type mapping and config resolution helpers.

use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use cli_framework::spec::value::ArgValue;

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

/// Extract a `&str` from `ArgValue::Str` or `ArgValue::Enum`.
fn get_str<'a>(args: &'a HashMap<String, ArgValue>, key: &str) -> Option<&'a str> {
    match args.get(key)? {
        ArgValue::Str(s) | ArgValue::Enum(s) => Some(s.as_str()),
        _ => None,
    }
}

pub fn resolve_common_args(args: &HashMap<String, ArgValue>) -> CommonArgs {
    let cwd = current_dir();
    let config = resolved_config(args, &cwd)
        .unwrap_or_else(|msg| crate::progress::fail("DK_CONFIG_PARSE", &msg));
    let input = map_input(args, &cwd, &config)
        .unwrap_or_else(|msg| crate::progress::fail("DK_INPUT_VALIDATION", &msg));
    let fmt = output_format(args, &config)
        .unwrap_or_else(|msg| crate::progress::fail("DK_INPUT_VALIDATION", &msg));
    let template_name = get_str(args, "template")
        .map(str::to_string)
        .unwrap_or_else(|| {
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
pub fn resolved_config(args: &HashMap<String, ArgValue>, cwd: &Path) -> Result<DkConfig, String> {
    let mut config = resolve_config(cwd).map_err(|e| e.to_string())?;
    if let Some(agent) = get_str(args, "agent") {
        config.agent.agent = agent.to_string();
    }
    if let Some(model) = get_str(args, "model") {
        config.agent.model = Some(model.to_string());
    }
    if let Some(ArgValue::Int(n)) = args.get("timeout") {
        config.agent.timeout_secs = if *n == 0 { None } else { Some(*n as u64) };
    }
    if let Some(ArgValue::Int(n)) = args.get("max-retries") {
        config.agent.max_retries = Some(*n as u32);
    }
    Ok(config)
}

pub fn output_format(
    args: &HashMap<String, ArgValue>,
    config: &DkConfig,
) -> Result<OutputFormat, String> {
    match get_str(args, "output-format") {
        Some(s) => OutputFormat::parse(s).ok_or_else(|| format!("invalid --output-format: {s}")),
        None => Ok(config.output.format),
    }
}

pub fn build_change_context(
    args: &HashMap<String, ArgValue>,
    cwd: &Path,
    config: &DkConfig,
) -> (Option<ChangeContext>, Option<String>) {
    let (git_ctx, git_target) = match get_str(args, "from-git") {
        Some(base) => {
            let (ctx, target) =
                dk_core::git::change_context_from_git(cwd, base, &config.scan.extensions);
            (Some(ctx), target)
        }
        None => (None, None),
    };

    let title = get_str(args, "title")
        .map(str::to_string)
        .or_else(|| git_ctx.as_ref()?.title.clone());
    let description = get_str(args, "description")
        .map(read_file_or_text)
        .or_else(|| git_ctx.as_ref()?.description.clone());
    let base_ref = get_str(args, "base-ref")
        .map(str::to_string)
        .or_else(|| git_ctx.as_ref()?.base_ref.clone());
    let head_ref = get_str(args, "head-ref")
        .map(str::to_string)
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

pub fn map_input(
    args: &HashMap<String, ArgValue>,
    cwd: &Path,
    config: &DkConfig,
) -> Result<ReviewInput, String> {
    let (change_context, git_target) = build_change_context(args, cwd, config);

    let target = get_str(args, "path").map(str::to_string).or(git_target);

    // `focus` is Cardinality::Repeated — arrives as ArgValue::List([ArgValue::Enum(...), ...])
    let focus = match args.get("focus") {
        Some(ArgValue::List(items)) => items
            .iter()
            .map(|v| match v {
                ArgValue::Enum(s) | ArgValue::Str(s) => FocusArea::parse(s)
                    .ok_or_else(|| format!("invalid --focus value: {s}")),
                _ => Err("invalid --focus value".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => vec![],
        Some(v) => return Err(format!("invalid --focus: {v}")),
    };

    // Range is enforced by ArgSpec min/max at parse time; default if absent.
    let max_findings = match args.get("max-findings") {
        Some(ArgValue::Int(n)) => *n as u8,
        _ => DEFAULT_MAX_FINDINGS,
    };

    let include_dimensions = {
        let dims = parse_comma_list(
            get_str(args, "include-dimensions"),
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

fn parse_comma_list<T, F>(arg: Option<&str>, flag_name: &str, parse: F) -> Result<Vec<T>, String>
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

pub fn flag(args: &HashMap<String, ArgValue>, name: &str) -> bool {
    matches!(args.get(name), Some(ArgValue::Bool(true)))
}

/// Return the effective SARIF output path: CLI flag takes precedence over config.
pub fn effective_sarif_path(
    args: &HashMap<String, ArgValue>,
    config: &DkConfig,
) -> Option<String> {
    get_str(args, "sarif")
        .map(str::to_string)
        .or_else(|| config.output.sarif_path.clone())
}

/// Write `content` to `--output-file` if set, otherwise to stdout.
pub fn emit(args: &HashMap<String, ArgValue>, content: &str) -> Result<(), std::io::Error> {
    match get_str(args, "output-file") {
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
pub fn prompt_or_default(flag: Option<&ArgValue>, label: &str, default: &str) -> String {
    if let Some(value) = flag.and_then(|v| match v {
        ArgValue::Str(s) | ArgValue::Enum(s) => Some(s.as_str()),
        _ => None,
    }) {
        return value.to_string();
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

    fn str_val(s: &str) -> ArgValue {
        ArgValue::Str(s.to_string())
    }

    fn args(pairs: &[(&str, ArgValue)]) -> HashMap<String, ArgValue> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
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
        let a = args(&[
            ("title", str_val("T")),
            ("description", str_val("raw body")),
            ("base-ref", str_val("main")),
            ("head-ref", str_val("HEAD")),
            (
                "focus",
                ArgValue::List(vec![
                    ArgValue::Enum("security".to_string()),
                    ArgValue::Enum("concurrency".to_string()),
                ]),
            ),
            ("max-findings", ArgValue::Int(10)),
        ]);
        let cwd = std::env::temp_dir();
        let input = map_input(&a, &cwd, &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("T"));
        assert_eq!(cc.description.as_deref(), Some("raw body"));
        assert_eq!(input.focus.len(), 2);
        assert_eq!(input.options.max_findings, 10);
    }

    #[test]
    fn map_input_rejects_bad_focus() {
        let cwd = std::env::temp_dir();
        assert!(map_input(
            &args(&[(
                "focus",
                ArgValue::List(vec![ArgValue::Enum("nope".to_string())])
            )]),
            &cwd,
            &default_config()
        )
        .is_err());
    }

    #[test]
    fn map_input_include_dimensions_rejects_invalid() {
        let cwd = std::env::temp_dir();
        assert!(map_input(
            &args(&[("include-dimensions", str_val("nope"))]),
            &cwd,
            &default_config()
        )
        .is_err());
    }

    #[test]
    fn map_input_include_dimensions_valid() {
        let cwd = std::env::temp_dir();
        let input = map_input(
            &args(&[("include-dimensions", str_val("design,tests"))]),
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
        let a = args(&[("agent", str_val("codex")), ("model", str_val("gpt-5"))]);
        let cfg = resolved_config(&a, dir.path()).unwrap();
        assert_eq!(cfg.agent.agent, "codex");
        assert_eq!(cfg.agent.model.as_deref(), Some("gpt-5"));

        let cfg2 = resolved_config(&args(&[]), dir.path()).unwrap();
        assert_eq!(cfg2.agent.agent, "claude");
        assert_eq!(cfg2.agent.model, None);
    }

    #[test]
    fn output_format_defaults_to_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = resolved_config(&args(&[]), dir.path()).unwrap();
        assert_eq!(
            output_format(&args(&[]), &cfg).unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            output_format(&args(&[("output-format", str_val("json"))]), &cfg).unwrap(),
            OutputFormat::Json
        );
    }

    #[test]
    fn output_format_rejects_invalid_value() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = resolved_config(&args(&[]), dir.path()).unwrap();
        let err = output_format(&args(&[("output-format", str_val("xml"))]), &cfg);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("xml"));
    }

    #[test]
    fn flag_detection() {
        assert!(flag(
            &args(&[("verbose", ArgValue::Bool(true))]),
            "verbose"
        ));
        assert!(!flag(&args(&[]), "verbose"));
    }

    #[test]
    fn emit_writes_to_output_file_creating_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("nested").join("deep").join("report.md");
        let a = args(&[("output-file", str_val(out.to_str().unwrap()))]);
        emit(&a, "hello world").unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "hello world");
    }

    #[test]
    fn emit_without_output_file_returns_ok() {
        assert!(emit(&args(&[]), "to stdout").is_ok());
    }

    #[test]
    fn test_sarif_flag_overrides_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("dk.toml"),
            "[output]\nsarif_path = \"default.sarif\"\n",
        )
        .unwrap();
        let cfg = resolved_config(&args(&[]), dir.path()).unwrap();
        let a = args(&[("sarif", str_val("override.sarif"))]);
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
        let cfg = resolved_config(&args(&[]), dir.path()).unwrap();
        assert_eq!(
            effective_sarif_path(&args(&[]), &cfg).as_deref(),
            Some("default.sarif")
        );
    }

    #[test]
    fn map_input_from_git_populates_title_in_git_repo() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "my pr title") {
            return;
        }
        let a = args(&[("from-git", str_val("HEAD"))]);
        let input = map_input(&a, dir.path(), &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("my pr title"));
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
        let a = args(&[
            ("from-git", str_val("HEAD")),
            ("title", str_val("Override")),
        ]);
        let input = map_input(&a, dir.path(), &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("Override"));
    }

    #[test]
    fn map_input_from_git_non_git_dir_does_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let a = args(&[("from-git", str_val("main"))]);
        let result = map_input(&a, dir.path(), &default_config());
        assert!(result.is_ok());
        let input = result.unwrap();
        let cc = input.change_context.unwrap();
        assert!(cc.title.is_none());
        assert!(cc.diff_stat.is_none());
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
        let a = args(&[("from-git", str_val("HEAD"))]);
        let input = map_input(&a, dir.path(), &default_config()).unwrap();
        let cc = input.change_context.unwrap();
        assert_eq!(cc.title.as_deref(), Some("check title"));
        assert_eq!(cc.base_ref.as_deref(), Some("HEAD"));
        assert_eq!(cc.head_ref.as_deref(), Some("HEAD"));
    }
}
