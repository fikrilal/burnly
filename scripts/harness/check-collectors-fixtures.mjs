import { access, readFile, readdir } from "node:fs/promises";
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
const processFixtures = ["fake-collector.sh", "fake-collector-old.sh"];

for (const fixture of processFixtures) {
  const content = await readFile(
    path.join(fixturesDir, "process", fixture),
    "utf8",
  );
  if (!content.startsWith("#!/bin/sh\n")) {
    throw new Error(
      `collector process fixture ${fixture} has an invalid header`,
    );
  }
}

const envelopeMatrices = new Map([
  [
    "claude-daily",
    [
      "additive-fields.json",
      "empty.json",
      "incompatible-envelope.json",
      "invalid-date.json",
      "invalid-json.json",
      "invalid-number.json",
      "valid.json",
    ],
  ],
  ["claude-session", ["valid.json"]],
  [
    "codex-daily",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "valid.json",
    ],
  ],
  [
    "codex-session",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "valid.json",
    ],
  ],
  [
    "opencode-daily",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "valid.json",
    ],
  ],
  [
    "opencode-session",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "valid.json",
    ],
  ],
]);

for (const [directoryName, expectedFixtures] of envelopeMatrices) {
  const envelopeDirectory = path.join(fixturesDir, directoryName);
  const actualEnvelopeFixtures = (await readdir(envelopeDirectory)).sort();

  if (
    JSON.stringify(actualEnvelopeFixtures) !== JSON.stringify(expectedFixtures)
  ) {
    throw new Error(
      `${directoryName} fixture matrix does not match the reviewed set`,
    );
  }

  for (const fixture of actualEnvelopeFixtures) {
    const content = await readFile(
      path.join(envelopeDirectory, fixture),
      "utf8",
    );
    const fixtureName = `${directoryName}/${fixture}`;
    if (fixture === "invalid-json.json") {
      assertInvalidJson(content, fixtureName);
      continue;
    }
    const envelope = JSON.parse(content);
    assertSanitized(envelope, fixtureName);
  }
}

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
  "Collector manifest, process fixtures, and envelope fixture matrices passed.",
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

function assertInvalidJson(content, fixture) {
  try {
    JSON.parse(content);
  } catch {
    return;
  }
  throw new Error(`collector fixture ${fixture} must remain invalid JSON`);
}

function assertSanitized(value, fixture) {
  const forbiddenKeys = new Set([
    "prompt",
    "projectPath",
    "requestId",
    "transcriptPath",
  ]);
  visit(value, (key, entry) => {
    if (
      key === "sessionId" &&
      (typeof entry !== "string" || !/^session-[0-9]+$/.test(entry))
    ) {
      throw new Error(
        `collector fixture ${fixture} contains a non-synthetic sessionId`,
      );
    }
    if (forbiddenKeys.has(key)) {
      throw new Error(
        `collector fixture ${fixture} contains sensitive key ${key}`,
      );
    }
    if (
      typeof entry === "string" &&
      /(?:\/home\/|\/Users\/|[A-Z]:\\)/.test(entry)
    ) {
      throw new Error(`collector fixture ${fixture} contains a local path`);
    }
  });
}

function visit(value, inspect) {
  if (Array.isArray(value)) {
    for (const entry of value) visit(entry, inspect);
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, entry] of Object.entries(value)) {
    inspect(key, entry);
    visit(entry, inspect);
  }
}
