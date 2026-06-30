import { open, stat } from "node:fs/promises";
import path from "node:path";

const providedDmgPath = process.argv[2];
if (!providedDmgPath) {
  console.error("Usage: pnpm macos-smoke:dmg <path-to-dmg>");
  process.exit(1);
}

const dmgPath = path.resolve(providedDmgPath);
const fileName = path.basename(dmgPath);
if (!fileName.endsWith(".dmg")) {
  throw new Error(`macOS installer must use .dmg extension: ${fileName}`);
}
if (!/^burnly-v\d+\.\d+\.\d+-macos-(aarch64|x86_64)\.dmg$/.test(fileName)) {
  throw new Error(`Unexpected macOS installer artifact name: ${fileName}`);
}

const metadata = await stat(dmgPath);
if (!metadata.isFile()) {
  throw new Error(`macOS installer is not a file: ${dmgPath}`);
}
if (metadata.size < 1024 * 1024) {
  throw new Error("macOS installer is unexpectedly small.");
}

// UDIF disk images end with a 512-byte trailer whose magic is "koly".
const trailerLength = 512;
const handle = await open(dmgPath, "r");
try {
  const trailer = Buffer.alloc(trailerLength);
  await handle.read(trailer, 0, trailerLength, metadata.size - trailerLength);
  if (trailer.subarray(0, 4).toString("ascii") !== "koly") {
    throw new Error("macOS installer is missing the UDIF koly trailer.");
  }
} finally {
  await handle.close();
}

console.log(
  JSON.stringify(
    {
      installer: fileName,
      bytes: metadata.size,
    },
    null,
    2,
  ),
);
