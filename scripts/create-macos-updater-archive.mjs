import { execFile } from "node:child_process";
import { access, constants, mkdir, readdir, rm } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);

const [targetTriple, appPathArgument] = process.argv.slice(2);
if (!targetTriple) {
  console.error(
    "Usage: node scripts/create-macos-updater-archive.mjs <rust-target-triple> [path-to-app]",
  );
  process.exit(1);
}

const bundleRoot = path.join(
  "src-tauri",
  "target",
  targetTriple,
  "release",
  "bundle",
);
const macosBundleDirectory = path.join(bundleRoot, "macos");
const appPath = await resolveAppPath(
  appPathArgument
    ? path.resolve(appPathArgument)
    : path.resolve(path.join(macosBundleDirectory, "Burnly.app")),
);
const outputPath = path.resolve(macosBundleDirectory, "Burnly.app.tar.gz");

if (!appPath.endsWith(".app")) {
  throw new Error(`macOS updater input must be an .app bundle: ${appPath}`);
}
await mkdir(path.dirname(outputPath), { recursive: true });
await rm(outputPath, { force: true });

await execute("tar", [
  "-C",
  path.dirname(appPath),
  "-czf",
  outputPath,
  path.basename(appPath),
]);

console.log(outputPath);

async function resolveAppPath(preferredPath) {
  if (await directoryExists(preferredPath)) {
    return preferredPath;
  }
  const candidates = await appBundlesBelow(path.resolve(bundleRoot));
  if (candidates.length === 0) {
    throw new Error(
      `No macOS .app bundle found. Build the app bundle before archiving. Checked ${preferredPath} and ${path.resolve(bundleRoot)}.`,
    );
  }
  candidates.sort((left, right) => score(left) - score(right));
  return candidates[0];
}

async function appBundlesBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true }).catch(
    () => [],
  );
  const candidates = [];
  for (const entry of entries) {
    const candidate = path.join(directory, entry.name);
    if (!entry.isDirectory()) continue;
    if (entry.name.endsWith(".app")) {
      candidates.push(candidate);
      continue;
    }
    candidates.push(...(await appBundlesBelow(candidate)));
  }
  return candidates;
}

async function directoryExists(directory) {
  try {
    await access(directory, constants.R_OK);
    return true;
  } catch {
    return false;
  }
}

function score(candidate) {
  const normalized = candidate.split(path.sep).join("/");
  if (normalized.endsWith("/bundle/macos/Burnly.app")) return 0;
  if (normalized.includes("/bundle/macos/")) return 1;
  if (normalized.endsWith("/Burnly.app")) return 2;
  return 3;
}
