use std::path::Path;
use std::process::Command;

pub fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

pub fn init_repo_with_commit(dir: &Path, message: &str) -> bool {
    if !git_available() {
        return false;
    }
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    run(&["init"])
        && run(&["config", "user.email", "test@test.com"])
        && run(&["config", "user.name", "Test"])
        && std::fs::write(dir.join("a.rs"), "fn a() {}").is_ok()
        && run(&["add", "."])
        && run(&["commit", "-m", message])
}
