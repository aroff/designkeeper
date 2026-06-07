use std::path::{Path, PathBuf};
use std::process::Command;

/// Copy a named fixture pack from `tests/fixtures/packs/<name>/` into `dest`.
pub fn copy_fixture_pack(name: &str, dest: &Path) {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/packs")
        .join(name);
    copy_dir_all(&src, dest).unwrap_or_else(|e| panic!("copy_fixture_pack({name}) failed: {e}"));
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

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
