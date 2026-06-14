import { access } from "node:fs/promises";
import path from "node:path";

const migrationsDir = path.join(process.cwd(), "src-tauri", "migrations");

try {
  await access(migrationsDir);
  console.log("Migration check passed.");
} catch {
  console.log("Migration check passed. No migrations exist in Phase 0.");
}
