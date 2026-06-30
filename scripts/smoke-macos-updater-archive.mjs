import { open, stat } from "node:fs/promises";
import path from "node:path";

const providedArchivePath = process.argv[2];
if (!providedArchivePath) {
  console.error("Usage: node scripts/smoke-macos-updater-archive.mjs <path>");
  process.exit(1);
}

const archivePath = path.resolve(providedArchivePath);
const fileName = path.basename(archivePath);
if (!fileName.endsWith(".app.tar.gz")) {
  throw new Error(
    `macOS updater archive must end with .app.tar.gz: ${fileName}`,
  );
}
if (
  !/^burnly-v\d+\.\d+\.\d+-macos-(aarch64|x86_64)\.app\.tar\.gz$/.test(fileName)
) {
  throw new Error(`Unexpected macOS updater archive name: ${fileName}`);
}

const metadata = await stat(archivePath);
if (!metadata.isFile()) {
  throw new Error(`macOS updater archive is not a file: ${archivePath}`);
}
if (metadata.size < 128) {
  throw new Error("macOS updater archive is unexpectedly small.");
}

const handle = await open(archivePath, "r");
try {
  const magic = Buffer.alloc(2);
  await handle.read(magic, 0, magic.length, 0);
  if (magic[0] !== 0x1f || magic[1] !== 0x8b) {
    throw new Error("macOS updater archive is not gzip-compressed.");
  }
} finally {
  await handle.close();
}

console.log(
  JSON.stringify(
    {
      archive: fileName,
      bytes: metadata.size,
    },
    null,
    2,
  ),
);
