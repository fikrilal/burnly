import { readFile } from "node:fs/promises";

const requestedTag = process.argv[2];
const packageDocument = JSON.parse(await readFile("package.json", "utf8"));
const cargoSource = await readFile("src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargoSource.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

if (cargoVersion !== packageDocument.version) {
  console.error(
    `Version mismatch: package.json=${packageDocument.version}, Cargo.toml=${cargoVersion ?? "missing"}.`,
  );
  process.exit(1);
}
if (requestedTag && requestedTag !== `v${packageDocument.version}`) {
  console.error(
    `Release tag ${requestedTag} must equal v${packageDocument.version}.`,
  );
  process.exit(1);
}

console.log(`Release version ${packageDocument.version} passed.`);
