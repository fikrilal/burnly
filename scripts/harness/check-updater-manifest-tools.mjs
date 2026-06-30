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
  for (const target of updaterTargets()) {
    const bundle = updaterBundle(target);
    const fileName = releaseTargets.artifactNameTemplate
      .replace("{version}", packageDocument.version)
      .replace("{platform}", target.platform)
      .replace("{architecture}", target.architecture)
      .replace("{extension}", bundle.extension);
    const signatureFileName = `${fileName}.sig`;
    const contents = Buffer.from(`${target.rustTargetTriple}:${bundle.kind}`);
    const signature = `signature-${target.platform}-${target.architecture}`;
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
              kind: bundle.kind,
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
  const crossPlatformOutputPath = path.join(artifactDirectory, "latest.json");
  await execute(
    process.execPath,
    [
      "scripts/generate-updater-manifest.mjs",
      artifactDirectory,
      baseUrl,
      crossPlatformOutputPath,
    ],
    { env },
  );
  await execute(process.execPath, [
    "scripts/verify-updater-manifest.mjs",
    artifactDirectory,
    crossPlatformOutputPath,
    baseUrl,
  ]);
  await writeFile(outputPath, await readFile(crossPlatformOutputPath, "utf8"));
  await execute(process.execPath, [
    "scripts/verify-updater-manifest.mjs",
    artifactDirectory,
    outputPath,
    baseUrl,
  ]);

  const manifest = JSON.parse(await readFile(crossPlatformOutputPath, "utf8"));
  if (
    Object.keys(manifest.platforms).join(",") !==
    "darwin-aarch64,darwin-x86_64,linux-aarch64,linux-x86_64,windows-x86_64"
  ) {
    throw new Error("updater manifest platforms are incomplete or unsorted");
  }
  if (!manifest.platforms["linux-x86_64"].signature.includes("x86_64")) {
    throw new Error("updater manifest does not inline signature contents");
  }
  if (
    !manifest.platforms["windows-x86_64"].url.endsWith("windows-x86_64.exe")
  ) {
    throw new Error("updater manifest does not include the Windows exe URL");
  }
  if (
    !manifest.platforms["darwin-aarch64"].url.endsWith(
      "macos-aarch64.app.tar.gz",
    )
  ) {
    throw new Error(
      "updater manifest does not include the macOS app archive URL",
    );
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

  const windowsSignature = path.join(
    artifactDirectory,
    `burnly-v${packageDocument.version}-windows-x86_64.exe.sig`,
  );
  await rm(windowsSignature);
  try {
    await execute(process.execPath, [
      "scripts/generate-updater-manifest.mjs",
      artifactDirectory,
      baseUrl,
      path.join(artifactDirectory, "missing-windows-signature.json"),
    ]);
    throw new Error("missing Windows signature was accepted");
  } catch (error) {
    if (error.message === "missing Windows signature was accepted") {
      throw error;
    }
  }

  console.log("Updater manifest generation and verification tests passed.");
} finally {
  await rm(workspace, { recursive: true, force: true });
}

function updaterTargets() {
  return releaseTargets.targets.filter(
    (target) =>
      target.platform === "linux" ||
      target.platform === "macos" ||
      target.rustTargetTriple === "x86_64-pc-windows-msvc",
  );
}

function updaterBundle(target) {
  if (target.platform === "linux") {
    return target.bundles.find((bundle) => bundle.kind === "appimage");
  }
  if (target.platform === "macos") {
    return target.bundles.find((bundle) => bundle.kind === "app");
  }
  if (target.rustTargetTriple === "x86_64-pc-windows-msvc") {
    return target.bundles.find((bundle) => bundle.kind === "nsis");
  }
  throw new Error(`unsupported updater target ${target.rustTargetTriple}`);
}
