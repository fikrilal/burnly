import { access, readdir, readFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const failures = [];
const forbiddenGenericNames = new Set([
  "helper",
  "helpers",
  "manager",
  "utils",
]);
const rustLayerRules = {
  application: {
    forbiddenLayers: ["bootstrap", "infrastructure", "ipc", "platform"],
    forbiddenTechnologies: [
      "tauri",
      "rusqlite",
      "std::process",
      "tokio::process",
      "ccusage",
      "serde_json::Value",
    ],
  },
  domain: {
    forbiddenLayers: [
      "application",
      "bootstrap",
      "infrastructure",
      "ipc",
      "platform",
    ],
    forbiddenTechnologies: [
      "tauri",
      "rusqlite",
      "std::process",
      "tokio::process",
      "ccusage",
      "serde_json::Value",
    ],
  },
  infrastructure: {
    forbiddenLayers: ["bootstrap", "ipc", "platform"],
    forbiddenTechnologies: [],
  },
  ipc: {
    forbiddenLayers: ["bootstrap", "infrastructure", "platform"],
    forbiddenTechnologies: [
      "rusqlite",
      "std::process",
      "tokio::process",
      "ccusage",
      "serde_json::Value",
    ],
  },
  platform: {
    forbiddenLayers: ["bootstrap", "infrastructure", "ipc"],
    forbiddenTechnologies: [
      "rusqlite",
      "std::process",
      "tokio::process",
      "ccusage",
      "serde_json::Value",
    ],
  },
};

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
  return repositoryPath(path.relative(root, filePath));
}

function repositoryPath(filePath) {
  return filePath.replaceAll("\\", "/");
}

