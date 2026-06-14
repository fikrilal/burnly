import { spawnSync } from "node:child_process";

const result = spawnSync("pnpm", ["tauri", "info"], {
  encoding: "utf8",
  stdio: "pipe",
});

if (result.stdout.trim().length > 0) {
  console.log(result.stdout.trim());
}

if (result.stderr.trim().length > 0) {
  console.error(result.stderr.trim());
}

const combinedOutput = `${result.stdout}\n${result.stderr}`;

if (result.status === 0 && !combinedOutput.includes("not installed")) {
  console.log("Desktop prerequisite evidence collected.");
  process.exit(0);
}

console.error(
  "Desktop prerequisite evidence failed. Install Tauri OS prerequisites.",
);
process.exit(result.status ?? 1);
