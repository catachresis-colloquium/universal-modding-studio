// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cytoville's data-only package boundary. Runtime rules stay in the game.
use crate::{ProfileError, read_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn compile(source_path: &Path, game_root: &Path) -> Result<Value, ProfileError> {
    let source = read_json(source_path)?;
    let contract = read_json(&game_root.join("contracts/cytoville/v1/manifest.json"))?;
    let invalid = |s: &str| ProfileError::Invalid(vec![s.into()]);
    if contract["profile"] != "cytoville"
        || contract["version"] != 1
        || contract["packageFormat"] != "cytoville-package/1"
        || source.pointer("/profile/id") != contract.get("profile")
        || source.pointer("/profile/version") != contract.get("sourceProfileVersion")
    {
        return Err(invalid(
            "Cytoville source and game contract identity/version mismatch",
        ));
    }
    let content = source
        .get("content")
        .filter(|v| v.is_object())
        .ok_or_else(|| invalid("content must be an object"))?;
    for name in contract["requiredCollections"]
        .as_array()
        .ok_or_else(|| invalid("invalid collection contract"))?
    {
        let key = name
            .as_str()
            .ok_or_else(|| invalid("invalid collection name"))?;
        if !content
            .get(key)
            .is_some_and(|v| v.is_object() || v.is_array())
        {
            return Err(invalid("required content collection is missing"));
        }
    }
    for id in contract["requiredDistricts"]
        .as_array()
        .ok_or_else(|| invalid("invalid district contract"))?
    {
        if content["LOCATIONS"]
            .get(id.as_str().ok_or_else(|| invalid("invalid district id"))?)
            .is_none()
        {
            return Err(invalid("required district is missing"));
        }
    }
    // This compiler checks the package boundary. The authoritative runtime
    // validates every typed rule and reference before accepting the package.
    let bytes = serde_json::to_vec(content).map_err(|e| invalid(&e.to_string()))?;
    Ok(
        json!({"format":contract["packageFormat"],"profile":contract["profile"],"version":contract["version"],"content_sha256":format!("{:x}",Sha256::digest(bytes)),"content":content}),
    )
}
