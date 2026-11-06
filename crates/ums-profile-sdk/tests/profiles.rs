// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2025-2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use ums_profile_sdk::{
    PROFILE_API_VERSION, ProfileRegistry, SimulationAdapter, chronicles_of_slavia, idaptik,
    is_profile_id, is_semver,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

#[test]
fn built_in_profiles_register_with_valid_ids_and_versions() {
    let registry = ProfileRegistry::with_builtins();
    let profiles: Vec<_> = registry.iter().collect();
    assert_eq!(profiles.len(), 3);
    for profile in profiles {
        assert_eq!(profile.api_version, PROFILE_API_VERSION);
        assert!(is_profile_id(&profile.profile_id));
        assert!(is_semver(&profile.version));
        assert_eq!(profile.validate(), Ok(()));
    }
}

#[test]
fn duplicate_profile_registration_is_rejected() {
    let mut registry = ProfileRegistry::default();
    registry.register(idaptik()).unwrap();
    let error = registry.register(idaptik()).unwrap_err();
    assert!(error.0.contains("already registered"));
}

#[test]
fn malformed_profile_id_and_version_are_rejected() {
    let mut profile = idaptik().clone();
    profile.profile_id = "IDApTIK Profile".into();
    profile.version = "next".into();
    let errors = profile.validate().unwrap_err();
    assert!(errors.iter().any(|error| error.0.contains("profileId")));
    assert!(errors.iter().any(|error| error.0.contains("semantic")));
}

#[test]
fn idaptik_vocabulary_and_verb_resolution_are_profile_owned() {
    let profile = idaptik();
    assert!(profile.vocabulary_contains("device-kinds", "PhoneSystem"));
    assert!(profile.vocabulary_contains("guard-ranks", "AntiHacker"));
    assert!(profile.edit_verb("add_guard").is_some());
    assert!(profile.edit_verb("stabilise_bridge").is_none());
}

#[test]
fn slavia_vocabulary_and_verb_resolution_are_profile_owned() {
    let profile = chronicles_of_slavia();
    assert!(profile.vocabulary_contains("heroine-influences", "Quickening"));
    assert!(profile.vocabulary_contains("world-disruptions", "RiftCorruption"));
    assert!(profile.edit_verb("stabilise_bridge").is_some());
    assert!(profile.edit_verb("add_guard").is_none());
}

#[test]
fn profile_terms_do_not_leak_across_games() {
    assert!(!chronicles_of_slavia().vocabulary_contains("guard-ranks", "AntiHacker"));
    assert!(!idaptik().vocabulary_contains("world-disruptions", "RiftCorruption"));
}

#[test]
fn idaptik_package_compiler_is_resolved_and_executed_through_the_sdk() {
    let registry = ProfileRegistry::with_builtins();
    let compiler = registry
        .package_compiler("idaptik")
        .expect("IDApTIK compiler declaration is supported")
        .expect("IDApTIK declares a package compiler");
    let workspace_root = root();
    let idaptik_root = std::env::var_os("IDAPTIK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            workspace_root
                .parent()
                .expect("UMS workspace has a parent")
                .join("IDApTIK")
        });
    let package = compiler
        .compile(
            &workspace_root.join("profiles/idaptik/v1/ghost-lobby.ums.json"),
            &idaptik_root,
        )
        .expect("SDK compiler executes against the game-owned contract");

    assert_eq!(package["format"], "idaptik-package/v1");
    assert_eq!(package["scenario"]["scenario_id"], package["scenario_id"]);
    assert_eq!(package["contract"]["version"], "1.0.0");
}

#[test]
fn reflection_only_profile_does_not_fabricate_a_compiler() {
    let registry = ProfileRegistry::with_builtins();
    assert!(
        registry
            .package_compiler("chronicles-of-slavia")
            .expect("Slavia compiler declaration is readable")
            .is_none()
    );
}

#[test]
fn unknown_profile_cannot_resolve_a_compiler() {
    let error = ProfileRegistry::with_builtins()
        .package_compiler("unknown-game")
        .err()
        .expect("unknown profile is rejected");
    assert!(error.0.contains("not registered"));
}

struct DeterministicPreview;

impl SimulationAdapter for DeterministicPreview {
    fn preview(&self, model: &Value, seed: u64) -> Result<Value, String> {
        Ok(json!({"seed": seed, "model": model, "trace": ["accepted"]}))
    }
}

#[test]
fn simulation_adapter_contract_is_deterministic_for_the_same_seed() {
    let adapter = DeterministicPreview;
    let model = json!({"entities": [{"id": "stable"}]});
    assert_eq!(
        adapter.preview(&model, 42).unwrap(),
        adapter.preview(&model, 42).unwrap()
    );
}

#[test]
fn minimal_fixtures_name_their_profile_and_stay_isolated() {
    let idaptik_fixture: Value = serde_json::from_str(include_str!(
        "../../../profiles/idaptik/v1/tests/minimal-level.json"
    ))
    .unwrap();
    let slavia_fixture: Value = serde_json::from_str(include_str!(
        "../../../profiles/slavia/v1/tests/zone-a.json"
    ))
    .unwrap();
    assert_eq!(idaptik_fixture["profile"], "idaptik");
    assert_eq!(slavia_fixture["profile"], "chronicles-of-slavia");
    assert!(idaptik_fixture.to_string().contains("securityTier"));
    assert!(!idaptik_fixture.to_string().contains("Rift"));
    assert!(slavia_fixture.to_string().contains("grove-birds"));
    assert!(!slavia_fixture.to_string().contains("AntiHacker"));
}
