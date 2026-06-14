import { access, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const generatedDir = path.join(process.cwd(), "src", "ipc", "generated");
const markerFile = path.join(generatedDir, ".gitkeep");

if (process.argv.includes("--generate-placeholder")) {
  await mkdir(generatedDir, { recursive: true });
  await writeFile(markerFile, "", { flag: "a" });
}

await access(generatedDir);

console.log("IPC contract check passed. No generated contracts exist yet.");
