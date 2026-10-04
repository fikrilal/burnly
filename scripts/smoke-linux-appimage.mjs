import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  readlink,
  rm,
  stat,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);
const selfTest = process.argv.includes("--self-test");
const providedAppImagePath = selfTest ? undefined : process.argv[2];
if (!selfTest && !providedAppImagePath) {
  console.error(
    "Usage: pnpm linux-smoke:appimage <path-to-appimage> | pnpm linux-smoke:appimage:test",
  );
  process.exit(1);
}
const appImagePath = providedAppImagePath
  ? path.resolve(providedAppImagePath)
  : undefined;
const payloadHeader = Buffer.from("BURNLY-CCUSAGE-PAYLOAD-V1\n", "utf8");
const elfMagic = Buffer.from([0x7f, 0x45, 0x4c, 0x46]);
// The AppImageHub catalog test mounts the AppImage on the oldest still-supported
// Ubuntu LTS (22.04, glibc 2.35) and starts the application there. A newer
// symbol version means the application cannot run on the oldest supported
// target, so it blocks the release instead of warning.
const glibcCeiling = "2.35";

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

async function pathMetadata(candidate) {
  try {
    return await stat(candidate);
  } catch {
    return undefined;
  }
}

async function entryMetadata(candidate) {
  try {
    return await lstat(candidate);
  } catch {
    return undefined;
  }
}

async function fileExists(filePath) {
  const metadata = await pathMetadata(filePath);
  return metadata !== undefined && metadata.isFile();
}

async function symlinksBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const symlinks = [];
  for (const entry of entries) {
    const candidate = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) symlinks.push(candidate);
    else if (entry.isDirectory())
      symlinks.push(...(await symlinksBelow(candidate)));
  }
  return symlinks;
}

async function isElf(filePath) {
  const handle = await open(filePath, "r");
  try {
    const header = Buffer.alloc(elfMagic.length);
    const { bytesRead } = await handle.read(header, 0, header.length, 0);
    return bytesRead === header.length && header.equals(elfMagic);
  } finally {
    await handle.close();
  }
}

