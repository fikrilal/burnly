import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const generatedDir = path.join(root, "src", "ipc", "generated");
const markerFile = path.join(generatedDir, ".gitkeep");
const responseModule = path.join(
  root,
  "src-tauri",
  "src",
  "ipc",
  "response.rs",
);
const fixtureDirectory = path.join(root, "tests", "fixtures", "ipc", "v1");
const failures = [];

if (process.argv.includes("--generate-placeholder")) {
  await mkdir(generatedDir, { recursive: true });
  await writeFile(markerFile, "", { flag: "a" });
}

await access(generatedDir);

const responseSource = await readFile(responseModule, "utf8");

if (!/const CONTRACT_VERSION: u16 = 1;/.test(responseSource)) {
  failures.push("Rust IPC contract version must remain explicitly set to 1.");
}

if (!/serde\(rename_all = "camelCase"\)/.test(responseSource)) {
  failures.push("Rust IPC response DTOs must enforce camelCase wire fields.");
}

const fixtureNames = ["response-success.json", "response-error.json"];
for (const fixtureName of fixtureNames) {
  const fixturePath = path.join(fixtureDirectory, fixtureName);
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"));

  if (fixture.meta?.contractVersion !== 1) {
    failures.push(`${fixtureName}: contractVersion must be 1.`);
  }

  if (typeof fixture.meta?.requestId !== "string") {
    failures.push(`${fixtureName}: requestId must be present.`);
  }

  if (!fixture.meta?.generatedAt?.endsWith("Z")) {
    failures.push(`${fixtureName}: generatedAt must be UTC.`);
  }
}

if (failures.length > 0) {
  console.error("IPC contract check failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "IPC response foundation and fixtures passed. Generated contracts begin in Phase 2B.",
);
