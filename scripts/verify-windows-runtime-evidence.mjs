import { readFile } from "node:fs/promises";

const evidencePath = process.argv[2];
if (!evidencePath) {
  console.error(
    "Usage: pnpm windows-runtime:evidence:check <windows-runtime-evidence.json>",
  );
  process.exit(1);
}

const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
const failures = [];

expect(evidence.schemaVersion === 1, "schemaVersion must be 1.");
expect(
  evidence.environment?.id === "windows-x86_64",
  "environment.id must be windows-x86_64.",
);
expect(
  evidence.environment?.os === "windows",
  "environment.os must be windows.",
);
expect(
  evidence.environment?.architecture === "x86_64",
  "environment.architecture must be x86_64.",
);
expectNonEmptyString(evidence.environment?.windowsVersion, "windowsVersion");
expectNonEmptyString(evidence.artifact?.version, "artifact.version");
expectNonEmptyString(evidence.artifact?.installerFileName, "installerFileName");
expect(
  evidence.artifact?.installerFileName?.endsWith("-windows-x86_64.exe") ===
    true,
  "installerFileName must be the canonical Windows x64 exe artifact.",
);
expectNonEmptyString(evidence.artifact?.source, "artifact.source");
expectNonEmptyString(evidence.install?.installPath, "install.installPath");
expectNonEmptyString(evidence.install?.appDataPath, "install.appDataPath");
expectNonEmptyString(evidence.install?.databasePath, "install.databasePath");

for (const check of [
  "firstLaunch",
  "trayPanel",
  "refresh",
  "ccusageSidecar",
  "sqlite",
  "launchAtLogin",
  "manualUpdateCheck",
  "updateInstallRestart",
]) {
  expectPassed(evidence.checks?.[check], `checks.${check}`);
}

expectNonEmptyString(
  evidence.checks?.ccusageSidecar?.observedVersion,
  "checks.ccusageSidecar.observedVersion",
);
expectNonEmptyString(
  evidence.checks?.refresh?.latestImportStatus,
  "checks.refresh.latestImportStatus",
);
expect(
  evidence.checks?.refresh?.latestImportStatus === "success",
  "checks.refresh.latestImportStatus must be success.",
);
expectNonEmptyString(
  evidence.checks?.manualUpdateCheck?.fromVersion,
  "checks.manualUpdateCheck.fromVersion",
);
expectNonEmptyString(
  evidence.checks?.manualUpdateCheck?.detectedVersion,
  "checks.manualUpdateCheck.detectedVersion",
);
expect(
  evidence.checks?.manualUpdateCheck?.fromVersion !==
    evidence.checks?.manualUpdateCheck?.detectedVersion,
  "manual update check must detect a different newer version.",
);
expectNonEmptyString(
  evidence.checks?.updateInstallRestart?.finalVersion,
  "checks.updateInstallRestart.finalVersion",
);
expect(
  evidence.checks?.updateInstallRestart?.finalVersion ===
    evidence.checks?.manualUpdateCheck?.detectedVersion,
  "finalVersion must match detectedVersion.",
);

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Windows runtime evidence passed.");

function expectPassed(value, field) {
  expect(value && typeof value === "object", `${field} must be an object.`);
  expect(value?.status === "passed", `${field}.status must be passed.`);
  expectNonEmptyString(value?.notes, `${field}.notes`);
}

function expectNonEmptyString(value, field) {
  expect(
    typeof value === "string" && value.trim() !== "",
    `${field} is required.`,
  );
}

function expect(condition, message) {
  if (!condition) failures.push(message);
}
