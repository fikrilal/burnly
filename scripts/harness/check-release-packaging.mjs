import { createHash } from "node:crypto";
import { access, readFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const placeholderIconSha256 =
  "273cd669e07c455ad1c7c095890a37984652157cee73128a867300067dfb80e7";
const expectedTargets = {
  "aarch64-apple-darwin": {
    platform: "macos",
    architecture: "aarch64",
    bundles: [
      ["dmg", "dmg"],
      ["app", "app.tar.gz"],
    ],
  },
  "x86_64-apple-darwin": {
    platform: "macos",
    architecture: "x86_64",
    bundles: [
      ["dmg", "dmg"],
      ["app", "app.tar.gz"],
    ],
  },
  "aarch64-pc-windows-msvc": {
    platform: "windows",
    architecture: "aarch64",
    bundles: [["nsis", "exe"]],
  },
  "x86_64-pc-windows-msvc": {
    platform: "windows",
    architecture: "x86_64",
    bundles: [["nsis", "exe"]],
  },
  "aarch64-unknown-linux-gnu": {
    platform: "linux",
    architecture: "aarch64",
    bundles: [["appimage", "AppImage"]],
  },
  "x86_64-unknown-linux-gnu": {
    platform: "linux",
    architecture: "x86_64",
    bundles: [["appimage", "AppImage"]],
  },
};

function cargoField(source, field) {
  return source.match(new RegExp(`^${field}\\s*=\\s*"([^"]+)"`, "m"))?.[1];
}

function canonicalArtifactNames(releaseTargets, version) {
  return releaseTargets.targets.flatMap((target) =>
    target.bundles.map((bundle) =>
      releaseTargets.artifactNameTemplate
        .replace("{version}", version)
        .replace("{platform}", target.platform)
        .replace("{architecture}", target.architecture)
        .replace("{extension}", bundle.extension),
    ),
  );
}

function validateIdentity(
  { config, packageDocument, cargoSource, iconSha256 },
  failures,
) {
  const bundle = config.bundle ?? {};
  const expectedMetadata = {
    productName: "Burnly",
    identifier: "app.burnly.desktop",
    version: "../package.json",
  };
  for (const [field, expected] of Object.entries(expectedMetadata)) {
    if (config[field] !== expected) {
      failures.push(`src-tauri/tauri.conf.json: ${field} must be ${expected}.`);
    }
  }

  const bundleMetadata = {
    publisher: "Burnly",
    category: "DeveloperTool",
    shortDescription: "Local AI coding-tool usage tracker.",
  };
  for (const [field, expected] of Object.entries(bundleMetadata)) {
    if (bundle[field] !== expected) {
      failures.push(
        `src-tauri/tauri.conf.json: bundle.${field} must be ${expected}.`,
      );
    }
  }
  if (!bundle.copyright?.includes("Burnly contributors")) {
    failures.push(
      "src-tauri/tauri.conf.json: reviewed copyright metadata is required.",
    );
  }
  if (!bundle.longDescription?.includes("local-first")) {
    failures.push(
      "src-tauri/tauri.conf.json: long description must state the local-first product boundary.",
    );
  }

  if (packageDocument.version !== cargoField(cargoSource, "version")) {
    failures.push("package.json and src-tauri/Cargo.toml versions must match.");
  }
  if (packageDocument.description !== cargoField(cargoSource, "description")) {
    failures.push(
      "package.json and src-tauri/Cargo.toml descriptions must match.",
    );
  }
  if (iconSha256 === placeholderIconSha256) {
    failures.push(
      "src-tauri/icons/icon.png: Tauri placeholder icon is forbidden.",
    );
  }

  const expectedIcons = [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",
    "icons/icon.ico",
  ];
  if (JSON.stringify(bundle.icon) !== JSON.stringify(expectedIcons)) {
    failures.push(
      "src-tauri/tauri.conf.json: desktop icon set must remain explicit.",
    );
  }
}

function validatePlatformConfigs(
  { linuxConfig, macosConfig, windowsConfig },
  failures,
) {
  const platformTargets = [
    ["linux", linuxConfig.bundle?.targets, ["appimage"]],
    ["macos", macosConfig.bundle?.targets, ["dmg"]],
    ["windows", windowsConfig.bundle?.targets, ["nsis"]],
  ];
  for (const [platform, actual, expected] of platformTargets) {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      failures.push(
        `src-tauri/tauri.${platform}.conf.json: targets must be ${expected.join(", ")}.`,
      );
    }
  }

  if (windowsConfig.bundle?.windows?.allowDowngrades !== false) {
    failures.push(
      "src-tauri/tauri.windows.conf.json: Windows downgrades must be blocked.",
    );
  }
  const nsis = windowsConfig.bundle?.windows?.nsis;
  if (nsis?.installMode !== "currentUser") {
    failures.push(
      "src-tauri/tauri.windows.conf.json: NSIS must use currentUser install mode.",
    );
  }
  if (
    nsis?.installerIcon !== "icons/icon.ico" ||
    nsis?.uninstallerIcon !== "icons/icon.ico"
  ) {
    failures.push(
      "src-tauri/tauri.windows.conf.json: NSIS icons must use the reviewed application icon.",
    );
  }
}

