import { readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

const [artifactDirectory, baseUrlArgument, outputPathArgument] =
  process.argv.slice(2);
if (!artifactDirectory || !baseUrlArgument) {
  console.error(
    "Usage: pnpm updater:manifest <artifact-directory> <base-url> [output-path]",
  );
  process.exit(1);
}

const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);
const baseUrl = releaseBaseUrl(baseUrlArgument);
const outputPath =
  outputPathArgument ?? path.join(artifactDirectory, "latest-linux.json");

const manifests = await targetManifests(artifactDirectory);
const platforms = {};
for (const target of releaseTargets.targets.filter(
  (candidate) => candidate.platform === "linux",
)) {
  const manifest = manifests.get(target.rustTargetTriple);
  if (!manifest) {
    throw new Error(`missing release manifest for ${target.rustTargetTriple}`);
  }
  const appImage = manifest.artifacts.find(
    (artifact) => artifact.kind === "appimage",
  );
  if (!appImage) {
    throw new Error(`${target.rustTargetTriple} is missing an AppImage`);
  }
  const signatureFileName =
    appImage.signature?.fileName ?? `${appImage.fileName}.sig`;
  const signature = (
    await readFile(path.join(artifactDirectory, signatureFileName), "utf8")
  ).trim();
  if (!signature) {
    throw new Error(`${signatureFileName} is empty`);
  }
  platforms[updaterPlatform(target)] = {
    signature,
    url: new URL(encodeURIComponent(appImage.fileName), baseUrl).toString(),
  };
}

const manifest = {
  version: packageDocument.version,
  notes: "",
  pub_date: process.env.BURNLY_UPDATER_PUB_DATE ?? new Date().toISOString(),
  platforms: sortObject(platforms),
};

await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(outputPath);

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

function updaterPlatform(target) {
  if (target.platform !== "linux") {
    throw new Error(`unsupported updater platform ${target.platform}`);
  }
  return `linux-${target.architecture}`;
}

function sortObject(value) {
  return Object.fromEntries(
    Object.entries(value).sort(([left], [right]) => left.localeCompare(right)),
  );
}
