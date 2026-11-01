<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";

interface Named { id: string; label?: string; level?: string; evidence?: string; applies_to?: string[] }
interface Descriptor {
  profileId: string;
  targetGame: { name: string };
  componentTypes: Named[];
  canvasLenses: Named[];
  inspectors: Named[];
  guarantees: Named[];
  capabilities: string[];
  packageCompiler: { id: string } | null;
}
interface Profile {
  id: string;
  mode: "compile" | "validate";
  descriptor: Descriptor;
  initial: Record<string, unknown>;
}
interface Entry { id: string; kind: string; data: Record<string, unknown> }
interface RunResult {
  ok: boolean;
  kind?: "compiled-package" | "fixture-validation";
  message?: string;
  package?: unknown;
}
const lenses = ["World", "Systems", "Actors", "Cognition / Knowledge", "Affect", "Conation / Goals", "Behaviour", "Narrative & Tropes", "Missions", "Presentation", "Testing & Validation"];
const profiles = ref<Profile[]>([]);
const activeId = ref("");
const activeLens = ref("World");
const selectedId = ref<string | null>(null);
const draftText = ref("");
const draftCache: Record<string, string> = {};
const loading = ref(true);
const busy = ref(false);
const bootError = ref("");
const result = ref<RunResult | null>(null);

const active = computed(() => profiles.value.find((p) => p.id === activeId.value));
const source = computed<Record<string, unknown> | null>(() => {
  try {
    const parsed: unknown = JSON.parse(draftText.value);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ?
      parsed as Record<string, unknown> : null;
  } catch {
    return null;
  }
});
const syntaxValid = computed(() => source.value !== null);
const entries = computed<Entry[]>(() => {
  const src = source.value;
  if (!src) return [];
  const raw = Array.isArray(src.entities) ? src.entities :
    (src.package && typeof src.package === "object" && !Array.isArray(src.package) &&
     Array.isArray((src.package as Record<string, unknown>).actors) ?
      (src.package as Record<string, unknown>).actors as unknown[] : []);
  return raw.flatMap((item): Entry[] => {
    if (!item || typeof item !== "object" || Array.isArray(item)) return [];
    const data = item as Record<string, unknown>;
    return typeof data.id === "string" ? [{ id: data.id, kind: String(data.kind ?? data.role ?? "entity"), data }] : [];
  });
});
const selected = computed(() => entries.value.find((e) => e.id === selectedId.value) ?? null);
const lensSupported = computed(() => activeLens.value === "World" || activeLens.value === "Actors");
const canPackage = computed(() => result.value?.ok && result.value?.kind === "compiled-package");

function switchProfile(id: string) {
  if (activeId.value) draftCache[activeId.value] = draftText.value;
  const next = profiles.value.find((p) => p.id === id);
  if (!next) return;
  activeId.value = id;
  draftText.value = draftCache[id] ?? JSON.stringify(next.initial, null, 2);
  selectedId.value = null;
  result.value = null;
}

function resetDraft() {
  if (!active.value) return;
  draftText.value = JSON.stringify(active.value.initial, null, 2);
  result.value = null;
}

