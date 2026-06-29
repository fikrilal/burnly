import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import { createRequire } from "node:module";

const root = process.cwd();
const require = createRequire(import.meta.url);
const sidecarDirectory = path.join(root, "src-tauri", "sidecars", "ccusage");
const releaseManifestPath = path.join(
  sidecarDirectory,
  "release-manifest.json",
);
const runtimeDirectory = path.join(sidecarDirectory, "runtime");
const payloadHeader = Buffer.from("BURNLY-CCUSAGE-PAYLOAD-V1\n", "utf8");
const manifest = JSON.parse(await readFile(releaseManifestPath, "utf8"));
const checkOnly = process.argv.includes("--check");
const rustTargetTriple =
  process.env.BURNLY_SIDECAR_TARGET ?? hostTargetTriple();
const entry = manifest.entries.find(
  (candidate) => candidate.rustTargetTriple === rustTargetTriple,
);

if (!entry) {
  throw new Error(`unsupported ccusage target ${rustTargetTriple}`);
}
if (manifest.expectedVersion !== "20.0.14") {
  throw new Error("ccusage release manifest version is not pinned to 20.0.14");
}

const ccusagePackage = await realpath(require.resolve("ccusage/package.json"));
const dependencyRoot = path.dirname(path.dirname(ccusagePackage));
const nativePackageDirectory = path.join(dependencyRoot, entry.packageName);
const nativePackage = JSON.parse(
  await readFile(path.join(nativePackageDirectory, "package.json"), "utf8"),
);
if (
  nativePackage.name !== entry.packageName ||
  nativePackage.version !== manifest.expectedVersion
) {
  throw new Error(
    `installed ${entry.packageName} does not match ${manifest.expectedVersion}`,
  );
}

const sourceBinary = path.join(
  nativePackageDirectory,
  "bin",
  entry.executableName,
);
const observedChecksum = await sha256(sourceBinary);
if (observedChecksum !== entry.integrity.sha256) {
  throw new Error(
    `${entry.packageName} checksum mismatch: expected ${entry.integrity.sha256}, observed ${observedChecksum}`,
  );
}

let verifiedBinary = sourceBinary;
if (!checkOnly) {
  await rm(runtimeDirectory, { recursive: true, force: true });
  await mkdir(runtimeDirectory, { recursive: true });
  verifiedBinary = path.join(runtimeDirectory, entry.executableName);
  await copyFile(sourceBinary, verifiedBinary);
  await writeFile(
    `${verifiedBinary}.payload`,
    Buffer.concat([payloadHeader, await readFile(sourceBinary)]),
  );
  if (process.platform !== "win32") {
    await chmod(verifiedBinary, 0o755);
  }
  await writeFile(
    path.join(runtimeDirectory, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

if (rustTargetTriple === hostTargetTriple()) {
  let temporaryDirectory;
  if (checkOnly && process.platform !== "win32") {
    temporaryDirectory = await mkdtemp(
      path.join(tmpdir(), "burnly-ccusage-check-"),
    );
    verifiedBinary = path.join(temporaryDirectory, entry.executableName);
    await copyFile(sourceBinary, verifiedBinary);
    await chmod(verifiedBinary, 0o755);
  }
  try {
    const version = execFileSync(verifiedBinary, ["--version"], {
      encoding: "utf8",
      timeout: 5_000,
    }).trim();
    if (version !== `ccusage ${manifest.expectedVersion}`) {
      throw new Error(`staged ccusage returned unexpected version: ${version}`);
    }
  } finally {
    if (temporaryDirectory) {
      await rm(temporaryDirectory, { recursive: true, force: true });
    }
  }
}

console.log(
  `${checkOnly ? "Verified" : "Prepared verified"} ccusage ${manifest.expectedVersion} for ${rustTargetTriple}.`,
);

function hostTargetTriple() {
  return execFileSync("rustc", ["--print", "host-tuple"], {
    encoding: "utf8",
  }).trim();
}

async function sha256(file) {
  const contents = await readFile(file);
  return createHash("sha256").update(contents).digest("hex");
}
