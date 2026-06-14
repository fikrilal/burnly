import { spawnSync } from "node:child_process";

run("pnpm", ["tauri", "info"], "Tauri prerequisite evidence");
run("pnpm", ["contracts:check"], "Generated contract evidence");
run("pnpm", ["build"], "Frontend build evidence");
run(
  "cargo",
  [
    "test",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "bootstrap::tests::tauri_bridge",
    "--",
    "--nocapture",
  ],
  "Tauri IPC bridge evidence",
);

console.log("Desktop runtime evidence passed.");

function run(command, args, label) {
  console.log(`\n${label}`);

  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: "pipe",
  });

  writeOutput(result.stdout, console.log);
  writeOutput(result.stderr, console.error);

  const combinedOutput = `${result.stdout}\n${result.stderr}`;
  if (result.status === 0 && !combinedOutput.includes("not installed")) {
    return;
  }

  console.error(`${label} failed.`);
  process.exit(result.status ?? 1);
}

function writeOutput(output, writer) {
  const trimmed = output.trim();
  if (trimmed.length > 0) {
    writer(trimmed);
  }
}
