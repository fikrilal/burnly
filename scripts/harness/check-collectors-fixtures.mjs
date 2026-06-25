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

const developmentManifestPath = path.join(
  process.cwd(),
  "src-tauri",
  "sidecars",
  "ccusage",
  "development-manifest.json",
);
const releaseManifestPath = path.join(
  process.cwd(),
  "src-tauri",
  "sidecars",
  "ccusage",
  "release-manifest.json",
);
const developmentManifest = JSON.parse(
  await readFile(developmentManifestPath, "utf8"),
);
const releaseManifest = JSON.parse(await readFile(releaseManifestPath, "utf8"));
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
      "real-shape.json",
      "valid.json",
    ],
  ],
  [
    "codex-session",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "real-shape.json",
      "valid.json",
    ],
  ],
  [
    "opencode-daily",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "real-shape.json",
      "valid.json",
    ],
  ],
  [
    "opencode-session",
    [
      "empty.json",
      "incompatible-envelope.json",
      "invalid-json.json",
      "real-shape.json",
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

validateManifest(developmentManifest, false);
validateManifest(releaseManifest, true);

console.log(
  "Collector manifest, process fixtures, and envelope fixture matrices passed.",
);

function assertEqual(actual, expected, field) {
  if (actual !== expected) {
    throw new Error(`ccusage manifest has an unexpected ${field}`);
  }
}

function validateManifest(manifest, release) {
  assertEqual(manifest.collectorKey, "ccusage", "collector key");
  assertEqual(manifest.expectedVersion, "20.0.14", "expected version");
  assertEqual(
    manifest.sourceRevision,
    "a7726bb9227ef828a8fa06422a08162254a61563",
    "source revision",
  );
  assertEqual(manifest.adapterVersion, 1, "adapter version");

  if (!Array.isArray(manifest.entries) || manifest.entries.length === 0) {
    throw new Error("ccusage manifest must declare at least one target");
  }

  const expectedTargets = new Map([
    [
      "darwin-arm64",
      ["aarch64-apple-darwin", "@ccusage/ccusage-darwin-arm64", "ccusage"],
    ],
    [
      "darwin-x64",
      ["x86_64-apple-darwin", "@ccusage/ccusage-darwin-x64", "ccusage"],
    ],
    [
      "linux-arm64",
      ["aarch64-unknown-linux-gnu", "@ccusage/ccusage-linux-arm64", "ccusage"],
    ],
    [
      "linux-x64",
      ["x86_64-unknown-linux-gnu", "@ccusage/ccusage-linux-x64", "ccusage"],
    ],
    [
      "windows-arm64",
      [
        "aarch64-pc-windows-msvc",
        "@ccusage/ccusage-win32-arm64",
        "ccusage.exe",
      ],
    ],
    [
      "windows-x64",
      ["x86_64-pc-windows-msvc", "@ccusage/ccusage-win32-x64", "ccusage.exe"],
    ],
  ]);
  const releaseChecksums = new Map([
    [
      "darwin-arm64",
      "df44e944327d6fc071c313d59785bb5ad28f1f57b70075bb6c5ef6da957cb3fc",
    ],
    [
      "darwin-x64",
      "97f5de32caab953e3ce789f3b62d45551219a8cc106d871dc93afccdf6bf61bf",
    ],
    [
      "linux-arm64",
      "f1f7e21073f17905a02daa31cc2b117e39fe10aae18454b228dec04a11a5d1de",
    ],
    [
      "linux-x64",
      "dfcd0ea98fc56d71cff77db000d307b011fe218333ac93f7697d242e1f587e35",
    ],
    [
      "windows-arm64",
      "336d4d321517546921a6b242e00b7c9d58d91317f23fed98a334f3c96faec6b1",
    ],
    [
      "windows-x64",
      "e25ee0cf400037fa4bcb4e2b7d267cfd56f1d43487c5bc5c823b6fc3c5c24c5c",
    ],
  ]);
  const targets = new Set();
  for (const entry of manifest.entries) {
    if (targets.has(entry.target)) {
      throw new Error(`ccusage manifest duplicates target ${entry.target}`);
    }
    targets.add(entry.target);
    const expected = expectedTargets.get(entry.target);
    if (!expected) {
      throw new Error(`ccusage manifest has unknown target ${entry.target}`);
    }
    assertEqual(entry.rustTargetTriple, expected[0], "Rust target triple");
    assertEqual(entry.packageName, expected[1], "native package");
    assertEqual(entry.executableName, expected[2], "executable name");
    validateIntegrity(entry.integrity, entry.target);
    if (release && entry.integrity.kind !== "release_sha256") {
      throw new Error(`release target ${entry.target} must be verified`);
    }
    if (
      release &&
      entry.integrity.sha256 !== releaseChecksums.get(entry.target)
    ) {
      throw new Error(`release target ${entry.target} checksum drifted`);
    }
  }
  if (release && targets.size !== expectedTargets.size) {
    throw new Error(
      "ccusage release manifest must declare all supported targets",
    );
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
