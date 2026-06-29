import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);
const fixtureDirectory = await mkdtemp(path.join(tmpdir(), "burnly-release-"));
const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);

function artifactName(target, bundle) {
  return releaseTargets.artifactNameTemplate
    .replace("{version}", packageDocument.version)
    .replace("{platform}", target.platform)
    .replace("{architecture}", target.architecture)
    .replace("{extension}", bundle.extension);
}

try {
  for (const target of releaseTargets.targets) {
    const artifacts = [];
    for (const bundle of target.bundles) {
      const fileName = artifactName(target, bundle);
      const contents = Buffer.from(`${target.rustTargetTriple}:${bundle.kind}`);
      await writeFile(path.join(fixtureDirectory, fileName), contents);
      const artifact = {
        kind: bundle.kind,
        fileName,
        bytes: contents.length,
        sha256: createHash("sha256").update(contents).digest("hex"),
      };
      if (target.platform === "linux" && bundle.kind === "appimage") {
        const signatureFileName = `${fileName}.sig`;
        const signature = Buffer.from(`signature:${target.rustTargetTriple}`);
        await writeFile(
          path.join(fixtureDirectory, signatureFileName),
          signature,
        );
        artifact.signature = {
          fileName: signatureFileName,
          bytes: signature.length,
          sha256: createHash("sha256").update(signature).digest("hex"),
        };
      }
      artifacts.push(artifact);
    }
    await writeFile(
      path.join(fixtureDirectory, `manifest-${target.rustTargetTriple}.json`),
      `${JSON.stringify(
        {
          schemaVersion: 1,
          version: packageDocument.version,
          rustTargetTriple: target.rustTargetTriple,
          artifacts,
        },
        null,
        2,
      )}\n`,
    );
  }

  await execute(process.execPath, [
    "scripts/verify-release-artifacts.mjs",
    fixtureDirectory,
  ]);
  const checksums = await readFile(
    path.join(fixtureDirectory, "SHA256SUMS"),
    "utf8",
  );
  const expectedChecksumLines = releaseTargets.targets.reduce(
    (count, target) =>
      count +
      target.bundles.length +
      target.bundles.filter(
        (bundle) => target.platform === "linux" && bundle.kind === "appimage",
      ).length,
    0,
  );
  if (checksums.trim().split("\n").length !== expectedChecksumLines) {
    throw new Error("checksum output is incomplete");
  }

  const tamperedName = artifactName(
    releaseTargets.targets[0],
    releaseTargets.targets[0].bundles[0],
  );
  await writeFile(path.join(fixtureDirectory, tamperedName), "tampered");
  try {
    await execute(process.execPath, [
      "scripts/verify-release-artifacts.mjs",
      fixtureDirectory,
    ]);
    throw new Error("tampered artifact was accepted");
  } catch (error) {
    if (error.message === "tampered artifact was accepted") throw error;
  }

  console.log("Release artifact aggregation and tamper tests passed.");
} finally {
  await rm(fixtureDirectory, { recursive: true, force: true });
}
