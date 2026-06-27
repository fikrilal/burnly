import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);
const debPath = process.argv[2];
if (!debPath) {
  console.error("Usage: pnpm linux-smoke:deb <path-to-deb>");
  process.exit(1);
}

const expectedVersion = JSON.parse(
  await readFile("package.json", "utf8"),
).version;
const expectedPackageFields = {
  Package: "burnly",
  Version: expectedVersion,
  Maintainer: "Burnly contributors",
};
const hostArchitectureByDebArchitecture = {
  amd64: "x64",
  arm64: "arm64",
};
const rustTargetByDebArchitecture = {
  amd64: "x86_64-unknown-linux-gnu",
  arm64: "aarch64-unknown-linux-gnu",
};

async function command(commandName, args) {
  return execute(commandName, args, {
    maxBuffer: 10 * 1024 * 1024,
    timeout: 15_000,
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

const workspace = await mkdtemp(path.join(tmpdir(), "burnly-linux-smoke-"));
try {
  const fieldNames = [
    ...Object.keys(expectedPackageFields),
    "Architecture",
    "Description",
  ];
  const packageFields = {};
  for (const fieldName of fieldNames) {
    const { stdout } = await command("dpkg-deb", ["-f", debPath, fieldName]);
    packageFields[fieldName] = stdout.trim();
  }
  for (const [fieldName, expected] of Object.entries(expectedPackageFields)) {
    if (packageFields[fieldName] !== expected) {
      throw new Error(
        `unexpected Debian ${fieldName}: ${packageFields[fieldName]}`,
      );
    }
  }
  if (!["amd64", "arm64"].includes(packageFields.Architecture)) {
    throw new Error(
      `unexpected Debian Architecture: ${packageFields.Architecture}`,
    );
  }
  if (
    !packageFields.Description.includes("Local AI coding-tool usage tracker")
  ) {
    throw new Error(
      "Debian description is missing the reviewed product scope.",
    );
  }

  const extractDirectory = path.join(workspace, "extract");
  await command("dpkg-deb", ["-x", debPath, extractDirectory]);
  const files = await filesBelow(extractDirectory);
  const relativeFiles = files.map((filePath) =>
    relativeTo(extractDirectory, filePath),
  );

  const desktopEntry = await firstMatchingFile(
    extractDirectory,
    (relativePath) =>
      relativePath.startsWith("usr/share/applications/") &&
      relativePath.endsWith(".desktop"),
  );
  if (!desktopEntry)
    throw new Error("Debian package is missing a desktop entry.");
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

  const appExecutable = path.join(extractDirectory, "usr", "bin", "burnly");
  const appMetadata = await stat(appExecutable);
  if ((appMetadata.mode & 0o111) === 0) {
    throw new Error("Burnly executable is not executable.");
  }

  const sidecarManifestPath = await firstMatchingFile(
    extractDirectory,
    (relativePath) => relativePath.endsWith("sidecars/ccusage/manifest.json"),
  );
  if (!sidecarManifestPath) {
    throw new Error("Debian package is missing the ccusage sidecar manifest.");
  }
  const sidecarManifest = JSON.parse(
    await readFile(sidecarManifestPath, "utf8"),
  );
  const expectedTarget =
    rustTargetByDebArchitecture[packageFields.Architecture];
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
  if ((sidecarMetadata.mode & 0o111) === 0) {
    throw new Error("Packaged ccusage sidecar is not executable.");
  }
  const sidecarHash = createHash("sha256")
    .update(await readFile(sidecarExecutable))
    .digest("hex");
  if (sidecarHash !== sidecarEntry.integrity?.sha256) {
    throw new Error(
      "Packaged ccusage sidecar checksum does not match manifest.",
    );
  }

  const icon = relativeFiles.find((relativePath) =>
    relativePath.endsWith("/icons/hicolor/128x128/apps/burnly.png"),
  );
  if (!icon)
    throw new Error("Debian package is missing the reviewed 128px icon.");

  const hostMatchesArtifact =
    hostArchitectureByDebArchitecture[packageFields.Architecture] ===
    process.arch;
  const sidecarVersion = hostMatchesArtifact
    ? await executableSidecarVersion(
        sidecarExecutable,
        sidecarManifest.expectedVersion,
      )
    : "skipped: artifact architecture does not match host";

  console.log(
    JSON.stringify(
      {
        package: packageFields.Package,
        version: packageFields.Version,
        architecture: packageFields.Architecture,
        desktopEntry: relativeTo(extractDirectory, desktopEntry),
        appExecutable: relativeTo(extractDirectory, appExecutable),
        sidecarManifest: relativeTo(extractDirectory, sidecarManifestPath),
        sidecarSha256: sidecarHash,
        sidecarVersion,
      },
      null,
      2,
    ),
  );
  console.log("Linux Debian smoke passed.");
} finally {
  await rm(workspace, { recursive: true, force: true });
}
