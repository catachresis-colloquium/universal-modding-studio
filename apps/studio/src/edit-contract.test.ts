// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import { buildEdit, validateEditScript, type EditDescriptor } from "./edit-contract";
import { resolve } from "node:path";

const descriptor: EditDescriptor = {
  profileId: "sample",
  vocabularies: { "roles": ["Sentinel", "Patrol"] },
  editVerbs: [
    { id: "add_guard", collection: "guards", summary: "Guard", arguments: [
      { id: "id", valueType: "string", required: true, vocabulary: null },
      { id: "rank", valueType: "string", required: true, vocabulary: "roles" },
      { id: "tier", valueType: "integer", required: true, vocabulary: null },
      { id: "worldX", valueType: "number", required: false, vocabulary: null },
      { id: "enabled", valueType: "boolean", required: false, vocabulary: null }
    ] },
    { id: "set_data", collection: "data", summary: "Data", arguments: [
      { id: "config", valueType: "object", required: true, vocabulary: null }
    ] }
  ]
};

describe("profile-reflected edit composition", () => {
  test("composes typed vocabulary-backed edits", () => {
    expect(buildEdit(descriptor, "add_guard", { id: "g", rank: "Sentinel", tier: "2", worldX: "4.5", enabled: "false" }))
      .toEqual({ verb: "add_guard", id: "g", rank: "Sentinel", tier: 2, worldX: 4.5, enabled: false });
  });
  test("refuses vocabulary invention", () => {
    expect(() => buildEdit(descriptor, "add_guard", { id: "g", rank: "Wizard", tier: "2" })).toThrow(/vocabulary/);
  });
  test("refuses unknown verbs and missing required values", () => {
    expect(() => buildEdit(descriptor, "invent_magic", {})).toThrow(/not declared/);
    expect(() => buildEdit(descriptor, "add_guard", { rank: "Sentinel", tier: "2" })).toThrow(/required/);
  });
  test("refuses unsafe and fractional integer numbers", () => {
    for (const tier of ["2.5", "NaN", "9007199254740992"]) {
      expect(() => buildEdit(descriptor, "add_guard", { id: "g", rank: "Patrol", tier })).toThrow();
    }
  });
  test("refuses invalid objects and permits valid objects", () => {
    expect(() => buildEdit(descriptor, "set_data", { config: "[]" })).toThrow(/JSON object/);
    expect(() => buildEdit(descriptor, "set_data", { config: "{bad" })).toThrow(/valid JSON/);
    expect(buildEdit(descriptor, "set_data", { config: '{"x":true}' }))
      .toEqual({ verb: "set_data", config: { x: true } });
  });
  test("server admission re-checks the actual declared fields", () => {
    const edits = validateEditScript(descriptor, { edits: [
      { verb: "add_guard", id: "g", rank: "Sentinel", tier: 2 }
    ] });
    expect(edits).toHaveLength(1);
    expect(() => validateEditScript(descriptor, { edits: [{ verb: "add_guard", id: "g", rank: "Sentinel", tier: 2, unknown: "x" }] })).toThrow(/Unexpected/);
    expect(() => validateEditScript(descriptor, { edits: [{ verb: "invent_magic" }] })).toThrow();
  });
  test("empty or oversized scripts cannot invoke Rust", () => {
    expect(() => validateEditScript(descriptor, { edits: [] })).toThrow();
    expect(() => validateEditScript(descriptor, { edits: Array(65).fill({ verb: "add_guard" }) })).toThrow();
  });
});

test("current IDApTIK contract drives field controls without copied vocabulary", async () => {
  const path = resolve(import.meta.dir, "../../../profiles/idaptik/v1/descriptor.json");
  const declared = JSON.parse(await Bun.file(path).text()) as EditDescriptor;
  const edit = buildEdit(declared, "add_guard", { id: "g1", rank: "Sentinel", zone: "lobby" });
  expect(edit).toEqual({ verb: "add_guard", id: "g1", rank: "Sentinel", zone: "lobby" });
  expect(() => buildEdit(declared, "add_guard", { id: "g1", rank: "Wizard", zone: "lobby" })).toThrow(/vocabulary/);
});
