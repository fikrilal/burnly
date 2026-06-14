import { access, readFile } from "node:fs/promises";
import path from "node:path";

const fixturesDir = path.join(
  process.cwd(),
  "tests",
  "fixtures",
  "collectors",
  "ccusage",
);

await access(fixturesDir);

const manifestPath = path.join(
  process.cwd(),
  "src-tauri",
  "sidecars",
  "ccusage",
  "development-manifest.json",
);
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));

assertEqual(manifest.collectorKey, "ccusage", "collector key");
assertEqual(manifest.expectedVersion, "20.0.11", "expected version");
assertEqual(
  manifest.sourceRevision,
  "43836bcec1558fec9da7cb73017928c51443b32b",
  "source revision",
);
assertEqual(manifest.adapterVersion, 1, "adapter version");

if (!Array.isArray(manifest.entries) || manifest.entries.length === 0) {
  throw new Error("ccusage manifest must declare at least one target");
}

const targets = new Set();
for (const entry of manifest.entries) {
  if (targets.has(entry.target)) {
    throw new Error(`ccusage manifest duplicates target ${entry.target}`);
  }
  targets.add(entry.target);
  validateIntegrity(entry.integrity, entry.target);
}

console.log(
  "Collector manifest and fixture checks passed. No collector fixtures exist yet.",
);

function assertEqual(actual, expected, field) {
  if (actual !== expected) {
    throw new Error(`ccusage manifest has an unexpected ${field}`);
  }
}

function validateIntegrity(integrity, target) {
  if (integrity?.kind === "unverified_dev") {
    if ("sha256" in integrity) {
      throw new Error(
        `development target ${target} must not declare a checksum`,
      );
    }
    return;
  }

  if (
    integrity?.kind === "release_sha256" &&
    /^[0-9a-f]{64}$/.test(integrity.sha256)
  ) {
    return;
  }

  throw new Error(`target ${target} has an invalid integrity policy`);
}
