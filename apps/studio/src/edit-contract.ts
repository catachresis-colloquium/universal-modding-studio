// SPDX-License-Identifier: AGPL-3.0-or-later
// Form generation is driven by profile reflection, never by game-specific nouns.
export interface EditField {
  id: string;
  valueType: string;
  required: boolean;
  vocabulary: string | null;
}
export interface EditVerb {
  id: string;
  collection: string;
  summary: string;
  arguments: EditField[];
}
export interface EditDescriptor {
  profileId: string;
  editVerbs: EditVerb[];
  vocabularies: Record<string, string[]>;
}
export type Edit = Record<string, unknown> & { verb: string };

export function buildEdit(
  descriptor: EditDescriptor,
  verbId: string,
  raw: Record<string, string>
): Edit {
  const verb = descriptor.editVerbs.find((item) => item.id === verbId);
  if (!verb) throw new Error("Selected verb is not declared by this profile");
  const edit: Edit = { verb: verb.id };
  for (const arg of verb.arguments) {
    const input = (raw[arg.id] ?? "").trim();
    if (!input) {
      if (arg.required) throw new Error(arg.id + " is required");
      continue;
    }
    const vocabulary = arg.vocabulary ? descriptor.vocabularies[arg.vocabulary] : undefined;
    if (arg.vocabulary && !vocabulary) throw new Error("Missing profile vocabulary: " + arg.vocabulary);
    if (vocabulary && !vocabulary.includes(input)) {
      throw new Error(arg.id + " is not in the declared vocabulary");
    }
    let value: unknown = input;
    if (arg.valueType === "integer" || arg.valueType === "number") {
      value = Number(input);
      if (!Number.isFinite(value) || (arg.valueType === "integer" && !Number.isSafeInteger(value))) {
        throw new Error(arg.id + " must be a valid " + arg.valueType);
      }
    } else if (arg.valueType === "boolean") {
      if (input !== "true" && input !== "false") throw new Error(arg.id + " must be true or false");
      value = input === "true";
    } else if (arg.valueType === "object") {
      try { value = JSON.parse(input); } catch { throw new Error(arg.id + " must be valid JSON"); }
      if (!value || typeof value !== "object" || Array.isArray(value)) {
        throw new Error(arg.id + " must be a JSON object");
      }
    } else if (arg.valueType !== "string") {
      throw new Error("Unsupported field type: " + arg.valueType);
    }
    edit[arg.id] = value;
  }
  return edit;
}

export function validateEditScript(descriptor: EditDescriptor, candidate: unknown): Edit[] {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate) ||
      !("edits" in candidate) || !Array.isArray(candidate.edits) ||
      candidate.edits.length < 1 || candidate.edits.length > 64) {
    throw new Error("Script must have between 1 and 64 edits");
  }
  const result: Edit[] = [];
  for (const candidateEdit of candidate.edits) {
    if (!candidateEdit || typeof candidateEdit !== "object" || Array.isArray(candidateEdit) ||
        typeof candidateEdit.verb !== "string") throw new Error("Invalid edit entry");
    const verb = descriptor.editVerbs.find((v) => v.id === candidateEdit.verb);
    if (!verb) throw new Error("Undeclared edit verb");
    const allowed = new Set(["verb", ...verb.arguments.map((a) => a.id)]);
    if (Object.keys(candidateEdit).some((key) => !allowed.has(key))) {
      throw new Error("Unexpected field in edit " + verb.id);
    }
    const raw: Record<string, string> = {};
    for (const field of verb.arguments) {
      const value: unknown = candidateEdit[field.id];
      if (value === undefined) continue;
      raw[field.id] = typeof value === "string" ? value : JSON.stringify(value);
    }
    result.push(buildEdit(descriptor, verb.id, raw));
  }
  return result;
}
