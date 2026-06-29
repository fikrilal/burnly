import { readFile } from "node:fs/promises";

const expectedEnvironmentIds = [
  "windows-x86_64",
  "windows-aarch64",
  "macos-aarch64",
  "macos-x86_64",
  "linux-gnome-x86_64",
  "linux-gnome-aarch64",
  "linux-kde-x86_64",
];
const requiredEvidence = [
  "first_launch",
  "packaged_sidecar_version",
  "refresh",
  "tray",
  "close_reopen",
  "export_dialog",
  "reveal_logs",
  "notifications",
  "recovery",
];
const supportedArtifacts = new Set([
  "release-windows-x86_64",
  "release-windows-aarch64",
  "release-macos-aarch64",
  "release-macos-x86_64",
  "release-linux-x86_64",
  "release-linux-aarch64",
]);
const expectedChunks = new Map([
  ["windows-x86_64", "10D-Windows"],
  ["windows-aarch64", "10D-Windows"],
  ["macos-aarch64", "10D-macOS"],
  ["macos-x86_64", "10D-macOS"],
  ["linux-gnome-x86_64", "10D-Linux"],
  ["linux-gnome-aarch64", "10D-Linux"],
  ["linux-kde-x86_64", "10D-Linux"],
]);

function validate({ matrix, guide, packageDocument }) {
  const failures = [];
  if (matrix.schemaVersion !== 1) {
    failures.push("platform behavior matrix schemaVersion must be 1.");
  }
  if (matrix.artifactBaselineRun !== "28090081218") {
    failures.push(
      "platform behavior matrix must name the successful release dry-run baseline.",
    );
  }

  const actualIds = new Set(
    (matrix.environments ?? []).map((environment) => environment.id),
  );
  for (const id of expectedEnvironmentIds) {
    if (!actualIds.has(id)) {
      failures.push(`platform behavior matrix is missing ${id}.`);
    }
  }
  if (actualIds.size !== expectedEnvironmentIds.length) {
    failures.push("platform behavior matrix has unexpected environments.");
  }

  for (const evidence of requiredEvidence) {
    if (!matrix.requiredEvidence?.includes(evidence)) {
      failures.push(
        `platform behavior matrix is missing ${evidence} evidence.`,
      );
    }
  }

  const desktops = new Set();
  for (const environment of matrix.environments ?? []) {
    desktops.add(environment.desktop);
    if (!supportedArtifacts.has(environment.artifact)) {
      failures.push(
        `${environment.id}: unsupported artifact ${environment.artifact}.`,
      );
    }
    if (environment.chunk !== expectedChunks.get(environment.id)) {
      failures.push(`${environment.id}: incorrect platform chunk.`);
    }
    if (
      !["native_installed_smoke", "manual_installed_smoke"].includes(
        environment.evidenceMode,
      )
    ) {
      failures.push(`${environment.id}: unsupported evidence mode.`);
    }
    if (environment.expectedCapabilities?.updates !== "unavailable") {
      failures.push(
        `${environment.id}: updates must remain unavailable in Phase 10D.`,
      );
    }
    if (environment.expectedCapabilities?.launchAtLogin !== "available") {
      failures.push(
        `${environment.id}: launch at login must be available in packaged builds.`,
      );
    }
  }
  for (const desktop of ["gnome", "kde"]) {
    if (!desktops.has(desktop)) {
      failures.push(`platform behavior matrix must include Linux ${desktop}.`);
    }
  }

  for (const requiredText of [
    "platform-behavior-matrix.json",
    "release workflow dry-run `28090081218`",
    "Linux is validated first",
    "Linux tray support is host-dependent",
    "Launch at login is available in packaged builds",
  ]) {
    if (!guide.includes(requiredText)) {
      failures.push(
        `cross-platform behavior guide is missing: ${requiredText}.`,
      );
    }
  }
  if (!packageDocument.scripts?.["linux-smoke:deb"]) {
    failures.push("package.json is missing linux-smoke:deb.");
  }
  return failures;
}

const inputs = {
  matrix: JSON.parse(
    await readFile("docs/engineering/platform-behavior-matrix.json", "utf8"),
  ),
  guide: await readFile("docs/engineering/cross-platform-behavior.md", "utf8"),
  packageDocument: JSON.parse(await readFile("package.json", "utf8")),
};
const failures = validate(inputs);

if (process.argv.includes("--self-test")) {
  const mutated = structuredClone(inputs);
  mutated.matrix.environments = mutated.matrix.environments.filter(
    (environment) => environment.id !== "linux-kde-x86_64",
  );
  mutated.matrix.requiredEvidence = mutated.matrix.requiredEvidence.filter(
    (evidence) => evidence !== "tray",
  );
  mutated.matrix.environments[0].expectedCapabilities.updates = "available";
  if (validate(mutated).length < 3) {
    console.error("Platform behavior harness self-test did not catch drift.");
    process.exit(1);
  }
  console.log("Platform behavior harness self-test passed.");
  process.exit(0);
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Platform behavior matrix passed.");
