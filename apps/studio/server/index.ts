// SPDX-License-Identifier: AGPL-3.0-or-later
// Local-only bridge to UMS's own Rust CLIs. No shell, plugins or arbitrary paths.
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { checkedDraft, isProfileKey, SOURCES, type ProfileKey } from "./core";
import { validateEditScript, type EditDescriptor } from "../src/edit-contract";

const root = resolve(import.meta.dir, "../../..");
const port = Number(process.env.UMS_STUDIO_API_PORT ?? "4178");
const host = "127.0.0.1";
const MAX_BYTES = 256 * 1024;
const ALLOWED_ORIGINS = new Set([
  "http://127.0.0.1:5173",
  "http://localhost:5173",
  "http://127.0.0.1:" + port
]);

const json = (data: unknown, status = 200): Response =>
  Response.json(data, {
    status,
    headers: { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" }
  });

async function fileJSON(path: string): Promise<unknown> {
  return JSON.parse(await readFile(resolve(root, path), "utf8"));
}

function errorText(text: string): string {
  return text.slice(0, 8192);
}

async function runRust(argv: string[]): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const child = Bun.spawn(["cargo", "run", "-q", ...argv], {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
    env: process.env
  });
  const watchdog = setTimeout(() => child.kill(), 180_000);
  try {
    const [code, stdout, stderr] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text()
    ]);
    return { ok: code === 0, stdout: errorText(stdout), stderr: errorText(stderr) };
  } finally {
    clearTimeout(watchdog);
  }
}

async function invoke(id: ProfileKey, source: unknown): Promise<Response> {
  const spec = SOURCES[id];
  const canonical = await fileJSON(spec.source);
  const checked = checkedDraft(id, source, canonical);
  const temp = await mkdtemp(join(tmpdir(), "ums-studio-"));
  const sourcePath = join(temp, "source.json");
  const outputPath = join(temp, "package.json");
  try {
    await writeFile(sourcePath, JSON.stringify(checked, null, 2) + "\n", { mode: 0o600 });
    if (id === "idaptik") {
      const gameRoot = process.env.IDAPTIK_ROOT;
      if (!gameRoot) {
        return json({ ok: false, message: "IDAPTIK_ROOT is not set. Compilation requires the game-owned contracts." }, 409);
      }
      const result = await runRust([
        "-p", "ums-profile-sdk", "--bin", "ums-profile", "--",
        "compile-idaptik", "--source", sourcePath,
        "--idaptik-root", resolve(gameRoot), "--output", outputPath
      ]);
      if (!result.ok) {
        return json({ ok: false, message: result.stderr || result.stdout || "Compiler refused the draft" }, 422);
      }
      return json({ ok: true, kind: "compiled-package", package: JSON.parse(await readFile(outputPath, "utf8")) });
    }
    const result = await runRust([
      "-p", "ums-profile-sdk", "--bin", "ums-profile", "--",
      "validate", "--profile", resolve(root, "profiles/slavia/v1/profile.json"),
      "--fixture", sourcePath
    ]);
    if (!result.ok) {
      return json({ ok: false, message: result.stderr || result.stdout || "Profile validation refused the draft" }, 422);
    }
    return json({ ok: true, kind: "fixture-validation", message: "Slavia fixture satisfies the declared profile checks. No runtime compilation or simulation was performed." });
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

async function invokeEdits(id: ProfileKey, script: unknown): Promise<Response> {
  if (id !== "idaptik") {
    return json({ ok: false, message: "This profile has no executable edit replay engine" }, 422);
  }
  const descriptor = await fileJSON(SOURCES[id].descriptor) as EditDescriptor;
  const edits = validateEditScript(descriptor, script);
  const temp = await mkdtemp(join(tmpdir(), "ums-edit-"));
  const path = join(temp, "edits.json");
  try {
    await writeFile(path, JSON.stringify({ edits }, null, 2) + "\n", { mode: 0o600 });
    const result = await runRust(["-p", "ums-ai-edit", "--", "check", path]);
    return json({
      ok: result.ok,
      kind: "edit-validation",
      message: result.ok ? "Accepted by the existing IDApTIK edit replay engine" :
        (result.stderr || "Edit replay refused the draft"),
      report: result.stdout
    }, result.ok ? 200 : 422);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

Bun.serve({
  hostname: host,
  port,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/api/bootstrap") {
      try {
        const profiles = await Promise.all(
          (Object.keys(SOURCES) as ProfileKey[]).map(async (id) => ({
            id,
            mode: SOURCES[id].mode,
            descriptor: await fileJSON(SOURCES[id].descriptor),
            initial: await fileJSON(SOURCES[id].source)
          }))
        );
        return json({ profiles });
      } catch {
        return json({ error: "Unable to read checked-in profile sources" }, 500);
      }
    }
    if (request.method === "POST" && (url.pathname === "/api/run" || url.pathname === "/api/check-edits")) {
      const origin = request.headers.get("origin");
      if (origin && !ALLOWED_ORIGINS.has(origin)) return json({ error: "Origin not allowed" }, 403);
      if (request.headers.get("x-ums-studio") !== "1" ||
          !request.headers.get("content-type")?.startsWith("application/json")) {
        return json({ error: "Expected a local studio JSON request" }, 415);
      }
      const length = Number(request.headers.get("content-length") ?? 0);
      if (!Number.isFinite(length) || length > MAX_BYTES) return json({ error: "Draft too large" }, 413);
      try {
        const body = await request.text();
        if (Buffer.byteLength(body, "utf8") > MAX_BYTES) return json({ error: "Draft too large" }, 413);
        const payload: unknown = JSON.parse(body);
        if (!payload || typeof payload !== "object" || !("id" in payload) ||
            !isProfileKey(payload.id)) {
          return json({ error: "Unknown profile or missing source" }, 400);
        }
        if (url.pathname === "/api/check-edits") {
          if (!("script" in payload)) return json({ error: "Missing edit script" }, 400);
          return await invokeEdits(payload.id, payload.script);
        }
        if (!("source" in payload)) return json({ error: "Missing profile source" }, 400);
        return await invoke(payload.id, payload.source);
      } catch (error) {
        return json({ ok: false, message: error instanceof Error ? error.message : "Invalid request" }, 422);
      }
    }
    return json({ error: "Unknown endpoint" }, 404);
  }
});

console.log("UMS Studio local API: http://" + host + ":" + port);
