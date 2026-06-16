import { spawnSync } from "node:child_process";
import { access, readFile, readdir } from "node:fs/promises";
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
  const migrationFiles = (await readdir(migrationsDir)).sort();
  const expectedFiles = ["0001_initial.sql"];

  if (JSON.stringify(migrationFiles) !== JSON.stringify(expectedFiles)) {
    failures.push(
      `migration files must be ${expectedFiles.join(", ")}; found ${migrationFiles.join(", ")}.`,
    );
  }

  const initialMigration = await readFile(
    path.join(migrationsDir, "0001_initial.sql"),
    "utf8",
  );
  const tableCount = [...initialMigration.matchAll(/CREATE TABLE /g)].length;
  const strictCount = [...initialMigration.matchAll(/\) STRICT;/g)].length;

  if (tableCount !== 13 || strictCount !== 13) {
    failures.push(
      `0001_initial.sql must define 13 STRICT tables; found ${tableCount} tables and ${strictCount} STRICT declarations.`,
    );
  }

  if (/PRAGMA\s+foreign_keys/i.test(initialMigration)) {
    failures.push(
      "migration SQL must not toggle foreign_keys inside the migration transaction.",
    );
  }

  if (/\bREAL\b/i.test(initialMigration)) {
    failures.push("migration SQL must not use floating-point REAL storage.");
  }

  if (failures.length > 0) {
    console.error("Migration check failed:");
    for (const failure of failures) {
      console.error(`- ${failure}`);
    }
    process.exit(1);
  }

  console.log("Migration dependency, naming, and schema checks passed.");
} catch {
  console.error("Migration check failed: src-tauri/migrations is missing.");
  process.exit(1);
}