function saveFile(filename: string, contents: string) {
  const url = URL.createObjectURL(new Blob([contents], { type: "application/json" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function exportDraft() {
  if (!active.value || !source.value) return;
  saveFile(active.value.id + "-draft.json", JSON.stringify(source.value, null, 2) + "\n");
}

function exportPackage() {
  if (!active.value || !canPackage.value) return;
  saveFile(active.value.id + "-package.json", JSON.stringify(result.value?.package, null, 2) + "\n");
}

async function runCheck() {
  if (!active.value || !source.value || busy.value) return;
  busy.value = true;
  result.value = null;
  try {
    const response = await fetch("/api/run", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-UMS-Studio": "1" },
      body: JSON.stringify({ id: activeId.value, source: source.value })
    });
    const data: RunResult = await response.json();
    result.value = { ...data, ok: response.ok && data.ok, message: data.message ?? (!response.ok ? "The backend refused this draft" : undefined) };
  } catch {
    result.value = { ok: false, message: "Local API unavailable. Start bun run api alongside bun run dev." };
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  try {
    const response = await fetch("/api/bootstrap");
    if (!response.ok) throw new Error("Profile service is not available");
    const data: { profiles: Profile[] } = await response.json();
    profiles.value = data.profiles;
    if (profiles.value.length) switchProfile(profiles.value[0]!.id);
  } catch (error) {
    bootError.value = error instanceof Error ? error.message : "Cannot load profiles";
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div class="studio-shell">
    <header class="topbar">
      <div class="brand">
        <div class="brand-symbol" aria-hidden="true">U<span>·</span></div>
        <div><strong>UNIVERSAL MODDING STUDIO</strong><small>Local authoring workbench · Alpha</small></div>
      </div>
      <div class="topbar-right">
        <span class="chip"><span class="live-dot"></span> Local workspace</span>
        <a href="https://github.com/catachresis-colloquium/universal-modding-studio" target="_blank" rel="noopener noreferrer">Source repository ↗</a>
      </div>
    </header>

    <main v-if="loading" class="empty-screen">Loading profile contracts…</main>
    <main v-else-if="bootError" class="empty-screen">
      <h1>Studio API unavailable</h1><p>{{ bootError }}</p>
      <code>cd apps/studio &amp;&amp; bun run api</code>
      <p>This is a local tool. No remotely hosted validation is implied.</p>
    </main>
    <main v-else class="workspace">
      <aside class="sidebar">
        <section class="sidebar-section">
          <h2>GAME PROFILES</h2>
          <button v-for="p in profiles" :key="p.id" class="profile-choice" :class="{ active: p.id === activeId }" type="button" @click="switchProfile(p.id)">
            <span class="profile-glyph">{{ p.id === "idaptik" ? "◆" : "✦" }}</span>
            <span><strong>{{ p.descriptor.targetGame.name }}</strong><small>{{ p.mode === "compile" ? "Compiler connected to Rust" : "Validation fixture only" }}</small></span>
          </button>
        </section>
        <section class="sidebar-section">
          <h2>WORKSPACE LENSES</h2>
          <nav aria-label="Workspace lenses">
            <button v-for="lens in lenses" :key="lens" class="lens-button" :class="{ active: lens === activeLens }" type="button" :aria-current="lens === activeLens ? 'page' : undefined" @click="activeLens = lens">
              <span class="lens-mark" aria-hidden="true">◈</span>{{ lens }}
            </button>
          </nav>
        </section>
        <section class="sidebar-section palette">
          <h2>PROFILE PALETTE</h2>
          <div v-for="component in active?.descriptor.componentTypes" :key="component.id" class="palette-row">
            <span>{{ component.label }}</span><small>{{ component.id }}</small>
          </div>
        </section>
      </aside>

      <section class="main-column">
        <div class="page-title">
          <div><div class="eyebrow">PROJECT / {{ active?.descriptor.profileId }}</div>
            <h1>{{ activeLens }}</h1>
            <p>One selected identity across views. Game-specific concepts stay in their own profile.</p>
          </div>
          <span class="state-pill">{{ active?.mode === "compile" ? "Executable profile" : "Reflection only" }}</span>
        </div>
        <div class="content-split">
          <section class="canvas-panel" aria-label="Object explorer">
            <div class="panel-heading"><h2>Object explorer</h2><span>{{ entries.length }} entities</span></div>
            <template v-if="lensSupported">
              <p class="note">Structured inventory view · Not a simulated game scene</p>
              <div v-if="!syntaxValid" class="problem">The draft is not valid JSON. Correct it in the source editor below.</div>
              <div v-else-if="!entries.length" class="placeholder">No identifiable entities in this source document.</div>
              <div v-else class="entity-grid">
                <button v-for="entity in entries" :key="entity.id" type="button" class="entity" :class="{ selected: selectedId === entity.id }" @click="selectedId = entity.id">
                  <span class="entity-icon" aria-hidden="true">◇</span>
                  <strong>{{ entity.id }}</strong><small>{{ entity.kind }}</small>
                </button>
              </div>
            </template>
            <div v-else class="placeholder"><span class="placeholder-glyph">◌</span>
              <strong>No {{ activeLens.toLowerCase() }} renderer yet</strong>
              <p>This lens has no implemented projection for the selected source. Selection is preserved; no fictional data is displayed.</p>
            </div>
            <div class="canvas-foot"><span>Profile-provided lenses</span>
              <span v-for="lens in active?.descriptor.canvasLenses" :key="lens.id" class="tag">{{ lens.label }} · specified</span>
            </div>
          </section>

          <aside class="inspector">
            <div class="panel-heading"><h2>Inspector</h2><span>Read-only projection</span></div>
            <template v-if="selected">
              <div class="eyebrow">SELECTED IDENTITY</div><h3>{{ selected.id }}</h3>
              <p class="muted">{{ selected.kind }}</p>
              <dl class="attributes"><template v-for="(value, key) in selected.data" :key="key"><dt>{{ key }}</dt><dd>{{ typeof value === "object" ? JSON.stringify(value) : String(value) }}</dd></template></dl>
            </template>
            <div v-else class="placeholder inspector-empty">Select an entity in the explorer to inspect its source fields.</div>
            <div class="inspector-divider"></div>
            <h2>Profile inspectors</h2>
            <p class="muted small">Declared contributions, not executable panels yet.</p>
            <div v-for="inspector in active?.descriptor.inspectors" :key="inspector.id" class="inspector-item">{{ inspector.label }}<small>{{ inspector.applies_to?.join(", ") }}</small></div>
            <p v-if="!active?.descriptor.inspectors.length" class="muted small">No custom inspectors declared.</p>
          </aside>
        </div>

        <div class="lower-grid">
          <section class="editor-panel">
            <div class="panel-heading"><h2>Authored source · JSON</h2>
              <div class="actions"><button type="button" @click="resetDraft">Reset</button><button type="button" :disabled="!syntaxValid" @click="exportDraft">Export draft</button></div>
            </div>
            <p class="note">Draft changes stay in this browser session. Only the existing Rust profile tools can approve a package.</p>
            <label class="visually-hidden" for="source-editor">JSON source editor</label>
            <textarea id="source-editor" v-model="draftText" spellcheck="false" autocapitalize="off" autocomplete="off" aria-label="JSON source editor" @input="result = null"></textarea>
            <p v-if="!syntaxValid" class="problem" role="status">Invalid JSON: compilation is unavailable.</p>
          </section>
          <section class="evidence-panel">
            <div class="panel-heading"><h2>Validation &amp; evidence</h2><span>Rust authority</span></div>
            <p class="note">Every guarantee is shown at its declared level, not promoted into an end-to-end proof.</p>
            <div v-for="guarantee in active?.descriptor.guarantees" :key="guarantee.id" class="guarantee">
              <span>{{ guarantee.id }}</span><small>{{ guarantee.level }}</small><small class="evidence-source">{{ guarantee.evidence }}</small>
            </div>
            <div class="run-section">
              <button type="button" class="primary-button" :disabled="!syntaxValid || busy" @click="runCheck">
                {{ busy ? "Running Rust checks…" : active?.mode === "compile" ? "Validate & compile package" : "Validate Slavia fixture" }}
              </button>
              <p class="muted small">{{ active?.mode === "compile" ? "Requires IDAPTIK_ROOT and a working Rust toolchain." : "Validates the bounded fixture; does not load a Slavia runtime." }}</p>
              <div v-if="result" class="run-result" :class="{ failure: !result.ok }" role="status">
                <strong>{{ result.ok ? "Accepted" : "Refused" }}</strong>
                <p>{{ result.message ?? (canPackage ? "Package compiled successfully from the draft." : "") }}</p>
                <button v-if="canPackage" type="button" @click="exportPackage">Download compiled package ↓</button>
              </div>
              <p v-else class="muted small">No validation run for this draft.</p>
            </div>
          </section>
        </div>
      </section>
    </main>
    <footer>UMS / profile-owned worlds · No game runtime or Marid server is required to open this local workbench.</footer>
  </div>
</template>
