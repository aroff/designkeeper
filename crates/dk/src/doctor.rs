//! `dk doctor` diagnostic checks.
//!
//! Reports on the runtime environment (CONTEXT.md §"doctor"): the effective
//! configuration file, template-pack status, which agent CLIs are installed,
//! and whether the configured agent is reachable on `PATH`.

use std::path::PathBuf;

use aikit_sdk::agent_runner::AgentDetector;
use cli_framework::doctor::{CheckSeverity, DoctorCheck, DoctorFinding, DoctorFuture};
use cli_framework::prelude::AppContext;

use dk_core::config::{find_up, resolve_config};
use dk_core::pack;

/// All `dk` doctor checks, ready to hand to `DoctorModule::new`.
pub fn checks() -> Vec<std::sync::Arc<dyn DoctorCheck>> {
    vec![
        std::sync::Arc::new(ConfigCheck),
        std::sync::Arc::new(TemplatePackCheck),
        std::sync::Arc::new(InstalledAgentsCheck),
        std::sync::Arc::new(AgentReachabilityCheck),
    ]
}

fn finding(
    id: &str,
    title: &str,
    severity: CheckSeverity,
    message: String,
    detail: Option<String>,
    remediation: Option<String>,
) -> DoctorFinding {
    DoctorFinding {
        check_id: id.to_string(),
        title: title.to_string(),
        severity,
        message,
        detail,
        remediation,
    }
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

// ---------------------------------------------------------------------------

struct ConfigCheck;
impl DoctorCheck for ConfigCheck {
    fn id(&self) -> &'static str {
        "config"
    }
    fn title(&self) -> &'static str {
        "Effective configuration"
    }
    fn description(&self) -> Option<&'static str> {
        Some("Resolves dk.toml (walking up) and reports the effective values")
    }
    fn run(&self, _ctx: &dyn AppContext) -> DoctorFuture {
        Box::pin(async {
            let dir = cwd();
            match resolve_config(&dir) {
                Ok(cfg) => {
                    let model = cfg.agent.model.as_deref().unwrap_or("(default)");
                    let detail = format!(
                        "agent = {}\nmodel = {}\noutput = {:?}\npack = {}",
                        cfg.agent.agent, model, cfg.output.format, cfg.templates.pack
                    );
                    match find_up(&dir, |d| {
                        let p = d.join("dk.toml");
                        if p.is_file() { Some(p) } else { None }
                    }) {
                        Some(path) => finding(
                            "config",
                            "Effective configuration",
                            CheckSeverity::Ok,
                            format!("Using {}", path.display()),
                            Some(detail),
                            None,
                        ),
                        None => finding(
                            "config",
                            "Effective configuration",
                            CheckSeverity::Ok,
                            "No dk.toml found; using built-in defaults".to_string(),
                            Some(detail),
                            Some("Run `dk init` to create a dk.toml.".to_string()),
                        ),
                    }
                }
                Err(e) => finding(
                    "config",
                    "Effective configuration",
                    CheckSeverity::Error,
                    format!("dk.toml is invalid: {e}"),
                    None,
                    Some("Fix the TOML syntax or run `dk init` to regenerate it.".to_string()),
                ),
            }
        })
    }
}

// ---------------------------------------------------------------------------

struct TemplatePackCheck;
impl DoctorCheck for TemplatePackCheck {
    fn id(&self) -> &'static str {
        "template-pack"
    }
    fn title(&self) -> &'static str {
        "Template pack"
    }
    fn description(&self) -> Option<&'static str> {
        Some("Checks for an installed .dk/ template pack")
    }
    fn run(&self, _ctx: &dyn AppContext) -> DoctorFuture {
        Box::pin(async {
            let dir = cwd();
            match find_up(&dir, |d| {
                let dk = d.join(".dk");
                if dk.exists() { Some(dk) } else { None }
            }) {
                Some(dk) if pack::prompt_path(&dk).is_file() => finding(
                    "template-pack",
                    "Template pack",
                    CheckSeverity::Ok,
                    format!("Installed at {}", dk.display()),
                    None,
                    None,
                ),
                _ => finding(
                    "template-pack",
                    "Template pack",
                    CheckSeverity::Warning,
                    "No .dk/ template pack found; using embedded defaults".to_string(),
                    None,
                    Some("Run `dk init` to install an editable template pack.".to_string()),
                ),
            }
        })
    }
}

// ---------------------------------------------------------------------------

struct InstalledAgentsCheck;
impl DoctorCheck for InstalledAgentsCheck {
    fn id(&self) -> &'static str {
        "installed-agents"
    }
    fn title(&self) -> &'static str {
        "Installed agents"
    }
    fn description(&self) -> Option<&'static str> {
        Some("Detects which agent CLIs are installed via AgentDetector")
    }
    fn run(&self, _ctx: &dyn AppContext) -> DoctorFuture {
        Box::pin(async {
            let infos = AgentDetector::detect();
            let found: Vec<String> = infos
                .iter()
                .filter(|i| i.installed)
                .map(|i| i.key.clone())
                .collect();
            if found.is_empty() {
                let all_keys: Vec<String> = infos.iter().map(|i| i.key.clone()).collect();
                finding(
                    "installed-agents",
                    "Installed agents",
                    CheckSeverity::Warning,
                    "No known agent CLIs found on PATH".to_string(),
                    Some(format!("Looked for: {}", all_keys.join(", "))),
                    Some("Install an agent CLI (e.g. claude, codex, gemini).".to_string()),
                )
            } else {
                finding(
                    "installed-agents",
                    "Installed agents",
                    CheckSeverity::Ok,
                    format!("Detected: {}", found.join(", ")),
                    None,
                    None,
                )
            }
        })
    }
}

// ---------------------------------------------------------------------------

struct AgentReachabilityCheck;
impl DoctorCheck for AgentReachabilityCheck {
    fn id(&self) -> &'static str {
        "agent-reachability"
    }
    fn title(&self) -> &'static str {
        "Configured agent reachability"
    }
    fn description(&self) -> Option<&'static str> {
        Some("Verifies the configured agent's CLI is on PATH")
    }
    fn run(&self, _ctx: &dyn AppContext) -> DoctorFuture {
        Box::pin(async {
            let dir = cwd();
            let cfg = match resolve_config(&dir) {
                Ok(c) => c,
                Err(_) => {
                    return finding(
                        "agent-reachability",
                        "Configured agent reachability",
                        CheckSeverity::Skipped,
                        "Skipped: dk.toml could not be parsed".to_string(),
                        None,
                        None,
                    )
                }
            };
            let key = cfg.agent.agent.as_str();

            let infos = AgentDetector::detect();
            let info = infos.iter().find(|i| i.key == key);
            let installed = info.map(|i| i.installed).unwrap_or(false);
            let reason = info.and_then(|i| i.reason.clone());

            if installed {
                finding(
                    "agent-reachability",
                    "Configured agent reachability",
                    CheckSeverity::Ok,
                    format!("Agent '{key}' is installed and reachable"),
                    None,
                    None,
                )
            } else {
                let detail = reason.map(|r| format!("Reason: {r}"));
                finding(
                    "agent-reachability",
                    "Configured agent reachability",
                    CheckSeverity::Error,
                    format!("Configured agent '{key}' not found on PATH"),
                    detail,
                    Some(format!(
                        "Install '{key}', or set a different agent via `dk init -a <agent>`."
                    )),
                )
            }
        })
    }
}