function validateReleaseTargets(
  { releaseTargets, sidecarManifest, packageDocument },
  failures,
) {
  const targetTriples = new Set();
  for (const target of releaseTargets.targets ?? []) {
    const expected = expectedTargets[target.rustTargetTriple];
    if (!expected) {
      failures.push(
        `src-tauri/release-targets.json: unexpected target ${target.rustTargetTriple}.`,
      );
      continue;
    }
    targetTriples.add(target.rustTargetTriple);
    const bundles = target.bundles.map((bundle) => [
      bundle.kind,
      bundle.extension,
    ]);
    if (
      target.platform !== expected.platform ||
      target.architecture !== expected.architecture ||
      JSON.stringify(bundles) !== JSON.stringify(expected.bundles)
    ) {
      failures.push(
        `src-tauri/release-targets.json: invalid metadata for ${target.rustTargetTriple}.`,
      );
    }
  }
  if (targetTriples.size !== Object.keys(expectedTargets).length) {
    failures.push(
      "src-tauri/release-targets.json: all six supported targets are required.",
    );
  }

  const sidecarTriples = new Set(
    sidecarManifest.entries.map((entry) => entry.rustTargetTriple),
  );
  if (
    [...targetTriples].some((target) => !sidecarTriples.has(target)) ||
    [...sidecarTriples].some((target) => !targetTriples.has(target))
  ) {
    failures.push(
      "release target triples must exactly match packaged sidecar targets.",
    );
  }

  if (
    releaseTargets.artifactNameTemplate !==
    "burnly-v{version}-{platform}-{architecture}.{extension}"
  ) {
    failures.push(
      "src-tauri/release-targets.json: artifact name template changed without review.",
    );
  }
  const artifactNames = canonicalArtifactNames(
    releaseTargets,
    packageDocument.version,
  );
  if (new Set(artifactNames).size !== artifactNames.length) {
    failures.push("canonical release artifact names must be unique.");
  }
  if (
    artifactNames.some(
      (name) =>
        !name.startsWith(`burnly-v${packageDocument.version}-`) ||
        !/-(aarch64|x86_64)\./.test(name),
    )
  ) {
    failures.push(
      "canonical release artifact names must include version and architecture.",
    );
  }
}

function validateGuide(packagingGuide, failures) {
  for (const requiredText of [
    "app.burnly.desktop",
    "Uninstalling Burnly does not delete",
    "Downgrades are unsupported",
    "burnly-v{version}-{platform}-{architecture}.{extension}",
  ]) {
    if (!packagingGuide.includes(requiredText)) {
      failures.push(
        `docs/engineering/release-packaging.md: missing ${requiredText}.`,
      );
    }
  }
}

function validateTauriRunner(tauriRunner, failures) {
  if (!tauriRunner.includes("BURNLY_CCUSAGE_DEV_BINARY")) {
    failures.push(
      "scripts/run-tauri.mjs: Tauri development must retain the explicit fake collector boundary.",
    );
  }
  if (!tauriRunner.includes('shell: process.platform === "win32"')) {
    failures.push(
      "scripts/run-tauri.mjs: Windows must launch the Tauri .cmd shim through a shell.",
    );
  }
}

function validate(inputs) {
  const failures = [];
  validateIdentity(inputs, failures);
  validatePlatformConfigs(inputs, failures);
  validateReleaseTargets(inputs, failures);
  validateGuide(inputs.packagingGuide, failures);
  validateTauriRunner(inputs.tauriRunner, failures);
  return failures;
}

async function loadInputs() {
  const readJson = async (relativePath) =>
    JSON.parse(await readFile(path.join(root, relativePath), "utf8"));
  const icon = await readFile(path.join(root, "src-tauri/icons/icon.png"));

  for (const iconPath of [
    "src-tauri/icons/burnly-icon.svg",
    "src-tauri/icons/32x32.png",
    "src-tauri/icons/128x128.png",
    "src-tauri/icons/128x128@2x.png",
    "src-tauri/icons/icon.icns",
    "src-tauri/icons/icon.ico",
  ]) {
    await access(path.join(root, iconPath));
  }
  for (const scriptPath of [
    "scripts/install-linux.sh",
    "scripts/install-macos.sh",
    "scripts/stage-release-artifacts.mjs",
  ]) {
    await access(path.join(root, scriptPath));
  }

  return {
    config: await readJson("src-tauri/tauri.conf.json"),
    linuxConfig: await readJson("src-tauri/tauri.linux.conf.json"),
    macosConfig: await readJson("src-tauri/tauri.macos.conf.json"),
    windowsConfig: await readJson("src-tauri/tauri.windows.conf.json"),
    packageDocument: await readJson("package.json"),
    cargoSource: await readFile(
      path.join(root, "src-tauri/Cargo.toml"),
      "utf8",
    ),
    releaseTargets: await readJson("src-tauri/release-targets.json"),
    sidecarManifest: await readJson(
      "src-tauri/sidecars/ccusage/release-manifest.json",
    ),
    packagingGuide: await readFile(
      path.join(root, "docs/engineering/release-packaging.md"),
      "utf8",
    ),
    tauriRunner: await readFile(
      path.join(root, "scripts/run-tauri.mjs"),
      "utf8",
    ),
    iconSha256: createHash("sha256").update(icon).digest("hex"),
  };
}

const inputs = await loadInputs();
const failures = validate(inputs);

if (process.argv.includes("--self-test")) {
  const mutated = structuredClone(inputs);
  mutated.config.identifier = "com.example.placeholder";
  mutated.windowsConfig.bundle.windows.allowDowngrades = true;
  mutated.releaseTargets.targets.pop();
  mutated.iconSha256 = placeholderIconSha256;
  mutated.tauriRunner = 'spawn(executable, args, { stdio: "inherit" });';
  if (validate(mutated).length < 4) {
    console.error("Release packaging harness self-test did not catch drift.");
    process.exit(1);
  }
  console.log("Release packaging harness self-test passed.");
  process.exit(0);
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Release packaging metadata passed.");
