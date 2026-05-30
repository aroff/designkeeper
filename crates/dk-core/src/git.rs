//! Git helpers used by the review pipeline (best-effort; all return Option<_>).

use std::path::Path;
use std::process::Command;

/// Run `git diff --stat <base>...<head>` in `cwd`.
/// Returns `None` on any error (git not found, non-zero exit, etc.).
pub fn diff_stat(cwd: &Path, base: &str, head: &str) -> Option<String> {
    let range = format!("{}...{}", base, head);
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["diff", "--stat", &range])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Run `git diff --name-only <base>...<head>` in `cwd`.
/// Returns `None` on any error.
pub fn changed_files(cwd: &Path, base: &str, head: &str) -> Option<Vec<String>> {
    let range = format!("{}...{}", base, head);
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["diff", "--name-only", &range])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let files: Vec<String> = s
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if files.is_empty() {
        None
    } else {
        Some(files)
    }
}

/// Run `git log -1 --format=<fmt> <ref>` in `cwd`.
/// Returns `None` on any error or empty output.
pub fn log_format(cwd: &Path, format: &str, git_ref: &str) -> Option<String> {
    let fmt = format!("--format={}", format);
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["log", "-1", &fmt, git_ref])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Derive a [`crate::types::ChangeContext`] and optional filtered target from git.
pub fn change_context_from_git(
    cwd: &Path,
    base: &str,
    extensions: &[String],
) -> (crate::types::ChangeContext, Option<String>) {
    let head = "HEAD";
    let title = log_format(cwd, "%s", head);
    let description = log_format(cwd, "%b", head);
    let diff_stat = diff_stat(cwd, base, head);
    let mut target = None;
    if let Some(files) = changed_files(cwd, base, head) {
        let filtered: Vec<String> = files
            .into_iter()
            .filter(|f| extensions.iter().any(|ext| f.ends_with(ext.as_str())))
            .collect();
        if !filtered.is_empty() {
            target = Some(filtered.join("\n"));
        }
    }
    let ctx = crate::types::ChangeContext {
        title,
        description,
        base_ref: Some(base.to_string()),
        head_ref: Some(head.to_string()),
        diff_stat,
    };
    (ctx, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::init_repo_with_commit;
    use std::process::Command as Cmd;
    use tempfile::tempdir;

    fn add_second_commit(dir: &Path) -> bool {
        let run = |args: &[&str]| {
            Cmd::new("git")
                .current_dir(dir)
                .args(args)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        std::fs::write(dir.join("b.rs"), "fn b() {}").is_ok()
            && run(&["add", "."])
            && run(&["commit", "-m", "add b.rs"])
    }

    #[test]
    fn diff_stat_returns_some_in_git_repo() {
        let dir = tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "initial commit") {
            return; // git not available, skip
        }
        if !add_second_commit(dir.path()) {
            return;
        }
        // diff HEAD~1...HEAD should show b.rs
        let result = diff_stat(dir.path(), "HEAD~1", "HEAD");
        assert!(result.is_some(), "expected Some diff_stat");
        let s = result.unwrap();
        assert!(s.contains("b.rs") || s.contains("1 file"), "got: {s}");
    }

    #[test]
    fn diff_stat_returns_none_outside_git_repo() {
        let dir = tempdir().unwrap();
        let result = diff_stat(dir.path(), "main", "HEAD");
        assert!(result.is_none());
    }

    #[test]
    fn log_format_returns_subject_in_git_repo() {
        let dir = tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "initial commit") {
            return;
        }
        let result = log_format(dir.path(), "%s", "HEAD");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "initial commit");
    }

    #[test]
    fn log_format_returns_none_outside_git_repo() {
        let dir = tempdir().unwrap();
        let result = log_format(dir.path(), "%s", "HEAD");
        assert!(result.is_none());
    }

    #[test]
    fn changed_files_returns_some_in_git_repo() {
        let dir = tempdir().unwrap();
        if !init_repo_with_commit(dir.path(), "initial commit") {
            return;
        }
        if !add_second_commit(dir.path()) {
            return;
        }
        let result = changed_files(dir.path(), "HEAD~1", "HEAD");
        assert!(result.is_some());
        assert!(result.unwrap().iter().any(|f| f.contains("b.rs")));
    }

    #[test]
    fn changed_files_returns_none_outside_git_repo() {
        let dir = tempdir().unwrap();
        let result = changed_files(dir.path(), "main", "HEAD");
        assert!(result.is_none());
    }
}
