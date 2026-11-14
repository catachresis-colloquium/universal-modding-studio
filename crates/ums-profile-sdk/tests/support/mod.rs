// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Shared helpers for the vendored-document tests.
//!
//! A vendored directory holds an upstream file set and its `SHA256SUMS`,
//! in `sha256sum` format (`<hex>  <path>`), over every other file in the
//! directory, sorted by path in byte order. The sums are regenerated here
//! from the files on disk and compared as a whole, so a changed, missing or
//! extra file fails. The digest of `SHA256SUMS` itself is pinned in the
//! provenance record beside the directory, so editing a file together with
//! its line in `SHA256SUMS` fails too.

#![allow(dead_code)] // Each test crate uses a subset.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(root, &path, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("under root")
                .to_str()
                .expect("UTF-8 path")
                .replace('\\', "/");
            out.push(relative);
        }
    }
}

/// `SHA256SUMS` text regenerated from every file under `dir` except
/// `SHA256SUMS` itself, sorted by path in byte order.
pub fn regenerate_sums(dir: &Path) -> String {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.retain(|path| path != "SHA256SUMS");
    files.sort();
    let mut out = String::new();
    for path in files {
        let bytes = fs::read(dir.join(&path)).expect("vendored file");
        out.push_str(&format!("{}  {path}\n", sha256_hex(&bytes)));
    }
    out
}

/// Check a vendored directory against its `SHA256SUMS` and the digest of
/// that file pinned in `provenance` (a JSON file with `sha256sums_digest`).
pub fn check_vendored(dir: &Path, provenance: &Path) -> Result<(), String> {
    let sums = fs::read(dir.join("SHA256SUMS"))
        .map_err(|error| format!("{}/SHA256SUMS: {error}", dir.display()))?;
    let record: Value = serde_json::from_str(
        &fs::read_to_string(provenance)
            .map_err(|error| format!("{}: {error}", provenance.display()))?,
    )
    .map_err(|error| format!("{}: {error}", provenance.display()))?;
    let pinned = record["sha256sums_digest"]
        .as_str()
        .ok_or("provenance has no sha256sums_digest")?;
    let actual = sha256_hex(&sums);
    if actual != pinned {
        return Err(format!(
            "SHA256SUMS digest {actual} differs from the pinned {pinned}"
        ));
    }
    let regenerated = regenerate_sums(dir);
    let recorded = String::from_utf8(sums).map_err(|error| error.to_string())?;
    if regenerated != recorded {
        let differences: Vec<String> = diff_lines(&recorded, &regenerated);
        return Err(format!(
            "regenerated sums differ from SHA256SUMS:\n{}",
            differences.join("\n")
        ));
    }
    Ok(())
}

fn diff_lines(recorded: &str, regenerated: &str) -> Vec<String> {
    let recorded: std::collections::BTreeSet<&str> = recorded.lines().collect();
    let regenerated: std::collections::BTreeSet<&str> = regenerated.lines().collect();
    let mut out: Vec<String> = recorded
        .difference(&regenerated)
        .map(|line| format!("- {line}"))
        .collect();
    out.extend(
        regenerated
            .difference(&recorded)
            .map(|line| format!("+ {line}")),
    );
    out
}

/// Copy a directory tree (for planted-failure tests on a scratch copy).
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copy");
    for entry in fs::read_dir(from).expect("read dir") {
        let path = entry.expect("entry").path();
        let target = to.join(path.file_name().expect("name"));
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).expect("copy");
        }
    }
}

pub fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
