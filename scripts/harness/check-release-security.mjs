import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const allowedCorePermissions = new Set([
  "core:event:allow-listen",
  "core:event:allow-unlisten",
]);
const forbiddenPermissionPrefixes = [
  "core:default",
  "shell:",
  "fs:",
  "http:",
  "websocket:",
  "process:",
  "opener:",
  "dialog:",
  "notification:",
  "updater:",
];

function commandPermission(command) {
  return `allow-${command.replaceAll("_", "-")}`;
}

function commandNames(contractSource) {
  return [
    ...contractSource.matchAll(
      /CommandSpec\s*\{[\s\S]*?\bname:\s*"([^"]+)"[\s\S]*?\}/g,
    ),
  ].map((match) => match[1]);
}

function buildManifestCommands(buildSource) {
  const block = buildSource.match(
    /AppManifest::new\(\)\.commands\(&\[([\s\S]*?)\]\)/,
  );
  if (!block) return [];
  return [...block[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

function validatePolicy({
  config,
  capabilities,
  contractCommands,
  manifestCommands,
  frontendDependencies,
}) {
  const failures = [];
  const security = config.app?.security;
  if (!security || security.csp === null || security.csp === undefined) {
    failures.push(
      "src-tauri/tauri.conf.json: production CSP must be explicit.",
    );
  } else {
    const cspText = JSON.stringify(security.csp);
    for (const unsafeSource of [
      "'unsafe-eval'",
      "http://*",
      "https://*",
      "ws://*",
      "wss://*",
    ]) {
      if (cspText.includes(unsafeSource)) {
        failures.push(
          `src-tauri/tauri.conf.json: CSP must not include ${unsafeSource}.`,
        );
      }
    }
    const requiredDirectives = {
      "default-src": "'self'",
      "connect-src": "ipc: http://ipc.localhost",
      "object-src": "'none'",
      "base-uri": "'none'",
      "frame-src": "'none'",
    };
    for (const [directive, expected] of Object.entries(requiredDirectives)) {
      if (security.csp[directive] !== expected) {
        failures.push(
          `src-tauri/tauri.conf.json: CSP ${directive} must be ${expected}.`,
        );
      }
    }
  }

  if (
    JSON.stringify(security?.capabilities) !== JSON.stringify(["main-window"])
  ) {
    failures.push(
      "src-tauri/tauri.conf.json: explicitly enable only the main-window capability.",
    );
  }
  if (security?.assetProtocol?.enable !== false) {
    failures.push(
      "src-tauri/tauri.conf.json: asset protocol must remain disabled.",
    );
  }
  if (security?.dangerousDisableAssetCspModification !== false) {
    failures.push(
      "src-tauri/tauri.conf.json: Tauri CSP injection must remain enabled.",
    );
  }

  if (capabilities.length !== 1) {
    failures.push(
      "src-tauri/capabilities: exactly one reviewed capability is allowed.",
    );
  }
  const capability = capabilities[0];
  if (capability?.identifier !== "main-window") {
    failures.push(
      "src-tauri/capabilities: capability identifier must be main-window.",
    );
  }
  if (JSON.stringify(capability?.windows) !== JSON.stringify(["main"])) {
    failures.push(
      "src-tauri/capabilities: capability must target only the main window.",
    );
  }
  if ("remote" in (capability ?? {})) {
    failures.push(
      "src-tauri/capabilities: remote URLs must not receive local capabilities.",
    );
  }

  const permissions = capability?.permissions ?? [];
  for (const permission of permissions) {
    if (typeof permission !== "string") {
      failures.push(
        "src-tauri/capabilities: scoped plugin permissions are not approved.",
      );
      continue;
    }
    if (
      forbiddenPermissionPrefixes.some(
        (prefix) => permission === prefix || permission.startsWith(prefix),
      )
    ) {
      failures.push(
        `src-tauri/capabilities: forbidden webview permission ${permission}.`,
      );
    }
  }

  const expectedCommands = new Set(contractCommands);
  const generatedCommands = new Set(manifestCommands);
  for (const command of expectedCommands) {
    if (!generatedCommands.has(command)) {
      failures.push(`src-tauri/build.rs: missing command ${command}.`);
    }
  }
  for (const command of generatedCommands) {
    if (!expectedCommands.has(command)) {
      failures.push(`src-tauri/build.rs: unregistered command ${command}.`);
    }
  }

  const expectedPermissions = new Set([
    ...allowedCorePermissions,
    ...contractCommands.map(commandPermission),
  ]);
  const actualPermissions = new Set(
    permissions.filter((permission) => typeof permission === "string"),
  );
  for (const permission of expectedPermissions) {
    if (!actualPermissions.has(permission)) {
      failures.push(
        `src-tauri/capabilities/main-window.json: missing ${permission}.`,
      );
    }
  }
  for (const permission of actualPermissions) {
    if (!expectedPermissions.has(permission)) {
      failures.push(
        `src-tauri/capabilities/main-window.json: unreviewed permission ${permission}.`,
      );
    }
  }

  for (const dependency of Object.keys(frontendDependencies)) {
    if (
      dependency.startsWith("@tauri-apps/plugin-") ||
      ["@tauri-apps/plugin-shell", "@tauri-apps/plugin-fs"].includes(dependency)
    ) {
      failures.push(
        `package.json: frontend plugin dependency ${dependency} is not approved.`,
      );
    }
  }

  return failures;
}

function runSelfTest() {
  const secure = {
    config: {
      app: {
        security: {
          capabilities: ["main-window"],
          csp: {
            "default-src": "'self'",
            "connect-src": "ipc: http://ipc.localhost",
            "object-src": "'none'",
            "base-uri": "'none'",
            "frame-src": "'none'",
          },
          assetProtocol: { enable: false },
          dangerousDisableAssetCspModification: false,
        },
      },
    },
    capabilities: [
      {
        identifier: "main-window",
        windows: ["main"],
        permissions: [...allowedCorePermissions, "allow-app-get-bootstrap"],
      },
    ],
    contractCommands: ["app_get_bootstrap"],
    manifestCommands: ["app_get_bootstrap"],
    frontendDependencies: {},
  };
  const cases = [
    { name: "secure baseline", mutate: () => {}, expected: 0 },
    {
      name: "null CSP",
      mutate: (input) => {
        input.config.app.security.csp = null;
      },
      expected: 1,
    },
    {
      name: "broad shell permission",
      mutate: (input) => {
        input.capabilities[0].permissions.push("shell:default");
      },
      expected: 2,
    },
    {
      name: "remote capability",
      mutate: (input) => {
        input.capabilities[0].remote = { urls: ["https://example.com"] };
      },
      expected: 1,
    },
    {
      name: "manifest drift",
      mutate: (input) => {
        input.manifestCommands = [];
      },
      expected: 1,
    },
  ];
  const failed = [];
  for (const testCase of cases) {
    const input = structuredClone(secure);
    testCase.mutate(input);
    const actual = validatePolicy(input).length;
    if (actual !== testCase.expected) {
      failed.push(
        `${testCase.name}: expected ${testCase.expected}, got ${actual}`,
      );
    }
  }
  if (failed.length > 0) {
    console.error("Release security self-test failed:");
    for (const failure of failed) console.error(`- ${failure}`);
    process.exit(1);
  }
  console.log("Release security self-test passed.");
}

if (process.argv.includes("--self-test")) {
  runSelfTest();
  process.exit(0);
}

const capabilityDirectory = path.join(root, "src-tauri", "capabilities");
const capabilityFiles = (await readdir(capabilityDirectory))
  .filter((file) => file.endsWith(".json"))
  .sort();
const capabilities = await Promise.all(
  capabilityFiles.map(async (file) =>
    JSON.parse(await readFile(path.join(capabilityDirectory, file), "utf8")),
  ),
);
const config = JSON.parse(
  await readFile(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"),
);
const contractSource = await readFile(
  path.join(root, "src-tauri", "src", "ipc", "contract.rs"),
  "utf8",
);
const buildSource = await readFile(
  path.join(root, "src-tauri", "build.rs"),
  "utf8",
);
const packageDocument = JSON.parse(
  await readFile(path.join(root, "package.json"), "utf8"),
);
const failures = validatePolicy({
  config,
  capabilities,
  contractCommands: commandNames(contractSource),
  manifestCommands: buildManifestCommands(buildSource),
  frontendDependencies: packageDocument.dependencies ?? {},
});

if (failures.length > 0) {
  console.error("Release security check failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Release security capability and CSP checks passed.");
