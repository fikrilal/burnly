import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

const [artifactDirectory, manifestPath, baseUrlArgument] =
  process.argv.slice(2);
if (!artifactDirectory || !manifestPath || !baseUrlArgument) {
  console.error(
    "Usage: pnpm updater:verify <artifact-directory> <manifest-path> <base-url>",
  );
  process.exit(1);
}

const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
const baseUrl = releaseBaseUrl(baseUrlArgument);
const targetManifestsByTarget = await targetManifests(artifactDirectory);

const failures = [];
if (manifest.version !== packageDocument.version) {
  failures.push("updater manifest version must match package.json.");
}
if (
  typeof manifest.pub_date !== "string" ||
  Number.isNaN(Date.parse(manifest.pub_date))
) {
  failures.push("updater manifest pub_date must be an ISO timestamp.");
}
if (typeof manifest.notes !== "string") {
  failures.push("updater manifest notes must be a string.");
}

const expectedPlatforms = {};
for (const target of updaterTargets()) {
  const releaseManifest = targetManifestsByTarget.get(target.rustTargetTriple);
  if (!releaseManifest) {
    failures.push(`missing release manifest for ${target.rustTargetTriple}.`);
    continue;
  }
  const updaterArtifact = artifactForUpdater(target, releaseManifest);
  if (!updaterArtifact) {
    failures.push(
      `${target.rustTargetTriple} is missing a ${updaterBundleKind(target)} artifact.`,
    );
    continue;
  }
  const signatureFileName =
    updaterArtifact.signature?.fileName ?? `${updaterArtifact.fileName}.sig`;
  let signature;
  try {
    signature = (
      await readFile(path.join(artifactDirectory, signatureFileName), "utf8")
    ).trim();
  } catch {
    failures.push(`${signatureFileName} is missing.`);
    continue;
  }
  if (!signature) {
    failures.push(`${signatureFileName} is empty.`);
  }
  expectedPlatforms[updaterPlatform(target)] = {
    signature,
    url: new URL(
      encodeURIComponent(updaterArtifact.fileName),
      baseUrl,
    ).toString(),
  };
}

if (
  JSON.stringify(manifest.platforms ?? {}) !==
  JSON.stringify(sortObject(expectedPlatforms))
) {
  failures.push("updater manifest platforms do not match staged artifacts.");
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Updater manifest passed.");

function releaseBaseUrl(value) {
  const url = new URL(value.endsWith("/") ? value : `${value}/`);
  if (url.protocol !== "https:" && url.hostname !== "localhost") {
    throw new Error("updater base URL must use HTTPS");
  }
  return url;
}

async function targetManifests(directory) {
  const result = new Map();
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (
      !entry.isFile() ||
      !entry.name.startsWith("manifest-") ||
      !entry.name.endsWith(".json")
    ) {
      continue;
    }
    const manifest = JSON.parse(
      await readFile(path.join(directory, entry.name), "utf8"),
    );
    result.set(manifest.rustTargetTriple, manifest);
  }
  return result;
}

function sortObject(value) {
  return Object.fromEntries(
    Object.entries(value).sort(([left], [right]) => left.localeCompare(right)),
  );
}

function updaterPlatform(target) {
  if (target.platform === "macos") {
    return `darwin-${target.architecture}`;
  }
  return `${target.platform}-${target.architecture}`;
}

function updaterTargets() {
  return releaseTargets.targets.filter(
    (target) =>
      target.platform === "linux" ||
      target.platform === "macos" ||
      target.rustTargetTriple === "x86_64-pc-windows-msvc",
  );
}

function updaterBundleKind(target) {
  if (target.platform === "linux") return "appimage";
  if (target.platform === "macos") return "app";
  if (target.rustTargetTriple === "x86_64-pc-windows-msvc") return "nsis";
  throw new Error(`unsupported updater target ${target.rustTargetTriple}`);
}

function artifactForUpdater(target, manifest) {
  const kind = updaterBundleKind(target);
  return manifest.artifacts.find((artifact) => artifact.kind === kind);
}
