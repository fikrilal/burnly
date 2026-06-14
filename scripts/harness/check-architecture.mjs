import { readdir, readFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const failures = [];
const forbiddenGenericNames = new Set([
  "helper",
  "helpers",
  "manager",
  "utils",
]);

async function collectFiles(directory, extensions) {
  const entries = await readdir(directory, { withFileTypes: true }).catch(
    () => [],
  );
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);

    if (entry.isDirectory()) {
      if (["node_modules", "dist", "target", "gen"].includes(entry.name)) {
        continue;
      }

      files.push(...(await collectFiles(fullPath, extensions)));
      continue;
    }

    if (extensions.includes(path.extname(entry.name))) {
      files.push(fullPath);
    }
  }

  return files;
}

function relative(filePath) {
  return path.relative(root, filePath);
}

function checkGenericName(file) {
  const rel = relative(file);
  const segments = rel.split(path.sep);

  for (const segment of segments) {
    const name = path.parse(segment).name.toLowerCase();
    if (forbiddenGenericNames.has(name)) {
      failures.push(
        `${rel}: generic name "${name}" hides ownership. Use a domain-specific name.`,
      );
      return;
    }
  }
}

async function resolveTypeScriptImport(fromFile, specifier, knownFiles) {
  if (!specifier.startsWith(".")) {
    return undefined;
  }

  const base = path.resolve(path.dirname(fromFile), specifier);
  const candidates = [
    base,
    `${base}.ts`,
    `${base}.tsx`,
    path.join(base, "index.ts"),
    path.join(base, "index.tsx"),
  ];

  return candidates.find((candidate) => knownFiles.has(candidate));
}

function findCycle(graph) {
  const visited = new Set();
  const active = new Set();
  const stack = [];

  function visit(node) {
    if (active.has(node)) {
      const start = stack.indexOf(node);
      return [...stack.slice(start), node];
    }

    if (visited.has(node)) {
      return undefined;
    }

    visited.add(node);
    active.add(node);
    stack.push(node);

    for (const dependency of graph.get(node) ?? []) {
      const cycle = visit(dependency);
      if (cycle !== undefined) {
        return cycle;
      }
    }

    stack.pop();
    active.delete(node);
    return undefined;
  }

  for (const node of graph.keys()) {
    const cycle = visit(node);
    if (cycle !== undefined) {
      return cycle;
    }
  }

  return undefined;
}

async function checkFrontendBoundaries() {
  const files = await collectFiles(path.join(root, "src"), [".ts", ".tsx"]);
  const knownFiles = new Set(files.map((file) => path.resolve(file)));
  const importGraph = new Map();

  for (const file of files) {
    const rel = relative(file);
    const content = await readFile(file, "utf8");
    checkGenericName(file);

    const dependencies = [];
    const importPattern =
      /(?:import|export)\s+(?:[^"']+\s+from\s+)?["']([^"']+)["']/g;
    for (const match of content.matchAll(importPattern)) {
      const dependency = await resolveTypeScriptImport(
        file,
        match[1],
        knownFiles,
      );
      if (dependency !== undefined) {
        dependencies.push(dependency);
      }
    }
    importGraph.set(path.resolve(file), dependencies);

    if (
      !rel.startsWith("src/ipc/") &&
      /from\s+["']@tauri-apps\/api/.test(content)
    ) {
      failures.push(
        `${rel}: Tauri APIs must be wrapped in src/ipc before reaching React code.`,
      );
    }

    if (
      rel.startsWith("src/components/ui/") &&
      /from\s+["'](?:\.\.\/)*\.\.\/features\//.test(content)
    ) {
      failures.push(`${rel}: shared UI components must not import features.`);
    }

    if (!rel.startsWith("src/ipc/") && /\bIpcResponse\b/.test(content)) {
      failures.push(
        `${rel}: transport details must remain behind the src/ipc boundary.`,
      );
    }
  }

  const cycle = findCycle(importGraph);
  if (cycle !== undefined) {
    failures.push(
      `TypeScript dependency cycle: ${cycle.map(relative).join(" -> ")}`,
    );
  }
}

async function checkRustBoundaries() {
  const rustFiles = await collectFiles(path.join(root, "src-tauri", "src"), [
    ".rs",
  ]);

  for (const file of rustFiles) {
    const rel = relative(file);
    const content = await readFile(file, "utf8");
    checkGenericName(file);

    if (
      (rel.startsWith("src-tauri/src/domain/") ||
        rel.startsWith("src-tauri/src/application/")) &&
      /\b(tauri|rusqlite|std::process|tokio::process)\b/.test(content)
    ) {
      failures.push(
        `${rel}: domain/application code must not depend on Tauri, SQLite, or process execution.`,
      );
    }

    if (
      (rel.startsWith("src-tauri/src/domain/") ||
        rel.startsWith("src-tauri/src/application/")) &&
      /\b(ccusage|CollectorEnvelope|RawCollector|serde_json::Value)\b/i.test(
        content,
      )
    ) {
      failures.push(
        `${rel}: collector transport details must remain in infrastructure adapters.`,
      );
    }

    if (
      !rel.startsWith("src-tauri/src/infrastructure/") &&
      /\brusqlite\b/.test(content)
    ) {
      failures.push(
        `${rel}: SQLite details must remain in infrastructure persistence modules.`,
      );
    }
  }
}

await checkFrontendBoundaries();
await checkRustBoundaries();

if (failures.length > 0) {
  console.error("Architecture boundary check failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Architecture boundary check passed.");
