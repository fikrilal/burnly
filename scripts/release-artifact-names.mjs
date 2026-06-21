import { readFile } from "node:fs/promises";

const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);

const requestedTarget = process.argv[2];
const selectedTargets = requestedTarget
  ? releaseTargets.targets.filter(
      (target) => target.rustTargetTriple === requestedTarget,
    )
  : releaseTargets.targets;

if (requestedTarget && selectedTargets.length === 0) {
  console.error(`Unsupported release target: ${requestedTarget}`);
  process.exit(1);
}

for (const target of selectedTargets) {
  for (const bundle of target.bundles) {
    console.log(
      releaseTargets.artifactNameTemplate
        .replace("{version}", packageDocument.version)
        .replace("{platform}", target.platform)
        .replace("{architecture}", target.architecture)
        .replace("{extension}", bundle.extension),
    );
  }
}
