// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Enaction's theory-of-mind conformance corpus, vendored by digest.
//!
//! `vendor/enaction/conformance/tom/v1` is a byte copy of
//! `conformance/tom/v1` from enaction-engine at the commit recorded in
//! `vendor/enaction/conformance/tom/PROVENANCE.json`. The corpus pins the
//! rule of CAC-KERNEL section 9, "Models are compared with minds"
//! (ADR-0028): a model is compared with the modelled bearer's own belief of
//! one order less, over the model's lifetime, and never with world truth.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use support::{check_vendored, copy_tree, read_json, repo_root};

fn corpus() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/v1")
}

fn provenance() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/PROVENANCE.json")
}

#[test]
fn the_vendored_corpus_matches_its_sums_and_pinned_digest() {
    check_vendored(&corpus(), &provenance()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_provenance_names_the_upstream_commit_and_path() {
    let record = read_json(&provenance());
    assert_eq!(
        record["repository"],
        "https://github.com/catachresis-colloquium/enaction-engine"
    );
    assert_eq!(record["path"], "conformance/tom/v1");
    let commit = record["commit"].as_str().expect("commit");
    assert_eq!(commit.len(), 40, "{commit}");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
}

#[test]
fn a_changed_vendored_file_fails_the_check() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vendored-tom");
    let _ = fs::remove_dir_all(&dir);
    copy_tree(&corpus(), &dir);
    let path = dir.join("accepted/false-exposure/expected.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("changed file");
    assert!(
        error.contains("accepted/false-exposure/expected.json"),
        "{error}"
    );
}
