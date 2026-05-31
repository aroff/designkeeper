use std::path::{Path, PathBuf};
use std::process::Command;

/// Copy the default template pack from the repo's `templates/default/` into `dest`.
/// Use in tests that need a real pack on disk.
pub fn copy_default_pack(dest: &Path) {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/default");
    copy_dir_all(&src, dest).expect("copy_default_pack failed");
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
