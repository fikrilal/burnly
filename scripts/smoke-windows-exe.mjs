import { readFile, stat } from "node:fs/promises";
import path from "node:path";

const providedExePath = process.argv[2];
if (!providedExePath) {
  console.error("Usage: pnpm windows-smoke:exe <path-to-exe>");
  process.exit(1);
}

const exePath = path.resolve(providedExePath);
const fileName = path.basename(exePath);
if (!fileName.endsWith(".exe")) {
  throw new Error(`Windows installer must use .exe extension: ${fileName}`);
}
if (!/^burnly-v\d+\.\d+\.\d+-windows-x86_64\.exe$/.test(fileName)) {
  throw new Error(`Unexpected Windows installer artifact name: ${fileName}`);
}

const metadata = await stat(exePath);
if (!metadata.isFile()) {
  throw new Error(`Windows installer is not a file: ${exePath}`);
}
if (metadata.size < 1024 * 1024) {
  throw new Error("Windows installer is unexpectedly small.");
}

const contents = await readFile(exePath);
const header = contents.subarray(0, 2);
if (header[0] !== 0x4d || header[1] !== 0x5a) {
  throw new Error("Windows installer is missing the PE MZ header.");
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
