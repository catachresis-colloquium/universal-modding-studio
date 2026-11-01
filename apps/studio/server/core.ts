// SPDX-License-Identifier: AGPL-3.0-or-later
// The studio's trust boundary; game-specific validation is delegated to Rust.
import { isDeepStrictEqual } from "node:util";

export type ProfileKey = "idaptik" | "chronicles-of-slavia";

export const SOURCES: Record<ProfileKey, { descriptor: string; source: string; mode: "compile" | "validate" }> = {
  idaptik: {
    descriptor: "profiles/idaptik/v1/descriptor.json",
    source: "profiles/idaptik/v1/ghost-lobby.ums.json",
    mode: "compile"
  },
  "chronicles-of-slavia": {
    descriptor: "profiles/slavia/v1/descriptor.json",
    source: "profiles/slavia/v1/fixtures/zone-a-border-path.json",
    mode: "validate"
  }
};

export function isProfileKey(value: unknown): value is ProfileKey {
  return typeof value === "string" && Object.hasOwn(SOURCES, value);
}

function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * Do not accept arbitrary source-defined paths for the game's compiler:
 * its v1 implementation resolves contract paths from its source document.
 * The studio pins that document to the repository's checked-in contract and
 * only allows the authored package to change. A different game/profile must
 * obtain a separately reviewed integration, not an arbitrary CLI or path.
 */
export function checkedDraft(id: ProfileKey, draft: unknown, canonical: unknown): object {
  if (!object(draft) || !object(canonical)) {
    throw new Error("Draft and canonical source must be JSON objects");
  }
  if (draft.format !== canonical.format || !isDeepStrictEqual(draft.profile, canonical.profile)) {
    throw new Error("Source format and profile identity are fixed by the selected profile");
  }
  if (id === "idaptik") {
    const allowed = ["format", "profile", "contract", "package"];
    if (Object.keys(draft).some((key) => !allowed.includes(key))) {
      throw new Error("Unexpected top-level key in IDApTIK source");
    }
    if (!isDeepStrictEqual(draft.contract, canonical.contract)) {
      throw new Error("Game-owned contract identity and paths cannot be edited from the studio");
    }
    if (!object(draft.package)) {
      throw new Error("IDApTIK package must be a JSON object");
    }
    // Rebuild from trusted data instead of forwarding untrusted contract paths.
    return { ...canonical, package: draft.package };
  }
  return draft;
}
