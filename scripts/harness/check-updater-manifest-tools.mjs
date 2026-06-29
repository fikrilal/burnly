import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);
const workspace = await mkdtemp(path.join(tmpdir(), "burnly-updater-"));
const artifactDirectory = path.join(workspace, "artifacts");
const releaseTargets = JSON.parse(
  await readFile("src-tauri/release-targets.json", "utf8"),
);
const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const baseUrl = "https://github.com/burnly/burnly/releases/download/v0.1.0";

try {
  await mkdir(artifactDirectory);
  for (const target of releaseTargets.targets.filter(
    (candidate) => candidate.platform === "linux",
  )) {
    const fileName = `burnly-v${packageDocument.version}-linux-${target.architecture}.AppImage`;
    const signatureFileName = `${fileName}.sig`;
    const contents = Buffer.from(`${target.rustTargetTriple}:appimage`);
    const signature = `signature-${target.architecture}`;
    await writeFile(path.join(artifactDirectory, fileName), contents);
    await writeFile(path.join(artifactDirectory, signatureFileName), signature);
    await writeFile(
      path.join(artifactDirectory, `manifest-${target.rustTargetTriple}.json`),
      `${JSON.stringify(
        {
          schemaVersion: 1,
          version: packageDocument.version,
          rustTargetTriple: target.rustTargetTriple,
          artifacts: [
            {
              kind: "appimage",
              fileName,
              bytes: contents.length,
              sha256: createHash("sha256").update(contents).digest("hex"),
              signature: {
                fileName: signatureFileName,
                bytes: signature.length,
                sha256: createHash("sha256").update(signature).digest("hex"),
              },
            },
          ],
        },
        null,
        2,
      )}\n`,
    );
  }

  const env = {
    ...process.env,
    BURNLY_UPDATER_PUB_DATE: "2026-06-29T00:00:00.000Z",
  };
  const outputPath = path.join(artifactDirectory, "latest-linux.json");
  await execute(
    process.execPath,
    [
      "scripts/generate-updater-manifest.mjs",
      artifactDirectory,
      baseUrl,
      outputPath,
    ],
    { env },
  );
  await execute(process.execPath, [
    "scripts/verify-updater-manifest.mjs",
    artifactDirectory,
    outputPath,
    baseUrl,
  ]);

  const manifest = JSON.parse(await readFile(outputPath, "utf8"));
  if (
    Object.keys(manifest.platforms).join(",") !== "linux-aarch64,linux-x86_64"
  ) {
    throw new Error("updater manifest platforms are incomplete or unsorted");
  }
  if (!manifest.platforms["linux-x86_64"].signature.includes("x86_64")) {
    throw new Error("updater manifest does not inline signature contents");
  }

  const tampered = structuredClone(manifest);
  tampered.platforms["linux-x86_64"].signature = "tampered";
  const tamperedPath = path.join(artifactDirectory, "tampered-latest.json");
  await writeFile(tamperedPath, `${JSON.stringify(tampered, null, 2)}\n`);
  try {
    await execute(process.execPath, [
      "scripts/verify-updater-manifest.mjs",
      artifactDirectory,
      tamperedPath,
      baseUrl,
    ]);
    throw new Error("tampered updater manifest was accepted");
  } catch (error) {
    if (error.message === "tampered updater manifest was accepted") {
      throw error;
    }
  }

  console.log("Updater manifest generation and verification tests passed.");
} finally {
  await rm(workspace, { recursive: true, force: true });
}
