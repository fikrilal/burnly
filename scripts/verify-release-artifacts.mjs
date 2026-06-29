import { createHash } from "node:crypto";
import { readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";

const artifactDirectory = process.argv[2];
if (!artifactDirectory) {
  console.error("Usage: node scripts/verify-release-artifacts.mjs <directory>");
  process.exit(1);
}

const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);
const publishedTargets = releaseTargets.targets.filter(
  (target) => target.platform === "linux",
);
const expectedTargets = new Set(
  publishedTargets.map((target) => target.rustTargetTriple),
);
const entries = await readdir(artifactDirectory, { withFileTypes: true });
const manifestNames = entries
  .filter(
    (entry) =>
      entry.isFile() &&
      entry.name.startsWith("manifest-") &&
      entry.name.endsWith(".json"),
  )
  .map((entry) => entry.name);

if (manifestNames.length !== expectedTargets.size) {
  console.error(
    `Expected ${expectedTargets.size} target manifests, found ${manifestNames.length}.`,
  );
  process.exit(1);
}

const checksumLines = [];
const observedFiles = new Set();
const allowedFiles = new Set(manifestNames);
for (const manifestName of manifestNames) {
  const manifest = JSON.parse(
    await readFile(path.join(artifactDirectory, manifestName), "utf8"),
  );
  if (
    manifest.schemaVersion !== 1 ||
    manifest.version !== packageDocument.version ||
    !expectedTargets.delete(manifest.rustTargetTriple)
  ) {
    console.error(`Invalid or duplicate target manifest: ${manifestName}`);
    process.exit(1);
  }
  const target = releaseTargets.targets.find(
    (candidate) => candidate.rustTargetTriple === manifest.rustTargetTriple,
  );
  const expectedArtifacts = target.bundles.map((bundle) => ({
    kind: bundle.kind,
    fileName: releaseTargets.artifactNameTemplate
      .replace("{version}", packageDocument.version)
      .replace("{platform}", target.platform)
      .replace("{architecture}", target.architecture)
      .replace("{extension}", bundle.extension),
  }));
  const declaredArtifacts = manifest.artifacts.map((artifact) => ({
    kind: artifact.kind,
    fileName: artifact.fileName,
  }));
  if (JSON.stringify(declaredArtifacts) !== JSON.stringify(expectedArtifacts)) {
    console.error(`Unexpected artifact declaration: ${manifestName}`);
    process.exit(1);
  }

  for (const artifact of manifest.artifacts) {
    if (observedFiles.has(artifact.fileName)) {
      console.error(`Duplicate artifact: ${artifact.fileName}`);
      process.exit(1);
    }
    observedFiles.add(artifact.fileName);
    allowedFiles.add(artifact.fileName);
    const artifactPath = path.join(artifactDirectory, artifact.fileName);
    const contents = await readFile(artifactPath);
    const metadata = await stat(artifactPath);
    const sha256 = createHash("sha256").update(contents).digest("hex");
    if (metadata.size !== artifact.bytes || sha256 !== artifact.sha256) {
      console.error(`Artifact integrity mismatch: ${artifact.fileName}`);
      process.exit(1);
    }
    checksumLines.push(`${sha256}  ${artifact.fileName}`);
    if (artifact.signature) {
      allowedFiles.add(artifact.signature.fileName);
      const signaturePath = path.join(
        artifactDirectory,
        artifact.signature.fileName,
      );
      const signatureContents = await readFile(signaturePath);
      const signatureMetadata = await stat(signaturePath);
      const signatureSha256 = createHash("sha256")
        .update(signatureContents)
        .digest("hex");
      if (
        signatureMetadata.size !== artifact.signature.bytes ||
        signatureSha256 !== artifact.signature.sha256
      ) {
        console.error(
          `Artifact signature integrity mismatch: ${artifact.signature.fileName}`,
        );
        process.exit(1);
      }
      checksumLines.push(`${signatureSha256}  ${artifact.signature.fileName}`);
    }
  }
}

if (expectedTargets.size > 0) {
  console.error(`Missing target manifests: ${[...expectedTargets].join(", ")}`);
  process.exit(1);
}

for (const entry of entries) {
  if (
    entry.isFile() &&
    entry.name !== "SHA256SUMS" &&
    !allowedFiles.has(entry.name)
  ) {
    console.error(`Unexpected release file: ${entry.name}`);
    process.exit(1);
  }
}

checksumLines.sort();
await writeFile(
  path.join(artifactDirectory, "SHA256SUMS"),
  `${checksumLines.join("\n")}\n`,
);
console.log(`Verified ${observedFiles.size} release artifacts.`);
