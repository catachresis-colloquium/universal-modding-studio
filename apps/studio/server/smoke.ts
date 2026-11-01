// SPDX-License-Identifier: AGPL-3.0-or-later
// Executable local API smoke test: start "bun run api" in another terminal.
export {};
interface Bootstrap {
  profiles: Array<{ id: string; initial: unknown }>;
}
interface Verdict {
  ok?: boolean;
  kind?: string;
  message?: string;
  report?: string;
  package?: { format?: string };
}
const port = process.env.UMS_STUDIO_API_PORT ?? "4178";
const root = "http://127.0.0.1:" + port;
const initial = await fetch(root + "/api/bootstrap");
if (!initial.ok) throw new Error("Profile bootstrap refused: " + initial.status);
const data = await initial.json() as Bootstrap;
if (data.profiles.length !== 2 || !data.profiles.some(p => p.id === "idaptik") ||
    !data.profiles.some(p => p.id === "chronicles-of-slavia")) {
  throw new Error("Actual profile inventory differs from the expected pair");
}
console.log("Profile bootstrap: 2 registered descriptors");

async function check(path: string, body: unknown, expected: number): Promise<Verdict> {
  const response = await fetch(root + path, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-UMS-Studio": "1",
      "Origin": "http://127.0.0.1:5173"
    },
    body: JSON.stringify(body)
  });
  const verdict = await response.json() as Verdict;
  if (response.status !== expected || verdict.ok !== (expected === 200)) {
    throw new Error(path + ": expected HTTP " + expected + ", got " + response.status + ": " +
      (verdict.message ?? verdict.report ?? "unknown response"));
  }
  return verdict;
}

const accepted = await check("/api/check-edits", {
  id: "idaptik",
  script: { edits: [{ verb: "add_zone", id: "lobby", securityTier: 0, worldXStart: 0, worldXEnd: 40 }] }
}, 200);
if (!accepted.report?.includes('"applied": 1')) {
  throw new Error("Rust reported success without an applied edit");
}
console.log("IDApTIK Rust replay: accepted one valid edit");

await check("/api/check-edits", {
  id: "idaptik",
  script: { edits: [{ verb: "add_guard", id: "g", rank: "Wizard", zone: "lobby" }] }
}, 422);
console.log("Unknown vocabulary: refused");

await check("/api/check-edits", {
  id: "chronicles-of-slavia",
  script: { edits: [{ verb: "declare_mythic_zone", id: "z", receptivity: "Receptive" }] }
}, 422);
console.log("Slavia runtime edit: explicitly unavailable");

const slaviaSource = data.profiles.find(p => p.id === "chronicles-of-slavia")!.initial;
const validated = await check("/api/run", { id: "chronicles-of-slavia", source: slaviaSource }, 200);
if (validated.kind !== "fixture-validation") throw new Error("Slavia reported an invented compiler outcome");
console.log("Slavia fixture: validated by the existing Rust profile CLI");

if (process.env.IDAPTIK_ROOT) {
  const idaptikSource = data.profiles.find(p => p.id === "idaptik")!.initial;
  const compiled = await check("/api/run", { id: "idaptik", source: idaptikSource }, 200);
  if (compiled.kind !== "compiled-package" || compiled.package?.format !== "idaptik-package/v1") {
    throw new Error("IDApTIK package compiler returned an unsupported envelope");
  }
  console.log("IDApTIK package: emitted from the game-owned contract");
}
console.log("UMS local API smoke test passed");