function checkGenericName(file) {
  const rel = relative(file);
  const segments = rel.split("/");

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

function rustLayerFor(relativePath) {
  const match = relativePath.match(/^src-tauri\/src\/([^/]+)\//);
  return match?.[1];
}

function rustLayerViolations(relativePath, content) {
  const layer = rustLayerFor(relativePath);
  const rule = rustLayerRules[layer];
  if (rule === undefined) {
    return [];
  }

  const violations = [];

  for (const forbiddenLayer of rule.forbiddenLayers) {
    const reference = new RegExp(`crate::${forbiddenLayer}\\b`);
    const groupedReference = new RegExp(
      `crate::\\{[^}]*\\b${forbiddenLayer}\\b`,
      "s",
    );
    const relativeReference = new RegExp(`(?:super::)+${forbiddenLayer}\\b`);
    if (
      reference.test(content) ||
      groupedReference.test(content) ||
      relativeReference.test(content)
    ) {
      violations.push(
        `${relativePath}: ${layer} must not depend on the ${forbiddenLayer} layer.`,
      );
    }
  }

  for (const technology of rule.forbiddenTechnologies) {
    if (content.toLowerCase().includes(technology.toLowerCase())) {
      violations.push(
        `${relativePath}: ${layer} must not depend on ${technology}.`,
      );
    }
  }

  return violations;
}

async function checkRequiredRustStructure() {
  const requiredFiles = [
    "application/mod.rs",
    "bootstrap.rs",
    "domain/mod.rs",
    "infrastructure/mod.rs",
    "ipc/mod.rs",
    "platform/mod.rs",
    "platform/single_instance.rs",
  ];

  for (const requiredFile of requiredFiles) {
    const fullPath = path.join(root, "src-tauri", "src", requiredFile);
    try {
      await access(fullPath);
    } catch {
      failures.push(
        `src-tauri/src/${requiredFile}: required Rust ownership module is missing.`,
      );
    }
  }

  const library = await readFile(
    path.join(root, "src-tauri", "src", "lib.rs"),
    "utf8",
  );
  for (const moduleName of [
    "application",
    "bootstrap",
    "domain",
    "infrastructure",
    "ipc",
    "platform",
  ]) {
    if (!new RegExp(`\\bmod\\s+${moduleName}\\s*;`).test(library)) {
      failures.push(
        `src-tauri/src/lib.rs: required module declaration "mod ${moduleName};" is missing.`,
      );
    }
  }
}

function runRustBoundarySelfTest() {
  if (repositoryPath("src\\ipc\\client.ts") !== "src/ipc/client.ts") {
    console.error("Architecture path normalization self-test failed.");
    process.exit(1);
  }

  const cases = [
    {
      name: "application may depend on domain",
      path: "src-tauri/src/application/example.rs",
      content: "use crate::domain::Usage;",
      expectedViolations: 0,
    },
    {
      name: "domain may not depend on application",
      path: "src-tauri/src/domain/example.rs",
      content: "use crate::application::UsageService;",
      expectedViolations: 1,
    },
    {
      name: "application may not depend on infrastructure",
      path: "src-tauri/src/application/example.rs",
      content: "use crate::infrastructure::Database;",
      expectedViolations: 1,
    },
    {
      name: "application may not use a relative path to infrastructure",
      path: "src-tauri/src/application/example.rs",
      content: "use super::super::infrastructure::Database;",
      expectedViolations: 1,
    },
    {
      name: "ipc may not depend on SQLite",
      path: "src-tauri/src/ipc/example.rs",
      content: "use rusqlite::Connection;",
      expectedViolations: 1,
    },
    {
      name: "infrastructure may depend on application",
      path: "src-tauri/src/infrastructure/example.rs",
      content: "use crate::application::UsageStore;",
      expectedViolations: 0,
    },
  ];

  const failedCases = cases.filter(
    (testCase) =>
      rustLayerViolations(testCase.path, testCase.content).length !==
      testCase.expectedViolations,
  );

  if (failedCases.length > 0) {
    console.error("Rust architecture self-test failed:");
    for (const failedCase of failedCases) {
      console.error(`- ${failedCase.name}`);
    }
    process.exit(1);
  }

  const ownershipCases = [
    {
      name: "database store may use rusqlite",
      path: "src-tauri/src/infrastructure/database/connection.rs",
      content: "use rusqlite::Connection;",
      expectFailure: false,
    },
    {
      name: "cline collector may use rusqlite for external tool database",
      path: "src-tauri/src/infrastructure/collectors/cline/store.rs",
      content: "use rusqlite::Connection;",
      expectFailure: false,
    },
    {
      name: "zcode collector may use rusqlite for external tool database",
      path: "src-tauri/src/infrastructure/collectors/zcode/store.rs",
      content: "use rusqlite::Connection;",
      expectFailure: false,
    },
    {
      name: "collector SQLite support may use rusqlite for external tool database opening",
      path: "src-tauri/src/infrastructure/collectors/support/sqlite.rs",
      content: "use rusqlite::Connection;",
      expectFailure: false,
    },
    {
      name: "infrastructure outside database may not use rusqlite",
      path: "src-tauri/src/infrastructure/leaked_store.rs",
      content: "use rusqlite::Connection;",
      expectFailure: true,
    },
    {
      name: "non-infrastructure file is not checked by ownership rule",
      path: "src-tauri/src/application/example.rs",
      content: "use rusqlite::Connection;",
      expectFailure: false,
    },
  ];

  const beforeLength = failures.length;
  for (const testCase of ownershipCases) {
    failures.length = beforeLength;
    checkDatabaseOwnership(testCase.path, testCase.content);
    const triggered = failures.length > beforeLength;
    if (triggered !== testCase.expectFailure) {
      console.error(`Rust architecture self-test failed: ${testCase.name}`);
      process.exit(1);
    }
  }
  failures.length = beforeLength;

  console.log("Rust architecture self-test passed.");
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
      !rel.startsWith("src/ipc/") &&
      /\b(?:invoke|listen)\s*\(/.test(content)
    ) {
      failures.push(
        `${rel}: direct Tauri invoke/listen calls must stay behind src/ipc.`,
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

const allowedRusqlitePaths = [
  "src-tauri/src/infrastructure/database/",
  "src-tauri/src/infrastructure/collectors/cline/",
  "src-tauri/src/infrastructure/collectors/zcode/",
  "src-tauri/src/infrastructure/collectors/support/sqlite.rs",
];

function checkDatabaseOwnership(relativePath, content) {
  if (!relativePath.startsWith("src-tauri/src/infrastructure/")) {
    return;
  }

  if (!content.includes("rusqlite")) {
    return;
  }

  const isAllowed = allowedRusqlitePaths.some((prefix) =>
    relativePath.startsWith(prefix),
  );

  if (!isAllowed) {
    failures.push(
      `${relativePath}: rusqlite may only be used inside infrastructure/database (production stores), infrastructure/collectors/{cline,zcode} (external tool database reads), or collectors/support/sqlite.rs (shared external database opening).`,
    );
  }
}

async function checkRustBoundaries() {
  await checkRequiredRustStructure();
  const rustFiles = await collectFiles(path.join(root, "src-tauri", "src"), [
    ".rs",
  ]);

  for (const file of rustFiles) {
    const rel = relative(file);
    const content = await readFile(file, "utf8");
    checkGenericName(file);
    failures.push(...rustLayerViolations(rel, content));
    checkDatabaseOwnership(rel, content);
  }
}

if (process.argv.includes("--self-test")) {
  runRustBoundarySelfTest();
  process.exit(0);
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
