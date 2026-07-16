import { spawn } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const args = process.argv.slice(2);
const executable = path.resolve(
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);

const environment = loadEnvFiles({
  ...process.env,
});

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

/**
 * Load optional `.env` then `.env.local` from the repo root.
 * Existing process env wins (does not override shell exports).
 */
function loadEnvFiles(base) {
  const env = { ...base };
  for (const name of [".env", ".env.local"]) {
    const filePath = path.join(root, name);
    if (!existsSync(filePath)) continue;
    applyEnvFile(env, readFileSync(filePath, "utf8"), name);
  }
  return env;
}

function applyEnvFile(env, contents, label) {
  let loaded = 0;
  for (const rawLine of contents.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const withoutExport = line.startsWith("export ")
      ? line.slice("export ".length).trim()
      : line;
    const eq = withoutExport.indexOf("=");
    if (eq <= 0) continue;
    const key = withoutExport.slice(0, eq).trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) continue;
    // Shell / CI already set — keep that value.
    if (Object.prototype.hasOwnProperty.call(process.env, key)) continue;
    let value = withoutExport.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    env[key] = value;
    loaded += 1;
  }
  if (loaded > 0) {
    console.info(`[burnly] loaded ${loaded} var(s) from ${label}`);
  }
}
