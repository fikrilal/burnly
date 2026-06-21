import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";

const [targetTriple, ...inputPaths] = process.argv.slice(2);
if (!targetTriple || inputPaths.length === 0) {
  console.error(
    "Usage: pnpm release:stage <rust-target-triple> <bundle-path...>",
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
if (inputPaths.length !== target.bundles.length) {
  console.error(
    `${targetTriple} requires ${target.bundles.length} bundle path(s), received ${inputPaths.length}.`,
  );
  process.exit(1);
}

function bundleForPath(inputPath) {
  const normalized = inputPath.toLowerCase();
  return target.bundles.find((bundle) => {
    if (bundle.kind === "appimage") return normalized.endsWith(".appimage");
    return normalized.endsWith(`.${bundle.extension.toLowerCase()}`);
  });
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
  staged.push({
    kind: bundle.kind,
    fileName: name,
    bytes: metadata.size,
    sha256: createHash("sha256").update(contents).digest("hex"),
  });
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
