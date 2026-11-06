// SPDX-License-Identifier: AGPL-3.0-or-later
use serde_json::json;
use ums_profile_sdk::ProfileRegistry;
#[test]
fn city_profile_compiles_against_explicit_contract_and_rejects_mismatch() {
    let root = std::env::temp_dir().join(format!("ums-cytoville-{}", std::process::id()));
    std::fs::create_dir_all(root.join("contracts/cytoville/v1")).unwrap();
    let contract = json!({"profile":"cytoville","version":1,"packageFormat":"cytoville-package/1","sourceProfileVersion":"1.0.0","requiredCollections":["LOCATIONS"],"requiredDistricts":["home"]});
    let source = json!({"profile":{"id":"cytoville","version":"1.0.0"},"content":{"LOCATIONS":{"home":{"id":"home"}}}});
    std::fs::write(
        root.join("contracts/cytoville/v1/manifest.json"),
        contract.to_string(),
    )
    .unwrap();
    let path = root.join("source.json");
    std::fs::write(&path, source.to_string()).unwrap();
    let registry = ProfileRegistry::with_builtins();
    let profile = registry.get("cytoville").unwrap();
    assert!(
        profile
            .capabilities
            .iter()
            .any(|v| v == "simulation.turn-based")
    );
    assert!(!profile.vocabulary_contains("guard-ranks", "AntiHacker"));
    let compiler = registry.package_compiler("cytoville").unwrap().unwrap();
    let package = compiler.compile(&path, &root).unwrap();
    assert_eq!(package["content"], source["content"]);
    assert_eq!(package["content_sha256"].as_str().unwrap().len(), 64);
    let mut bad = source;
    bad["profile"]["version"] = json!("2.0.0");
    std::fs::write(&path, bad.to_string()).unwrap();
    assert!(compiler.compile(&path, &root).is_err());
    // Only this test's explicit temporary directory is removed.
    std::fs::remove_dir_all(&root).unwrap();
}
