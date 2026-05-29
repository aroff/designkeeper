//! `dk.toml` resolution and built-in defaults.

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

/// Default recognized source extensions (see CONTEXT.md / spec §4.2).
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".c", ".cpp", ".h", ".hpp",
    ".rb", ".ex", ".exs", ".scala", ".swift", ".cs",
];

#[derive(Debug, Clone, PartialEq)]
pub struct DkConfig {
    pub scan: ScanConfig,
    pub output: OutputConfig,
    pub agent: AgentConfig,
    pub templates: TemplatesConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanConfig {
    pub extensions: Vec<String>,
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputConfig {
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Markdown,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => Some(Self::Markdown),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub agent: String,
    pub model: Option<String>,
    pub timeout_secs: Option<u64>,
    pub max_retries: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplatesConfig {
    pub pack: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    /// `dk.toml` found but contained invalid TOML or unknown fields.
    #[error("failed to parse dk.toml at {path}: {message}")]
    Parse { path: String, message: String },
    #[error("io error reading dk.toml: {0}")]
    Io(#[from] std::io::Error),
}

impl ConfigError {
    pub fn code(&self) -> &'static str {
        match self {
            ConfigError::Parse { .. } => "DK_CONFIG_PARSE",
            ConfigError::Io(_) => "DK_IO_ERROR",
        }
    }
}

/// Walk up from `start` calling `pred` on each directory. Returns the first
/// `pred(dir)` that returns `Some(T)`, or `None` if the root is reached.
pub fn find_up<F, T>(start: &Path, mut pred: F) -> Option<T>
where
    F: FnMut(&Path) -> Option<T>,
{
    let mut dir = Some(start);
    while let Some(current) = dir {
        if let Some(result) = pred(current) {
            return Some(result);
        }
        dir = current.parent();
    }
    None
}

/// Built-in defaults used when no `dk.toml` is found.
pub fn default_config() -> DkConfig {
    DkConfig {
        scan: ScanConfig {
            extensions: DEFAULT_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
            ignore_patterns: Vec::new(),
        },
        output: OutputConfig {
            format: OutputFormat::Markdown,
        },
        agent: AgentConfig {
            agent: "claude".to_string(),
            model: None,
            timeout_secs: None,
            max_retries: None,
        },
        templates: TemplatesConfig {
            pack: "default".to_string(),
        },
    }
}

/// Walk up from `working_dir` looking for `dk.toml`. Parse it into [`DkConfig`]
/// (filling absent sections from defaults). Absent file -> defaults, no error.
pub fn resolve_config(working_dir: &Path) -> Result<DkConfig, ConfigError> {
    find_up(working_dir, |dir| {
        let candidate = dir.join("dk.toml");
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    })
    .map(|path| {
        let text = std::fs::read_to_string(&path)?;
        let raw: RawConfig = toml::from_str(&text).map_err(|e| ConfigError::Parse {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        Ok(raw.into_config())
    })
    .transpose()
    .map(|opt| opt.unwrap_or_else(default_config))
}

// ---- TOML deserialization shapes (all optional, merged over defaults) ----

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    scan: Option<RawScan>,
    #[serde(default)]
    output: Option<RawOutput>,
    #[serde(default)]
    agent: Option<RawAgent>,
    #[serde(default)]
    templates: Option<RawTemplates>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScan {
    extensions: Option<Vec<String>>,
    ignore_patterns: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutput {
    format: Option<OutputFormat>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgent {
    agent: Option<String>,
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_retries: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplates {
    pack: Option<String>,
}

impl RawConfig {
    fn into_config(self) -> DkConfig {
        let mut cfg = default_config();
        if let Some(scan) = self.scan {
            if let Some(ext) = scan.extensions {
                cfg.scan.extensions = ext;
            }
            if let Some(ignore) = scan.ignore_patterns {
                cfg.scan.ignore_patterns = ignore;
            }
        }
        if let Some(output) = self.output {
            if let Some(format) = output.format {
                cfg.output.format = format;
            }
        }
        if let Some(agent) = self.agent {
            if let Some(a) = agent.agent {
                cfg.agent.agent = a;
            }
            // Treat an empty model string as "unset" (CONTEXT.md uses model = "").
            cfg.agent.model = agent.model.filter(|m| !m.trim().is_empty());
            cfg.agent.timeout_secs = agent.timeout_secs;
            cfg.agent.max_retries = agent.max_retries;
        }
        if let Some(templates) = self.templates {
            if let Some(pack) = templates.pack {
                cfg.templates.pack = pack;
            }
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_file_returns_defaults() {
        let dir = tempdir().unwrap();
        let cfg = resolve_config(dir.path()).unwrap();
        assert_eq!(cfg, default_config());
        assert_eq!(cfg.agent.agent, "claude");
        assert_eq!(cfg.output.format, OutputFormat::Markdown);
        assert_eq!(cfg.scan.extensions.len(), DEFAULT_EXTENSIONS.len());
    }

    #[test]
    fn parses_valid_toml() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("dk.toml"),
            r#"
[scan]
extensions = [".rs", ".ts"]
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "json"

[agent]
agent = "codex"
model = "gpt-5"

[templates]
pack = "https://example.com/pack"
"#,
        )
        .unwrap();
        let cfg = resolve_config(dir.path()).unwrap();
        assert_eq!(cfg.scan.extensions, vec![".rs", ".ts"]);
        assert_eq!(cfg.scan.ignore_patterns, vec!["vendor/", "generated/"]);
        assert_eq!(cfg.output.format, OutputFormat::Json);
        assert_eq!(cfg.agent.agent, "codex");
        assert_eq!(cfg.agent.model.as_deref(), Some("gpt-5"));
        assert_eq!(cfg.templates.pack, "https://example.com/pack");
    }

    #[test]
    fn empty_model_is_unset() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("dk.toml"),
            "[agent]\nagent = \"claude\"\nmodel = \"\"\n",
        )
        .unwrap();
        let cfg = resolve_config(dir.path()).unwrap();
        assert_eq!(cfg.agent.model, None);
    }

    #[test]
    fn walks_up_to_find_config() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("dk.toml"), "[agent]\nagent = \"root\"\n").unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        let cfg = resolve_config(&nested).unwrap();
        assert_eq!(cfg.agent.agent, "root");
    }

    #[test]
    fn invalid_toml_errors_with_code() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("dk.toml"), "this is = = not toml").unwrap();
        let err = resolve_config(dir.path()).unwrap_err();
        assert_eq!(err.code(), "DK_CONFIG_PARSE");
    }

    #[test]
    fn unknown_field_errors() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("dk.toml"), "[scan]\nbogus = 3\n").unwrap();
        let err = resolve_config(dir.path()).unwrap_err();
        assert_eq!(err.code(), "DK_CONFIG_PARSE");
    }
}
