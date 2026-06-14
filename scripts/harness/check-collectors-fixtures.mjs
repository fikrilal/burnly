import { access } from "node:fs/promises";
import path from "node:path";

const fixturesDir = path.join(
  process.cwd(),
  "tests",
  "fixtures",
  "collectors",
  "ccusage",
);

await access(fixturesDir);

console.log("Collector fixture check passed. No collector fixtures exist yet.");
