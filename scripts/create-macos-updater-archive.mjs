import { execFile } from "node:child_process";
import { access, constants, mkdir, rm } from "node:fs/promises";
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

const bundleDirectory = path.join(
  "src-tauri",
  "target",
  targetTriple,
  "release",
  "bundle",
  "macos",
);
const appPath = path.resolve(
  appPathArgument ?? path.join(bundleDirectory, "Burnly.app"),
);
const outputPath = path.resolve(bundleDirectory, "Burnly.app.tar.gz");

if (!appPath.endsWith(".app")) {
  throw new Error(`macOS updater input must be an .app bundle: ${appPath}`);
}
await access(appPath, constants.R_OK);
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
