//! Guards on the shipped text, not on the wire.
//!
//! Mirrors `TestNoHardcodedGatewayHosts` in the Go SDK: Paratro is also deployed
//! privately (customers run their own gateway), so the crate ships no environment
//! presets and must not mention a Paratro cloud gateway address anywhere in code,
//! comments or error messages. The cloud endpoints are documented in exactly one
//! place — the table in `README.md` → Gateway base URL — and the CHANGELOG's
//! migration notes may repeat them; every other shipped file uses the
//! `https://<gateway-host>` placeholder.

use std::fs;
use std::path::{Path, PathBuf};

/// Files that may name the Paratro cloud hosts (the README table and the
/// CHANGELOG migration notes), relative to the crate root.
const ALLOWED: &[&str] = &["README.md", "CHANGELOG.md"];

/// Directories that are not part of the shipped crate.
const SKIPPED_DIRS: &[&str] = &[".git", "target", "docs"];

/// Extensions of the shipped text files that are checked.
const CHECKED_EXTENSIONS: &[&str] = &["rs", "md", "toml", "yml", "yaml", "example"];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}")) {
        let path = entry.expect("dir entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if !SKIPPED_DIRS.contains(&name) {
                walk(&path, out);
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| CHECKED_EXTENSIONS.contains(&ext))
        {
            out.push(path);
        }
    }
}

#[test]
fn no_hardcoded_gateway_hosts_outside_the_readme_and_changelog_tables() {
    // Assembled from parts so that a plain grep for the host names over the
    // code finds nothing outside the README / CHANGELOG.
    let cloud = format!("paratro{}", ".com");
    let hosts = [format!("api-sandbox.{cloud}"), format!("api.{cloud}")];

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let this_file = root.join(file!());
    let mut files = Vec::new();
    walk(root, &mut files);
    assert!(
        files.iter().any(|f| f.ends_with("src/config.rs")),
        "walk did not reach src/: {files:?}"
    );

    let mut offenders = Vec::new();
    for path in files {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if path == this_file || ALLOWED.iter().any(|a| rel == Path::new(a)) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue; // not UTF-8 text; nothing to scan
        };
        for (i, line) in text.lines().enumerate() {
            if hosts.iter().any(|h| line.contains(h.as_str())) {
                offenders.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "Paratro cloud gateway hosts belong only in the README/CHANGELOG table; \
         use https://<gateway-host> elsewhere:\n  {}",
        offenders.join("\n  ")
    );
}
