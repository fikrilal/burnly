import { spawnSync } from "node:child_process";
import os from "node:os";

printPlatformEvidence();
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
run(
  "cargo",
  ["test", "--manifest-path", "src-tauri/Cargo.toml", "platform::"],
  "Phase 7 platform lifecycle and tray unit evidence",
);
run(
  "cargo",
  [
    "test",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "application::refresh::scheduler",
  ],
  "Phase 7 background refresh scheduler evidence",
);

console.log("Desktop runtime evidence passed.");

function printPlatformEvidence() {
  console.log("\nDesktop platform evidence");
  console.log(`platform=${process.platform}`);
  console.log(`arch=${process.arch}`);
  console.log(`os=${os.type()} ${os.release()}`);
  console.log(`desktop=${readEnv("XDG_CURRENT_DESKTOP")}`);
  console.log(`desktopSession=${readEnv("DESKTOP_SESSION")}`);
  console.log(`sessionType=${readEnv("XDG_SESSION_TYPE")}`);
  console.log(`display=${readEnv("DISPLAY")}`);
  console.log(`waylandDisplay=${readEnv("WAYLAND_DISPLAY")}`);
}

function readEnv(name) {
  return process.env[name] || "unreported";
}

function run(command, args, label) {
  console.log(`\n${label}`);

  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: "pipe",
  });

  writeOutput(result.stdout, console.log);
  writeOutput(result.stderr, console.error);

  if (result.status === 0) {
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
