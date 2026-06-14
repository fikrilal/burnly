import { spawnSync } from "node:child_process";
import { access } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const manifestPath = path.join(root, "src-tauri", "Cargo.toml");
const metadataResult = spawnSync(
  "cargo",
  [
    "metadata",
    "--format-version",
    "1",
    "--manifest-path",
    manifestPath,
    "--no-deps",
  ],
  { encoding: "utf8" },
);

if (metadataResult.status !== 0) {
  console.error(metadataResult.stderr.trim());
  process.exit(metadataResult.status ?? 1);
}

const metadata = JSON.parse(metadataResult.stdout);
const burnly = metadata.packages.find((packageMetadata) =>
  packageMetadata.manifest_path.endsWith("/src-tauri/Cargo.toml"),
);

if (burnly === undefined) {
  console.error("Migration check failed: Burnly Cargo package was not found.");
  process.exit(1);
}

const failures = [];
const rusqlite = burnly.dependencies.find(
  (dependency) => dependency.name === "rusqlite",
);
const migration = burnly.dependencies.find(
  (dependency) => dependency.name === "rusqlite_migration",
);

if (rusqlite?.req !== "=0.40.1") {
  failures.push("rusqlite must remain pinned to =0.40.1.");
}

for (const feature of ["backup", "bundled"]) {
  if (!rusqlite?.features.includes(feature)) {
    failures.push(`rusqlite must enable the ${feature} feature.`);
  }
}

if (migration?.req !== "=2.6.0") {
  failures.push("rusqlite_migration must remain pinned to =2.6.0.");
}

if (failures.length > 0) {
  console.error("Migration check failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

const migrationsDir = path.join(root, "src-tauri", "migrations");

try {
  await access(migrationsDir);
  console.log("Migration dependency and directory checks passed.");
} catch {
  console.log(
    "Migration dependency check passed. Migration files begin in Phase 1C.",
  );
}