function compareGlibcVersions(left, right) {
  const leftParts = left.split(".").map(Number);
  const rightParts = right.split(".").map(Number);
  const partCount = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < partCount; index += 1) {
    const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

async function requiredGlibcVersions(filePath) {
  let stdout;
  try {
    ({ stdout } = await command("objdump", ["-T", filePath]));
  } catch (error) {
    throw new Error(
      `could not read the symbol versions of ${filePath}: ${error.message}`,
      { cause: error },
    );
  }
  const versions = new Set();
  for (const match of stdout.matchAll(/GLIBC_(\d+\.\d+(?:\.\d+)?)/g)) {
    versions.add(match[1]);
  }
  return [...versions];
}

async function appImageResourceDirectory(extractDirectory) {
  const tauriResourceDirectory = path.join(
    extractDirectory,
    "usr",
    "lib",
    "burnly",
  );
  const productResourceDirectory = path.join(
    extractDirectory,
    "usr",
    "lib",
    "Burnly",
  );
  const tauriManifest = path.join(
    tauriResourceDirectory,
    "sidecars",
    "ccusage",
    "manifest.json",
  );
  const productManifest = path.join(
    productResourceDirectory,
    "sidecars",
    "ccusage",
    "manifest.json",
  );

  if (await fileExists(tauriManifest)) return tauriResourceDirectory;
  if (await fileExists(productManifest)) return productResourceDirectory;

  throw new Error(
    "AppImage is missing the ccusage sidecar manifest at the runtime resource path.",
  );
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

async function assertSymlinksResolveInsideAppDir(extractDirectory) {
  const symlinks = await symlinksBelow(extractDirectory);
  const appDirPrefix = `${extractDirectory}${path.sep}`;
  for (const symlinkPath of symlinks) {
    const displayPath = relativeTo(extractDirectory, symlinkPath);
    const target = await readlink(symlinkPath);
    if (path.isAbsolute(target)) {
      throw new Error(
        `AppImage symlink ${displayPath} points outside the AppDir: ${target}. ` +
          "An absolute target resolves on the build machine only, so the link dangles wherever the AppImage is mounted. Ship a relative symlink instead.",
      );
    }
    const resolved = path.resolve(path.dirname(symlinkPath), target);
    if (resolved !== extractDirectory && !resolved.startsWith(appDirPrefix)) {
      throw new Error(
        `AppImage symlink ${displayPath} escapes the AppDir: ${target}.`,
      );
    }
    if ((await pathMetadata(resolved)) === undefined) {
      throw new Error(
        `AppImage symlink ${displayPath} is dangling: ${target} does not exist inside the AppDir.`,
      );
    }
  }
  return symlinks.length;
}

async function assertGlibcCeiling(extractDirectory) {
  const requirements = [];
  let highest;
  for (const filePath of await filesBelow(extractDirectory)) {
    if (!(await isElf(filePath))) continue;
    for (const version of await requiredGlibcVersions(filePath)) {
      requirements.push({
        file: relativeTo(extractDirectory, filePath),
        version,
      });
      if (highest === undefined || compareGlibcVersions(version, highest) > 0) {
        highest = version;
      }
    }
  }
  const aboveCeiling = requirements
    .filter(
      (requirement) =>
        compareGlibcVersions(requirement.version, glibcCeiling) > 0,
    )
    .sort((left, right) => compareGlibcVersions(right.version, left.version));
  if (aboveCeiling.length > 0) {
    const offenders = aboveCeiling
      .slice(0, 5)
      .map(
        (requirement) => `  ${requirement.file} (GLIBC_${requirement.version})`,
      )
      .join("\n");
    throw new Error(
      `AppImage requires GLIBC_${aboveCeiling[0].version}, above the GLIBC_${glibcCeiling} that the oldest still-supported Ubuntu LTS provides:\n${offenders}\nBuild the Linux AppImage on an ubuntu-22.04 runner so it runs on the oldest still-supported Ubuntu LTS.`,
    );
  }
  return highest;
}

async function expectFailure(action, expectedText) {
  try {
    await action();
  } catch (error) {
    if (String(error.message).includes(expectedText)) return;
    throw new Error(
      `self-test expected a failure containing "${expectedText}" but observed: ${error.message}`,
      { cause: error },
    );
  }
  throw new Error(
    `self-test expected a failure containing "${expectedText}" but nothing failed.`,
  );
}

async function runSelfTest() {
  const workspace = await mkdtemp(
    path.join(tmpdir(), "burnly-appimage-self-test-"),
  );
  try {
    const appDir = path.join(workspace, "squashfs-root");
    const desktopEntry = path.join(
      appDir,
      "usr",
      "share",
      "applications",
      "Burnly.desktop",
    );
    const icon = path.join(appDir, "Burnly.png");
    await mkdir(path.dirname(desktopEntry), { recursive: true });
    await writeFile(desktopEntry, "[Desktop Entry]\n");
    await writeFile(icon, "icon");

    // A relative symlink that resolves inside the AppDir is the required shape.
    const dirIcon = path.join(appDir, ".DirIcon");
    await symlink("Burnly.png", dirIcon);
    if ((await assertSymlinksResolveInsideAppDir(appDir)) !== 1) {
      throw new Error("self-test did not count the AppDir symlink.");
    }

    // An absolute target is the defect that dangles off the build machine.
    await rm(dirIcon);
    await symlink(icon, dirIcon);
    await expectFailure(
      () => assertSymlinksResolveInsideAppDir(appDir),
      "points outside the AppDir",
    );

    // A relative target that still leaves the AppDir is a defect too.
    await rm(dirIcon);
    await symlink(path.join("..", "..", "outside.png"), dirIcon);
    await expectFailure(
      () => assertSymlinksResolveInsideAppDir(appDir),
      "escapes the AppDir",
    );

    // A relative target that does not exist dangles just the same.
    await rm(dirIcon);
    await symlink("missing.png", dirIcon);
    await expectFailure(
      () => assertSymlinksResolveInsideAppDir(appDir),
      "is dangling",
    );

    for (const [left, right, expected] of [
      ["2.39", "2.35", 1],
      ["2.36", "2.35", 1],
      ["2.35", "2.35", 0],
      ["2.34", "2.35", -1],
      ["2.3.4", "2.35", -1],
      ["3.0", "2.35", 1],
    ]) {
      const observed = Math.sign(compareGlibcVersions(left, right));
      if (observed !== expected) {
        throw new Error(
          `self-test compared GLIBC_${left} against ${right}: expected ${expected}, observed ${observed}.`,
        );
      }
    }

    // ELF detection uses a synthetic header so the self-test does not depend on
    // the host: process.execPath is a PE file on Windows, which also runs
    // `pnpm harness:check` through verify:windows.
    const elfHeaderFile = path.join(workspace, "elf-header.bin");
    await writeFile(elfHeaderFile, elfMagic);
    if (!(await isElf(elfHeaderFile))) {
      throw new Error("self-test did not recognize an ELF header.");
    }
    if (await isElf(desktopEntry)) {
      throw new Error("self-test treated a text file as an ELF file.");
    }
    if ((await assertGlibcCeiling(appDir)) !== undefined) {
      throw new Error(
        "self-test reported a glibc requirement for a directory without ELF files.",
      );
    }
  } finally {
    await rm(workspace, { recursive: true, force: true });
  }
  console.log("Linux AppImage smoke self-test passed.");
}

if (selfTest) {
  await runSelfTest();
  process.exit(0);
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

  const dirIcon = path.join(extractDirectory, ".DirIcon");
  if ((await entryMetadata(dirIcon)) === undefined) {
    throw new Error(
      "AppImage is missing .DirIcon at the AppDir root, which the AppImageHub catalog test requires.",
    );
  }
  const symlinkCount =
    await assertSymlinksResolveInsideAppDir(extractDirectory);
  const glibcRequirement = await assertGlibcCeiling(extractDirectory);

  const resourceDirectory = await appImageResourceDirectory(extractDirectory);
  const sidecarManifestPath = path.join(
    resourceDirectory,
    "sidecars",
    "ccusage",
    "manifest.json",
  );
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
        dirIcon: relativeTo(extractDirectory, dirIcon),
        symlinksChecked: symlinkCount,
        glibcRequired: glibcRequirement ?? "none",
        desktopEntry: relativeTo(extractDirectory, desktopEntry),
        resourceDirectory: relativeTo(extractDirectory, resourceDirectory),
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
