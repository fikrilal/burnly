import { createHash } from "node:crypto";
import {
  copyFile,
  constants,
  mkdir,
  readFile,
  readdir,
  stat,
  writeFile,
  access,
} from "node:fs/promises";
import path from "node:path";

const [targetTriple, ...providedInputPaths] = process.argv.slice(2);
if (!targetTriple) {
  console.error(
    "Usage: pnpm release:stage <rust-target-triple> [bundle-path...]",
  );
  process.exit(1);
}

const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);
const target = releaseTargets.targets.find(
  (candidate) => candidate.rustTargetTriple === targetTriple,
);
if (!target) {
  console.error(`Unsupported release target: ${targetTriple}`);
  process.exit(1);
}
async function filesBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true }).catch(
    () => [],
  );
  const files = [];
  for (const entry of entries) {
    const candidate = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await filesBelow(candidate)));
    else if (entry.isFile()) files.push(candidate);
  }
  return files;
}

const inputPaths =
  providedInputPaths.length > 0
    ? providedInputPaths
    : (
        await filesBelow(
          path.join("src-tauri", "target", targetTriple, "release", "bundle"),
        )
      ).filter((candidate) =>
        target.bundles.some((bundle) => bundleForPath(candidate, bundle)),
      );

if (inputPaths.length !== target.bundles.length) {
  console.error(
    `${targetTriple} requires ${target.bundles.length} bundle path(s), received ${inputPaths.length}.`,
  );
  process.exit(1);
}

function bundleForPath(inputPath, expectedBundle) {
  const normalized = inputPath.toLowerCase();
  const matches = (bundle) => {
    if (bundle.kind === "appimage") return normalized.endsWith(".appimage");
    return normalized.endsWith(`.${bundle.extension.toLowerCase()}`);
  };
  if (expectedBundle) return matches(expectedBundle);
  return target.bundles.find(matches);
}

function artifactName(bundle) {
  return releaseTargets.artifactNameTemplate
    .replace("{version}", packageDocument.version)
    .replace("{platform}", target.platform)
    .replace("{architecture}", target.architecture)
    .replace("{extension}", bundle.extension);
}

const outputDirectory =
  process.env.BURNLY_RELEASE_ARTIFACT_DIR ??
  path.join("src-tauri", "target", "release-artifacts");
await mkdir(outputDirectory, { recursive: true });

const staged = [];
const seenKinds = new Set();
for (const inputPath of inputPaths) {
  const bundle = bundleForPath(inputPath);
  if (!bundle || seenKinds.has(bundle.kind)) {
    console.error(`Unexpected or duplicate bundle path: ${inputPath}`);
    process.exit(1);
  }
  seenKinds.add(bundle.kind);

  const contents = await readFile(inputPath);
  const name = artifactName(bundle);
  const outputPath = path.join(outputDirectory, name);
  await copyFile(inputPath, outputPath);
  const metadata = await stat(outputPath);
  const artifact = {
    kind: bundle.kind,
    fileName: name,
    bytes: metadata.size,
    sha256: createHash("sha256").update(contents).digest("hex"),
  };
  const signaturePath = `${inputPath}.sig`;
  if (await fileExists(signaturePath)) {
    const signatureFileName = `${name}.sig`;
    const signatureOutputPath = path.join(outputDirectory, signatureFileName);
    await copyFile(signaturePath, signatureOutputPath);
    const signatureContents = await readFile(signatureOutputPath);
    const signatureMetadata = await stat(signatureOutputPath);
    artifact.signature = {
      fileName: signatureFileName,
      bytes: signatureMetadata.size,
      sha256: createHash("sha256").update(signatureContents).digest("hex"),
    };
  }
  staged.push(artifact);
  console.log(outputPath);
}

const manifest = {
  schemaVersion: 1,
  version: packageDocument.version,
  rustTargetTriple: targetTriple,
  artifacts: staged.sort((left, right) => left.kind.localeCompare(right.kind)),
};
await writeFile(
  path.join(outputDirectory, `manifest-${targetTriple}.json`),
  `${JSON.stringify(manifest, null, 2)}\n`,
);

async function fileExists(file) {
  try {
    await access(file, constants.R_OK);
    return true;
  } catch {
    return false;
  }
}
