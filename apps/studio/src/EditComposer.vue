<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { buildEdit, type Edit, type EditDescriptor, type EditField } from "./edit-contract";

const props = defineProps<{ descriptor: EditDescriptor; profileId: string }>();
const fields = reactive<Record<string, string>>({});
const choice = ref("");
const queue = ref<Edit[]>([]);
const message = ref("");
const evidence = ref("");
const running = ref(false);

const verb = computed(() => props.descriptor.editVerbs.find(v => v.id === choice.value));
const executable = computed(() => props.profileId === "idaptik");
const canQueue = computed(() => queue.value.length < 64 && Boolean(verb.value));

function init() {
  choice.value = props.descriptor.editVerbs[0]?.id ?? "";
  queue.value = [];
  message.value = "";
  evidence.value = "";
  clearFields();
}
function clearFields() {
  for (const key of Object.keys(fields)) delete fields[key];
  for (const arg of verb.value?.arguments ?? []) {
    fields[arg.id] = arg.valueType === "boolean" ? "false" : "";
  }
  message.value = "";
}
watch(() => props.descriptor, init, { immediate: true });
watch(choice, clearFields);

function options(arg: EditField): string[] {
  return arg.vocabulary ? props.descriptor.vocabularies[arg.vocabulary] ?? [] : [];
}
function enqueue() {
  try {
    if (!verb.value || !canQueue.value) throw new Error("No available edit operation");
    queue.value.push(buildEdit(props.descriptor, verb.value.id, fields));
    evidence.value = "";
    clearFields();
    message.value = "Edit added to the ordered script; it has not yet passed game validation.";
  } catch (error) {
    message.value = error instanceof Error ? error.message : "Invalid edit";
  }
}
function removeEdit(index: number) {
  queue.value.splice(index, 1);
  evidence.value = "";
}
function exportScript() {
  if (!queue.value.length) return;
  const url = URL.createObjectURL(new Blob([JSON.stringify({ edits: queue.value }, null, 2) + "\n"], { type: "application/json" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = props.profileId + "-edit-script.json";
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
async function validate() {
  if (!executable.value || !queue.value.length || running.value) return;
  running.value = true;
  evidence.value = "";
  try {
    const response = await fetch("/api/check-edits", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-UMS-Studio": "1" },
      body: JSON.stringify({ id: props.profileId, script: { edits: queue.value } })
    });
    const result: { ok?: boolean; message?: string; report?: string } = await response.json();
    evidence.value = (result.ok && response.ok ? "Accepted by Rust replay" : "Refused by Rust replay") +
      (result.message ? "\n" + result.message : "") + (result.report ? "\n" + result.report : "");
  } catch {
    evidence.value = "Local Rust validation unavailable. No acceptance was recorded.";
  } finally {
    running.value = false;
  }
}
</script>

<template>
  <section class="composer-panel" aria-label="Profile-driven edit composer">
    <div class="panel-heading">
      <h2>Profile-driven edit composer</h2>
      <span>Contract: {{ descriptor.profileId }}</span>
    </div>
    <p class="note">Fields and vocabulary choices come from the selected profile descriptor. Every edit remains a draft until its real validator accepts the ordered script.</p>
    <div class="composer-content">
      <label class="form-field">
        <span>Available operation</span>
        <select v-model="choice">
          <option v-for="operation in descriptor.editVerbs" :key="operation.id" :value="operation.id">{{ operation.id }}</option>
        </select>
      </label>
      <p class="note">{{ verb?.summary ?? "This profile declares no edit verbs." }}</p>
      <div class="composer-fields">
        <label v-for="field in verb?.arguments ?? []" :key="field.id" class="form-field">
          <span>{{ field.id }} <small>({{ field.valueType }}{{ field.required ? ", required" : "" }})</small></span>
          <select v-if="field.vocabulary" v-model="fields[field.id]">
            <option value="">Select {{ field.id }}</option>
            <option v-for="value in options(field)" :key="value" :value="value">{{ value }}</option>
          </select>
          <select v-else-if="field.valueType === 'boolean'" v-model="fields[field.id]">
            <option value="false">false</option><option value="true">true</option>
          </select>
          <textarea v-else-if="field.valueType === 'object'" v-model="fields[field.id]" rows="3" placeholder="{ }" spellcheck="false"></textarea>
          <input v-else v-model="fields[field.id]" :type="field.valueType === 'integer' || field.valueType === 'number' ? 'number' : 'text'" :step="field.valueType === 'number' ? 'any' : field.valueType === 'integer' ? '1' : undefined" :placeholder="field.id" />
        </label>
      </div>
      <div class="composer-actions">
        <button type="button" :disabled="!canQueue" @click="enqueue">Add edit to script</button>
        <button type="button" :disabled="!queue.length" @click="exportScript">Export script</button>
      </div>
      <p v-if="message" class="note" role="status">{{ message }}</p>
      <div class="composer-queue">
        <h3>Ordered edits ({{ queue.length }})</h3>
        <p v-if="queue.length === 0" class="muted small">No edits queued.</p>
        <div v-for="(item, index) in queue" :key="index" class="queued-edit">
          <span><strong>{{ index + 1 }}. {{ item.verb }}</strong><small>{{ JSON.stringify(item) }}</small></span>
          <button type="button" :aria-label="'Remove edit ' + (index + 1)" @click="removeEdit(index)">Remove</button>
        </div>
        <button v-if="executable" class="primary-button" type="button" :disabled="!queue.length || running" @click="validate">
          {{ running ? "Checking through Rust…" : "Replay and validate with Rust" }}
        </button>
        <p v-else class="muted small">This profile declares edit vocabulary, but does not have an executable editing engine. Export is draft-only.</p>
        <pre v-if="evidence" class="composer-evidence" role="status">{{ evidence }}</pre>
      </div>
    </div>
  </section>
</template>
