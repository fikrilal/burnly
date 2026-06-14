import { readdir, readFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const budgetPath = path.join(
  root,
  "scripts",
  "harness",
  "public-api-budget.json",
);
const budgets = JSON.parse(await readFile(budgetPath, "utf8"));
const failures = [];

async function collectPublicApiFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true }).catch(
    () => [],
  );
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectPublicApiFiles(fullPath)));
    } else if (entry.name === "index.ts" || entry.name === "index.tsx") {
      files.push(path.relative(root, fullPath));
    }
  }

  return files;
}

const publicApiFiles = await collectPublicApiFiles(path.join(root, "src"));

for (const publicApiFile of publicApiFiles) {
  if (!(publicApiFile in budgets)) {
    failures.push(
      `${publicApiFile}: public API file has no budget. Add it deliberately with execution-plan justification.`,
    );
  }
}

for (const [relativePath, budget] of Object.entries(budgets)) {
  const content = await readFile(path.join(root, relativePath), "utf8");
  const exportCount = content
    .split("\n")
    .filter((line) => /^export\s/.test(line.trim())).length;

  if (exportCount > budget) {
    failures.push(
      `${relativePath}: exports ${exportCount} public symbols; budget is ${budget}. ` +
        "Justify the API growth in the execution plan and update the budget deliberately.",
    );
  }
}

if (failures.length > 0) {
  console.error("Public API check failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Public API check passed.");
