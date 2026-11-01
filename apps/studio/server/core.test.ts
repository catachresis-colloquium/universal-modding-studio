// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import { checkedDraft, isProfileKey } from "./core";

const canonical = {
  format: "ums-profile-source/v1",
  profile: { id: "idaptik", version: "1.0.0" },
  contract: {
    id: "trusted",
    metadata_path: "contracts/idaptik/v1/contract.json",
    scenario_path: "contracts/idaptik/v1/fixtures/ghost-lobby-scenario.json"
  },
  package: { actors: [{ id: "billy" }] }
};

describe("studio boundary", () => {
  test("only registered profiles are accepted", () => {
    expect(isProfileKey("idaptik")).toBe(true);
    expect(isProfileKey("chronicles-of-slavia")).toBe(true);
    expect(isProfileKey("../secrets")).toBe(false);
    expect(isProfileKey("__proto__")).toBe(false);
  });

  test("IDApTIK draft changes only the package", () => {
    const result = checkedDraft("idaptik", {
      ...canonical,
      package: { actors: [{ id: "changed" }] }
    }, canonical);
    expect(result.package).toEqual({ actors: [{ id: "changed" }] });
    expect(result.contract).toEqual(canonical.contract);
    expect(canonical.package.actors[0]?.id).toBe("billy");
  });

  test("path traversal in game contract is refused", () => {
    expect(() => checkedDraft("idaptik", {
      ...canonical, contract: { ...canonical.contract, metadata_path: "../../secrets" }
    }, canonical)).toThrow(/contract identity and paths/);
  });

  test("changed identity and unknown top-level fields are refused", () => {
    expect(() => checkedDraft("idaptik", { ...canonical, profile: { id: "other" } }, canonical)).toThrow();
    expect(() => checkedDraft("idaptik", { ...canonical, surprise: true }, canonical)).toThrow();
    expect(() => checkedDraft("idaptik", { ...canonical, package: [] }, canonical)).toThrow();
  });

  test("Slavia remains a reflection/validation fixture", () => {
    const fixture = { format: "ums-profile-fixture/v1", profile: { id: "chronicles-of-slavia", version: "1.0.0" }, entities: [] };
    expect(checkedDraft("chronicles-of-slavia", fixture, fixture)).toEqual(fixture);
    expect(() => checkedDraft("chronicles-of-slavia", { ...fixture, format: "untrusted" }, fixture)).toThrow();
  });
});
