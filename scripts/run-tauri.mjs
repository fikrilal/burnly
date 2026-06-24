import { spawn } from "node:child_process";
import path from "node:path";

const args = process.argv.slice(2);
const executable = path.resolve(
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);
const environment = { ...process.env };

if (args[0] === "dev" && environment.BURNLY_CCUSAGE_DEV_BINARY === undefined) {
  environment.BURNLY_CCUSAGE_DEV_BINARY = path.resolve(
    "tests/fixtures/collectors/ccusage/process/fake-collector.sh",
  );
}

const child = spawn(executable, args, {
  env: environment,
  shell: process.platform === "win32",
  stdio: "inherit",
});

child.on("error", (error) => {
  console.error(`Failed to start Tauri CLI: ${error.message}`);
  process.exit(1);
});
child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
