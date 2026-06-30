import { readFile } from "node:fs/promises";

const expectedTargets = [
  "aarch64-unknown-linux-gnu",
  "x86_64-unknown-linux-gnu",
  "x86_64-pc-windows-msvc",
];

const deferredTargets = [
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "aarch64-pc-windows-msvc",
];

function validate({ verifyWorkflow, releaseWorkflow, packageDocument }) {
  const failures = [];
  const combined = `${verifyWorkflow}\n${releaseWorkflow}`;

  if (!verifyWorkflow.includes("pull_request:")) {
    failures.push("verify workflow must run for pull requests.");
  }
  if (!verifyWorkflow.includes("runs-on: ubuntu-24.04")) {
    failures.push("verify workflow must run on Ubuntu 24.04.");
  }
  for (const runner of ["macos-15", "windows-2022"]) {
    if (verifyWorkflow.includes(runner)) {
      failures.push(`verify workflow must stay Linux-only: ${runner}.`);
    }
  }
  if (!verifyWorkflow.includes("permissions:\n  contents: read")) {
    failures.push(
      "verify workflow must have read-only repository permissions.",
    );
  }
  if (!verifyWorkflow.includes("pnpm verify")) {
    failures.push("verify workflow must run the complete Linux gate.");
  }
  if (verifyWorkflow.includes("pnpm verify:windows")) {
    failures.push("verify workflow must stay Linux-only for the MVP.");
  }
  if (!combined.includes("xdg-utils")) {
    failures.push("Linux workflow prerequisites must include xdg-utils.");
  }

  const actionReferences = [...combined.matchAll(/uses:\s+[^@\s]+@([^\s]+)/g)];
  for (const [, reference] of actionReferences) {
    if (!/^[0-9a-f]{40}$/.test(reference)) {
      failures.push(
        `GitHub Action reference must be a full commit SHA: ${reference}.`,
      );
    }
  }
  if (actionReferences.length < 10) {
    failures.push("release workflows must use the reviewed pinned actions.");
  }
  if (verifyWorkflow.includes("${{ secrets.")) {
    failures.push("verification workflow must not read secrets.");
  }
  if (!releaseWorkflow.includes("- v*")) {
    failures.push("release workflow must trigger only for version tags.");
  }
  if (releaseWorkflow.includes("- burnly-v*")) {
    failures.push("release workflow must not trigger for branded tags.");
  }
  for (const requiredSecret of [
    "TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}",
    "TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}",
  ]) {
    if (!releaseWorkflow.includes(requiredSecret)) {
      failures.push(
        `release workflow is missing signing secret: ${requiredSecret}.`,
      );
    }
  }

  for (const target of expectedTargets) {
    if (!releaseWorkflow.includes(`target: ${target}`)) {
      failures.push(`release build matrix is missing ${target}.`);
    }
  }
  for (const target of deferredTargets) {
    if (releaseWorkflow.includes(`target: ${target}`)) {
      failures.push(
        `release build matrix includes deferred target: ${target}.`,
      );
    }
  }
  for (const requiredBoundary of [
    "attestations: write",
    "id-token: write",
    "retention-days: 14",
    "if-no-files-found: error",
    "pnpm tauri signer sign",
    "pnpm release:stage ${{ matrix.target }}",
    "pnpm linux-smoke:appimage",
    "pnpm windows-smoke:exe",
    "pnpm updater:manifest artifacts",
    "pnpm updater:verify artifacts",
    "latest.json",
    "latest-linux.json",
    "install-linux.sh",
    "pnpm release:verify artifacts",
    "merge-multiple: true",
    "needs:\n      - validate\n      - build",
    "if: github.event_name == 'push' || inputs.publish == true",
    "contents: write",
    "--verify-tag",
    "--notes-file",
  ]) {
    if (!releaseWorkflow.includes(requiredBoundary)) {
      failures.push(
        `release workflow is missing boundary: ${requiredBoundary}.`,
      );
    }
  }
  if (!releaseWorkflow.includes("cancel-in-progress: false")) {
    failures.push(
      "release workflow cancellation must be explicit and disabled.",
    );
  }

  const scripts = packageDocument.scripts ?? {};
  for (const script of [
    "release:version",
    "release:stage",
    "release:verify",
    "updater:manifest",
    "updater:verify",
    "updater-metadata:test",
  ]) {
    if (!scripts[script]) failures.push(`package.json is missing ${script}.`);
  }

  return failures;
}

const inputs = {
  verifyWorkflow: await readFile(".github/workflows/verify.yml", "utf8"),
  releaseWorkflow: await readFile(".github/workflows/release.yml", "utf8"),
  packageDocument: JSON.parse(await readFile("package.json", "utf8")),
};
const failures = validate(inputs);

if (process.argv.includes("--self-test")) {
  const mutated = structuredClone(inputs);
  mutated.verifyWorkflow = mutated.verifyWorkflow.replace(
    /actions\/checkout@[0-9a-f]{40}/,
    "actions/checkout@v5",
  );
  mutated.releaseWorkflow = mutated.releaseWorkflow
    .replace("target: aarch64-unknown-linux-gnu", "target: unsupported-target")
    .replace("needs:\n      - validate\n      - build", "needs: validate");
  if (validate(mutated).length < 3) {
    console.error("Release workflow harness self-test did not catch drift.");
    process.exit(1);
  }
  console.log("Release workflow harness self-test passed.");
  process.exit(0);
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Release workflow policy passed.");
