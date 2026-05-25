//! Default-target file discovery using `ignore` (respects `.gitignore`) and
//! `globset` (extension filtering + extra ignore patterns).

use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::config::ScanConfig;

/// Walk `working_dir`, returning repo-relative paths of files whose extension
/// matches `scan.extensions` and which are not excluded by `scan.ignore_patterns`.
/// Respects `.gitignore` via the `ignore` crate defaults.
pub fn discover_paths(
    scan: &ScanConfig,
    working_dir: &Path,
) -> Result<Vec<String>, std::io::Error> {
    let ext_set = build_extension_globset(&scan.extensions)?;
    let ignore_set = build_ignore_globset(&scan.ignore_patterns)?;

    let mut results = Vec::new();
    // `require_git(false)` makes `.gitignore` files apply even when the tree is
    // not inside a git repository. Hidden files/dirs (incl. `.git`) are skipped
    // by the default `hidden(true)`.
    let walker = WalkBuilder::new(working_dir).require_git(false).build();
    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(working_dir).unwrap_or(path);
        let rel_str = rel.to_string_lossy();

        if !ignore_set.is_empty() && ignore_set.is_match(rel.as_os_str()) {
            continue;
        }
        if !ext_set.is_match(rel.as_os_str()) {
            continue;
        }
        results.push(rel_str.replace('\\', "/"));
    }
    results.sort();
    Ok(results)
}

/// Build a globset matching any of the configured extensions. Extensions are
/// stored with a leading dot (e.g. `.rs`); the glob matches `**/*.rs`.
fn build_extension_globset(extensions: &[String]) -> Result<GlobSet, std::io::Error> {
    let mut builder = GlobSetBuilder::new();
    for ext in extensions {
        let bare = ext.trim_start_matches('.');
        if bare.is_empty() {
            continue;
        }
        let glob = Glob::new(&format!("**/*.{bare}")).map_err(to_io_err)?;
        builder.add(glob);
    }
    builder.build().map_err(to_io_err)
}

/// Build a globset for the user-supplied ignore patterns. A trailing-slash
/// pattern like `vendor/` is treated as "anything under vendor/".
fn build_ignore_globset(patterns: &[String]) -> Result<GlobSet, std::io::Error> {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let pat = pat.trim();
        if pat.is_empty() {
            continue;
        }
        if let Some(dir) = pat.strip_suffix('/') {
            builder.add(Glob::new(&format!("{dir}/**")).map_err(to_io_err)?);
            builder.add(Glob::new(&format!("**/{dir}/**")).map_err(to_io_err)?);
        } else {
            builder.add(Glob::new(pat).map_err(to_io_err)?);
            builder.add(Glob::new(&format!("**/{pat}")).map_err(to_io_err)?);
        }
    }
    builder.build().map_err(to_io_err)
}

fn to_io_err(e: globset::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn scan(exts: &[&str], ignore: &[&str]) -> ScanConfig {
        ScanConfig {
            extensions: exts.iter().map(|s| s.to_string()).collect(),
            ignore_patterns: ignore.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn discovers_matching_extensions() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("b.py"), "print(1)").unwrap();
        fs::write(dir.path().join("c.txt"), "ignore me").unwrap();
        let found = discover_paths(&scan(&[".rs", ".py"], &[]), dir.path()).unwrap();
        assert_eq!(found, vec!["a.rs", "b.py"]);
    }

    #[test]
    fn respects_gitignore() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(dir.path().join("kept.rs"), "fn a() {}").unwrap();
        fs::write(dir.path().join("ignored.rs"), "fn b() {}").unwrap();
        let found = discover_paths(&scan(&[".rs"], &[]), dir.path()).unwrap();
        assert_eq!(found, vec!["kept.rs"]);
    }

    #[test]
    fn applies_ignore_patterns() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("vendor")).unwrap();
        fs::write(dir.path().join("vendor").join("dep.rs"), "fn v() {}").unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        let found = discover_paths(&scan(&[".rs"], &["vendor/"]), dir.path()).unwrap();
        assert_eq!(found, vec!["main.rs"]);
    }

    #[test]
    fn empty_dir_returns_empty() {
        let dir = tempdir().unwrap();
        let found = discover_paths(&scan(&[".rs"], &[]), dir.path()).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn nested_paths_are_relative() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src").join("util")).unwrap();
        fs::write(
            dir.path().join("src").join("util").join("x.rs"),
            "fn x() {}",
        )
        .unwrap();
        let found = discover_paths(&scan(&[".rs"], &[]), dir.path()).unwrap();
        assert_eq!(found, vec!["src/util/x.rs"]);
    }
}
