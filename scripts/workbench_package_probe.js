"use strict";

// Fixed local main-module fixture; no browser/network or arbitrary module.
const { webcrypto } = require("node:crypto");
// One sentinel byte beyond the API input cap is an intentional refusal fixture.
const MAX_WIRE = 8 * 1024 * 1024 + 1;
const MAX_OUTPUT = 16 * 1024 * 1024;
let chunks = [], size = 0;
function refuse() { process.stdout.write('{"ok":false,"error":"Package preparation failed"}\n'); }
process.stdin.on("data", (chunk) => {
  size += chunk.length;
  if (size <= MAX_WIRE) chunks.push(chunk);
});
process.stdin.on("error", () => { chunks = []; process.exitCode = 2; });
process.stdin.on("end", async () => {
  try {
    if (process.argv.length !== 2 || size > MAX_WIRE) throw new Error();
    const api = require("../tools/prepared-post-workbench/publication-package.js");
    let result;
    try { result = await api.preparePackage(Buffer.concat(chunks), webcrypto); }
    catch (error) {
      if (error instanceof api.PackageError) { refuse(); return; }
      throw new Error();
    }
    const output = JSON.stringify({ ok: true, archive_base64: Buffer.from(result.archiveBytes).toString("base64"),
      payloadBytes: result.payloadBytes, receiptBytes: result.receiptBytes,
      sha256: result.sha256, postCount: result.postCount }) + "\n";
    if (Buffer.byteLength(output) > MAX_OUTPUT) throw new Error();
    process.stdout.write(output);
  } catch { process.stderr.write("Workbench package probe failed\n"); process.exitCode = 2; }
});
