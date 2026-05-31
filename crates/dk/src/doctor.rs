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
use dk_core::pack_store;

use crate::input::current_dir;

/// All `dk` doctor checks, ready to hand to `DoctorModule::new`.
pub fn checks() -> Vec<std::sync::Arc<dyn DoctorCheck>> {
    vec![
        std::sync::Arc::new(ConfigCheck { dir: None }),
        std::sync::Arc::new(InstalledPacksCheck { dir: None }),
        std::sync::Arc::new(InstalledAgentsCheck),
        std::sync::Arc::new(AgentReachabilityCheck { dir: None }),
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

// ---------------------------------------------------------------------------

/// Each check resolves its working directory from `dir` when set, falling back
/// to the process CWD. Production wiring leaves `dir` as `None`; tests inject a
/// controlled directory so the check is deterministic.
struct ConfigCheck {
    dir: Option<PathBuf>,
}
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
        let id = self.id();
        let title = self.title();
        let dir_override = self.dir.clone();
        Box::pin(async move {
            let dir = dir_override.unwrap_or_else(current_dir);
            let config_path = find_up(&dir, |d| {
                let p = d.join("dk.toml");
                if p.is_file() {
                    Some(p)
                } else {
                    None
                }
            });
            match resolve_config(&dir) {
                Ok(cfg) => {
                    let model = cfg.agent.model.as_deref().unwrap_or("(default)");
                    let detail = format!(
                        "agent = {}\nmodel = {}\noutput = {:?}",
                        cfg.agent.agent, model, cfg.output.format
                    );
                    let (message, remediation) = match &config_path {
                        Some(path) => (format!("Using {}", path.display()), None),
                        None => (
                            "No dk.toml found; using built-in defaults".to_string(),
                            Some("Run `dk init` to create a dk.toml.".to_string()),
                        ),
                    };
                    finding(
                        id,
                        title,
                        CheckSeverity::Ok,
                        message,
                        Some(detail),
                        remediation,
                    )
                }
                Err(e) => finding(
                    id,
                    title,
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

struct InstalledPacksCheck {
    dir: Option<PathBuf>,
}
impl DoctorCheck for InstalledPacksCheck {
    fn id(&self) -> &'static str {
        "installed-packs"
    }
    fn title(&self) -> &'static str {
        "Installed template packs"
    }
    fn description(&self) -> Option<&'static str> {
        Some("Lists installed template packs (project-local and global)")
    }
    fn run(&self, _ctx: &dyn AppContext) -> DoctorFuture {
        let id = self.id();
        let title = self.title();
        let dir_override = self.dir.clone();
        Box::pin(async move {
            let dir = dir_override.unwrap_or_else(current_dir);
            let packs = pack_store::list_packs(&dir);
            if packs.is_empty() {
                finding(
                    id,
                    title,
                    CheckSeverity::Warning,
                    "No template packs installed. Built-in fallbacks (default, structural) are available.".to_string(),
                    None,
                    Some("Run `dk install` to fetch and install packs.".to_string()),
                )
            } else {
                let detail = packs
                    .iter()
                    .map(|p| format!("{} ({}) — {}", p.name, p.scope, p.path.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                finding(
                    id,
                    title,
                    CheckSeverity::Ok,
                    format!("{} pack(s) installed", packs.len()),
                    Some(detail),
                    None,
                )
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
        let id = self.id();
        let title = self.title();
        Box::pin(async move {
            let infos = AgentDetector::detect();
            let found: Vec<String> = infos
                .iter()
                .filter(|i| i.installed)
                .map(|i| i.key.clone())
                .collect();
            if found.is_empty() {
                let all_keys: Vec<String> = infos.iter().map(|i| i.key.clone()).collect();
                finding(
                    id,
                    title,
                    CheckSeverity::Warning,
                    "No known agent CLIs found on PATH".to_string(),
                    Some(format!("Looked for: {}", all_keys.join(", "))),
                    Some("Install an agent CLI (e.g. claude, codex, gemini).".to_string()),
                )
            } else {
                finding(
                    id,
                    title,
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

struct AgentReachabilityCheck {
    dir: Option<PathBuf>,
}
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
        let id = self.id();
        let title = self.title();
        let dir_override = self.dir.clone();
        Box::pin(async move {
            let dir = dir_override.unwrap_or_else(current_dir);
            let cfg = match resolve_config(&dir) {
                Ok(c) => c,
                Err(_) => {
                    return finding(
                        id,
                        title,
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
                    id,
                    title,
                    CheckSeverity::Ok,
                    format!("Agent '{key}' is installed and reachable"),
                    None,
                    None,
                )
            } else {
                let detail = reason.map(|r| format!("Reason: {r}"));
                finding(
                    id,
                    title,
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCtx;
    impl AppContext for TestCtx {}

    fn at(dir: &std::path::Path) -> Option<PathBuf> {
        Some(dir.to_path_buf())
    }

    #[tokio::test]
    async fn config_reports_built_in_defaults_when_no_dk_toml() {
        let dir = tempfile::tempdir().unwrap();
        let f = ConfigCheck {
            dir: at(dir.path()),
        }
        .run(&TestCtx)
        .await;
        assert_eq!(f.severity, CheckSeverity::Ok);
        assert!(
            f.message.contains("built-in defaults"),
            "message: {}",
            f.message
        );
        assert!(
            f.remediation.unwrap().contains("dk init"),
            "should suggest dk init"
        );
    }

    #[tokio::test]
    async fn config_reports_using_path_when_dk_toml_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dk.toml"), "[agent]\nagent = \"claude\"\n").unwrap();
        let f = ConfigCheck {
            dir: at(dir.path()),
        }
        .run(&TestCtx)
        .await;
        assert_eq!(f.severity, CheckSeverity::Ok);
        assert!(f.message.contains("Using"), "message: {}", f.message);
        assert!(f.remediation.is_none(), "no remediation when config exists");
    }

    #[tokio::test]
    async fn config_errors_on_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dk.toml"), "this is = = not valid").unwrap();
        let f = ConfigCheck {
            dir: at(dir.path()),
        }
        .run(&TestCtx)
        .await;
        assert_eq!(f.severity, CheckSeverity::Error);
        assert!(f.message.contains("invalid"), "message: {}", f.message);
    }

    #[tokio::test]
    async fn installed_packs_lists_a_project_pack() {
        let dir = tempfile::tempdir().unwrap();
        let prompt = dir
            .path()
            .join(".dk")
            .join("packs")
            .join("acme")
            .join("templates")
            .join("review.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(&prompt, "# prompt").unwrap();

        let f = InstalledPacksCheck {
            dir: at(dir.path()),
        }
        .run(&TestCtx)
        .await;
        assert_eq!(f.severity, CheckSeverity::Ok);
        assert!(
            f.message.contains("pack(s) installed"),
            "message: {}",
            f.message
        );
        assert!(
            f.detail.unwrap().contains("acme"),
            "detail should name the installed pack"
        );
    }

    #[tokio::test]
    async fn agent_reachability_skipped_when_config_invalid() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dk.toml"), "this is = = not valid").unwrap();
        let f = AgentReachabilityCheck {
            dir: at(dir.path()),
        }
        .run(&TestCtx)
        .await;
        assert_eq!(f.severity, CheckSeverity::Skipped);
        assert!(
            f.message.contains("could not be parsed"),
            "message: {}",
            f.message
        );
    }
}
