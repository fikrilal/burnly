// Regenerates the embedded models.dev pricing snapshot used by the Burnly
// cost calculator.
//
// Usage:
//   node scripts/update-cost-pricing.mjs            # regenerate from live API
//   node scripts/update-cost-pricing.mjs --check    # validate embedded snapshot
//
// Fetches https://models.dev/api.json, compacts the pricing-relevant models
// into the per-token format consumed by application/cost, and writes
// src-tauri/src/application/cost/models-dev-pricing.json.
//
// --check is offline: it validates the embedded snapshot parses, is
// non-empty, and carries the expected shape. It deliberately does NOT compare
// against the live API, because models.dev changes continuously and the
// embedded snapshot is a pinned review point, not a mirror.

import { readFile, writeFile } from "node:fs/promises";

const SNAPSHOT_URL = "https://models.dev/api.json";
const SNAPSHOT_PATH = "src-tauri/src/application/cost/models-dev-pricing.json";
const PER_MILLION = 1_000_000;

// Providers whose models are included in the compact snapshot. The bare
// model ids used by OpenCode-style tools (deepseek-v4-flash, mimo-v2.5,
// nemotron-*, ...) are matched independently.
const PROVIDERS_OF_INTEREST = new Set([
  "deepseek",
  "xiaomi",
  "nvidia",
  "opencode",
  "zhipuai",
  "moonshotai",
]);

const checkOnly = process.argv.includes("--check");

if (checkOnly) {
  const embedded = JSON.parse(await readFile(SNAPSHOT_PATH, "utf8"));
  const count = Object.keys(embedded).length;
  if (count === 0) {
    console.error("Embedded pricing snapshot is empty.");
    process.exit(1);
  }
  for (const [modelId, entry] of Object.entries(embedded)) {
    if (
      typeof entry?.i !== "number" ||
      typeof entry?.o !== "number" ||
      !modelId
    ) {
      console.error(
        `Embedded pricing snapshot has an invalid entry: ${modelId}`,
      );
      process.exit(1);
    }
  }
  console.log(`Embedded pricing snapshot is valid (${count} models).`);
  process.exit(0);
}

const response = await fetch(SNAPSHOT_URL);
if (!response.ok) {
  throw new Error(`models.dev fetch failed: ${response.status}`);
}
const api = await response.json();

const compact = {};
for (const [provider, providerData] of Object.entries(api)) {
  const models = providerData?.models;
  if (!models || typeof models !== "object") {
    continue;
  }
  for (const [modelId, model] of Object.entries(models)) {
    const cost = model?.cost;
    if (!cost || typeof cost !== "object") {
      continue;
    }
    const input = cost.input;
    const output = cost.output;
    if (typeof input !== "number" || typeof output !== "number") {
      continue;
    }
    const entry = {
      i: input / PER_MILLION,
      o: output / PER_MILLION,
    };
    if (typeof cost.cache_read === "number") {
      entry.cr = cost.cache_read / PER_MILLION;
    }
    if (typeof cost.cache_write === "number") {
      entry.cw = cost.cache_write / PER_MILLION;
    }
    if (PROVIDERS_OF_INTEREST.has(provider)) {
      compact[modelId] = entry;
    }
  }
}

await writeFile(SNAPSHOT_PATH, `${JSON.stringify(compact, null, 1)}\n`);
console.log(
  `Regenerated ${SNAPSHOT_PATH} (${Object.keys(compact).length} models).`,
);
