import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);
const providedAppImagePath = process.argv[2];
if (!providedAppImagePath) {
  console.error("Usage: pnpm linux-smoke:appimage <path-to-appimage>");
  process.exit(1);
}
const appImagePath = path.resolve(providedAppImagePath);
const payloadHeader = Buffer.from("BURNLY-CCUSAGE-PAYLOAD-V1\n", "utf8");

const rustTargetByArchitecture = {
  x64: "x86_64-unknown-linux-gnu",
  arm64: "aarch64-unknown-linux-gnu",
};

async function command(commandName, args, options = {}) {
  return execute(commandName, args, {
    maxBuffer: 20 * 1024 * 1024,
    timeout: 30_000,
    ...options,
  });
}

async function filesBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const candidate = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await filesBelow(candidate)));
    else if (entry.isFile()) files.push(candidate);
  }
  return files;
}

function relativeTo(root, filePath) {
  return path.relative(root, filePath).split(path.sep).join("/");
}

async function firstMatchingFile(root, predicate) {
  const files = await filesBelow(root);
  return files.find((filePath) => predicate(relativeTo(root, filePath)));
}

async function executableSidecarVersion(
  executablePath,
  expectedSidecarVersion,
) {
  const { stdout, stderr } = await command(executablePath, ["--version"]);
  const output = `${stdout}\n${stderr}`.trim();
  if (!output.includes(expectedSidecarVersion)) {
    throw new Error(
      `packaged sidecar reported unexpected version: ${output || "<empty>"}`,
    );
  }
  return output;
}

async function materializedPayloadExecutable({
  payloadPath,
  executableName,
  expectedSha256,
  workspace,
}) {
  const payload = await readFile(payloadPath);
  if (!payload.subarray(0, payloadHeader.length).equals(payloadHeader)) {
    throw new Error("Packaged ccusage payload has an invalid header.");
  }
  const executableBytes = payload.subarray(payloadHeader.length);
  const observedSha256 = createHash("sha256")
    .update(executableBytes)
    .digest("hex");
  if (observedSha256 !== expectedSha256) {
    throw new Error(
      "Packaged ccusage payload checksum does not match manifest.",
    );
  }
  const materializedDirectory = path.join(workspace, "materialized-sidecar");
  await mkdir(materializedDirectory);
  const executablePath = path.join(materializedDirectory, executableName);
  await writeFile(executablePath, executableBytes);
  await chmod(executablePath, 0o700);
  return { executablePath, sha256: observedSha256 };
}

const workspace = await mkdtemp(path.join(tmpdir(), "burnly-appimage-smoke-"));
try {
  const appImageMetadata = await stat(appImagePath);
  if ((appImageMetadata.mode & 0o111) === 0) {
    throw new Error("AppImage artifact is not executable.");
  }

  await command(appImagePath, ["--appimage-extract"], {
    cwd: workspace,
    env: {
      ...process.env,
      APPIMAGE_EXTRACT_AND_RUN: "1",
    },
  });

  const extractDirectory = path.join(workspace, "squashfs-root");
  const files = await filesBelow(extractDirectory);
  const relativeFiles = files.map((filePath) =>
    relativeTo(extractDirectory, filePath),
  );

  const desktopEntry = await firstMatchingFile(
    extractDirectory,
    (relativePath) =>
      relativePath.endsWith(".desktop") &&
      path.basename(relativePath).toLowerCase() === "burnly.desktop",
  );
  if (!desktopEntry) {
    throw new Error("AppImage is missing a desktop entry.");
  }
  const desktopSource = await readFile(desktopEntry, "utf8");
  for (const requiredText of [
    "Name=Burnly",
    "Exec=burnly",
    "Icon=burnly",
    "Categories=Development;",
  ]) {
    if (!desktopSource.includes(requiredText)) {
      throw new Error(`desktop entry is missing ${requiredText}.`);
    }
  }

  const appRun = path.join(extractDirectory, "AppRun");
  const appRunMetadata = await stat(appRun);
  if ((appRunMetadata.mode & 0o111) === 0) {
    throw new Error("AppImage AppRun is not executable.");
  }

  const appExecutable = await firstMatchingFile(
    extractDirectory,
    (relativePath) => relativePath === "usr/bin/burnly",
  );
  if (!appExecutable) {
    throw new Error("AppImage is missing the Burnly executable.");
  }

  const sidecarManifestPath = await firstMatchingFile(
    extractDirectory,
    (relativePath) => relativePath.endsWith("sidecars/ccusage/manifest.json"),
  );
  if (!sidecarManifestPath) {
    throw new Error("AppImage is missing the ccusage sidecar manifest.");
  }
  const sidecarManifest = JSON.parse(
    await readFile(sidecarManifestPath, "utf8"),
  );
  const expectedTarget = rustTargetByArchitecture[process.arch];
  if (!expectedTarget) {
    throw new Error(`unsupported host architecture ${process.arch}`);
  }
  const sidecarEntry = sidecarManifest.entries?.find(
    (entry) => entry.rustTargetTriple === expectedTarget,
  );
  if (!sidecarEntry) {
    throw new Error(`sidecar manifest is missing ${expectedTarget}.`);
  }
  const sidecarDirectory = path.dirname(sidecarManifestPath);
  const sidecarExecutable = path.join(
    sidecarDirectory,
    sidecarEntry.executableName,
  );
  const sidecarMetadata = await stat(sidecarExecutable);
  const sidecarHash = createHash("sha256")
    .update(await readFile(sidecarExecutable))
    .digest("hex");
  const directSidecarIsExecutable = (sidecarMetadata.mode & 0o111) !== 0;
  const verifiedSidecar =
    directSidecarIsExecutable && sidecarHash === sidecarEntry.integrity?.sha256
      ? { executablePath: sidecarExecutable, sha256: sidecarHash }
      : await materializedPayloadExecutable({
          payloadPath: `${sidecarExecutable}.payload`,
          executableName: sidecarEntry.executableName,
          expectedSha256: sidecarEntry.integrity?.sha256,
          workspace,
        });

  const icon = relativeFiles.find((relativePath) =>
    relativePath.endsWith("/icons/hicolor/128x128/apps/burnly.png"),
  );
  if (!icon) {
    throw new Error("AppImage is missing the reviewed 128px icon.");
  }

  const sidecarVersion = await executableSidecarVersion(
    verifiedSidecar.executablePath,
    sidecarManifest.expectedVersion,
  );

  console.log(
    JSON.stringify(
      {
        appRun: relativeTo(extractDirectory, appRun),
        appExecutable: relativeTo(extractDirectory, appExecutable),
        desktopEntry: relativeTo(extractDirectory, desktopEntry),
        sidecarManifest: relativeTo(extractDirectory, sidecarManifestPath),
        sidecarExecutable:
          verifiedSidecar.executablePath === sidecarExecutable
            ? relativeTo(extractDirectory, sidecarExecutable)
            : "materialized from ccusage.payload",
        sidecarSha256: verifiedSidecar.sha256,
        sidecarVersion,
      },
      null,
      2,
    ),
  );
  console.log("Linux AppImage smoke passed.");
} finally {
  await rm(workspace, { recursive: true, force: true });
}
