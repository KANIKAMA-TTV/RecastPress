"use strict";

// Finite local fixtures only. Native admission/byte comparison is a separate gate.
const test = require("node:test");
const assert = require("node:assert/strict");
const { webcrypto, createHash } = require("node:crypto");
const { crc32 } = require("node:zlib");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const api = require("../tools/prepared-post-workbench/publication-package.js");
const parser = require("../tools/prepared-post-workbench/input-parser.js");
const encoder = new TextEncoder();
const MAX = 8 * 1024 * 1024;
const post = (body_text = "", title = "", target = "/a?b=2&a=1") => ({ target, title, body_text });
const input = (posts = [post()]) => encoder.encode(JSON.stringify({ format_version: 1, posts }) + "\n");
const prepare = (posts) => api.preparePackage(input(posts), webcrypto);

function unpack(bytes) {
  const data = Buffer.from(bytes);
  const end = data.length - 22;
  assert.equal(data.readUInt32LE(end), 0x06054b50);
  assert.equal(data.readUInt16LE(end + 4), 0);
  assert.equal(data.readUInt16LE(end + 6), 0);
  assert.equal(data.readUInt16LE(end + 8), 2);
  assert.equal(data.readUInt16LE(end + 10), 2);
  assert.equal(data.readUInt16LE(end + 20), 0);
  const central = data.readUInt32LE(end + 16);
  assert.equal(central + data.readUInt32LE(end + 12), end);
  let at = central;
  let expectedLocal = 0;
  const files = new Map();
  for (const expectedName of ["bundle.json", "receipt.json"]) {
    assert.equal(data.readUInt32LE(at), 0x02014b50);
    assert.equal(data.readUInt16LE(at + 4), 20);
    assert.equal(data.readUInt16LE(at + 6), 20);
    assert.equal(data.readUInt16LE(at + 8), 0); // flags: no encryption/descriptor
    assert.equal(data.readUInt16LE(at + 10), 0); // STORE
    assert.equal(data.readUInt16LE(at + 12), 0);
    assert.equal(data.readUInt16LE(at + 14), 33); // DOS 1980-01-01
    for (const offset of [30, 32, 34, 36]) assert.equal(data.readUInt16LE(at + offset), 0);
    assert.equal(data.readUInt32LE(at + 38), 0);
    const length = data.readUInt32LE(at + 20);
    assert.equal(length, data.readUInt32LE(at + 24));
    const nameLength = data.readUInt16LE(at + 28);
    assert.equal(data.subarray(at + 46, at + 46 + nameLength).toString(), expectedName);
    const local = data.readUInt32LE(at + 42);
    assert.equal(local, expectedLocal);
    assert.equal(data.readUInt32LE(local), 0x04034b50);
    assert.equal(data.readUInt16LE(local + 4), 20);
    assert.equal(data.readUInt16LE(local + 6), 0);
    assert.equal(data.readUInt16LE(local + 8), 0);
    assert.equal(data.readUInt16LE(local + 10), 0);
    assert.equal(data.readUInt16LE(local + 12), 33);
    assert.equal(data.readUInt32LE(local + 18), length);
    assert.equal(data.readUInt32LE(local + 22), length);
    assert.equal(data.readUInt16LE(local + 26), nameLength);
    assert.equal(data.readUInt16LE(local + 28), 0);
    assert.equal(data.subarray(local + 30, local + 30 + nameLength).toString(), expectedName);
    const bodyStart = local + 30 + nameLength;
    const body = data.subarray(bodyStart, bodyStart + length);
    assert.equal(body.length, length);
    // Independent native zlib implementation, including the standard CRC vector.
    assert.equal(data.readUInt32LE(at + 16), crc32(body));
    assert.equal(data.readUInt32LE(local + 14), crc32(body));
    files.set(expectedName, body);
    expectedLocal = bodyStart + length;
    at += 46 + nameLength;
  }
  assert.equal(expectedLocal, central);
  assert.equal(at, end);
  return files;
}

function contents(result) {
  assert.deepEqual(Object.keys(result), ["archiveBytes", "payloadBytes", "receiptBytes", "sha256", "postCount"]);
  const files = unpack(result.archiveBytes);
  const payload = files.get("bundle.json");
  const receiptBytes = files.get("receipt.json");
  const receipt = JSON.parse(receiptBytes);
  assert.equal(payload.at(-1), 10);
  assert.equal(receiptBytes.at(-1), 10);
  assert.equal(result.payloadBytes, payload.length);
  assert.equal(result.receiptBytes, receiptBytes.length);
  assert.equal(result.sha256, createHash("sha256").update(payload).digest("hex"));
  assert.deepEqual(receipt, { format_version: 1, payload_bytes: payload.length, sha256: result.sha256 });
  assert.equal(result.archiveBytes.length, payload.length + receiptBytes.length + 220);
  return { payload, bundle: JSON.parse(payload) };
}

async function refuses(value, provider = webcrypto) {
  await assert.rejects(api.preparePackage(value, provider), (error) => {
    assert.ok(error instanceof api.PackageError);
    assert.equal(error.name, "PackageError");
    assert.equal(error.message, "公開用パッケージを生成できません。入力・サイズ・ブラウザーの暗号機能を確認してください。");
    assert.equal(Object.hasOwn(error, "cause"), false);
    return true;
  });
}

test("fixed renderer, six literal escapes, leading LF/CR and ordered exact targets", async () => {
  const result = await prepare([post('\n日本語😀\r\n<script>&\"\'\t', 'T<&>\"\'\r'), post("", "", "/empty?")]);
  const { payload, bundle } = contents(result);
  const expected = '<!doctype html>\n<html><head><meta charset="utf-8"><title>T&lt;&amp;&gt;&quot;&#39;&#13;</title></head><body><main><article><h1>T&lt;&amp;&gt;&quot;&#39;&#13;</h1><div class="post-body" style="white-space:pre-wrap">\n日本語😀&#13;\n&lt;script&gt;&amp;&quot;&#39;\t</div></article></main></body></html>\n';
  assert.deepEqual(bundle.pages.map((page) => page.target), ["/a?b=2&a=1", "/empty?"]);
  assert.equal(bundle.pages[0].body, expected);
  assert.equal(result.postCount, 2);
  assert.equal(payload.toString(), JSON.stringify({ format_version: 1, pages: [
    { target: "/a?b=2&a=1", status: 200, content_type: "text/html; charset=utf-8", body: expected },
    bundle.pages[1],
  ] }) + "\n");
});

test("all non-NUL ASCII controls and Unicode remain exact text / JSON data", async () => {
  const raw = Array.from({ length: 31 }, (_, index) => String.fromCharCode(index + 1)).join("") + "\x7f\u0085\u2028\u2029é😀";
  const { bundle } = contents(await prepare([post(raw)]));
  const body = bundle.pages[0].body;
  assert.ok(body.includes(raw.replace(/\r/gu, "&#13;")));
  assert.ok(!body.includes("\ufffd"));
  assert.ok(body.includes("\u2028\u2029"));
});

test("strict input rejects invalid bytes/surrogates/NUL/shapes/routes before digest", async () => {
  let calls = 0;
  const provider = { subtle: { digest() { calls += 1; throw Error("PRIVATE"); } } };
  const bad = [null, [], new Uint8Array([0xff]), new Uint8Array([0xef, 0xbb, 0xbf, 0x7b]),
    encoder.encode('{"format_version":1,"format_version":1,"posts":[]}'), input([]),
    input([post("\ud800")]), input([post("\0")]), input([post("", "", "/health/ready?x")]),
    input([post("", "", "//bad")]), input([post("", "", "/#bad")]),
    input([post(), post()]), encoder.encode('{"format_version":1,"posts":[{"target":"/a","title":"","body_text":"","extra":1}]}')];
  for (const value of bad) await refuses(value, provider);
  assert.equal(calls, 0);
});

test("input 8MiB exact padding accepted, sentinel rejected without digest", async () => {
  const basic = input();
  const exact = new Uint8Array(MAX).fill(32);
  exact.set(basic);
  contents(await api.preparePackage(exact, webcrypto));
  let calls = 0;
  await refuses(new Uint8Array(MAX + 1).fill(32), { subtle: { digest() { calls += 1; } } });
  assert.equal(calls, 0);
});

test("rendered single-body and aggregate limits precede oversized output and crypto", async () => {
  let calls = 0;
  const provider = { subtle: { digest() { calls += 1; } } };
  await refuses(input([post("&".repeat(Math.floor(MAX / 5) + 1))]), provider);
  const half = "&".repeat(Math.floor(MAX / 10));
  await refuses(input([post(half), post(half, "", "/b")]), provider);
  assert.equal(calls, 0);
});

test("complete bundle cap includes JSON shell and LF: exact 8MiB and plus one", async () => {
  const empty = await prepare([post()]);
  const count = MAX - empty.payloadBytes;
  const exact = await prepare([post("x".repeat(count))]);
  assert.equal(contents(exact).payload.length, MAX);
  let calls = 0;
  await refuses(input([post("x".repeat(count + 1))]), { subtle: { digest() { calls += 1; } } });
  assert.equal(calls, 0);
});

test("same finalized bytes are hashed once; input mutation during await cannot change package", async () => {
  const original = input([post("PUBLIC snapshot")]);
  let release;
  let captured;
  let calls = 0;
  const provider = { subtle: { digest(algorithm, bytes) {
    calls += 1; assert.equal(algorithm, "SHA-256"); captured = Buffer.from(bytes);
    // Use an exact ArrayBuffer independent of Node Buffer's optional slab.
    return new Promise((resolve) => { release = () => resolve(Uint8Array.from(createHash("sha256").update(bytes).digest()).buffer); });
  } } };
  const pending = api.preparePackage(original, provider);
  original.fill(0);
  release();
  const result = await pending;
  assert.equal(calls, 1);
  assert.deepEqual(contents(result).payload, captured);
  assert.ok(captured.includes(Buffer.from("PUBLIC snapshot")));
});

test("WebCrypto missing, rejection, wrong digest size/type remain opaque with no fallback", async () => {
  for (const provider of [null, {}, { subtle: {} }, { subtle: { digest() { throw Error("PRIVATE secret"); } } },
    { subtle: { digest: async () => { throw Error("PRIVATE promise"); } } },
    { subtle: { digest: async () => new ArrayBuffer(31) } },
    { subtle: { digest: async () => new Uint8Array(32) } }]) await refuses(input(), provider);
});

test("fixed STORE archive is deterministic with both independent CRCs and standard vector", async () => {
  assert.equal(crc32(Buffer.from("123456789")), 0xcbf43926);
  const first = await prepare([post("CRC public fixture")]);
  const second = await prepare([post("CRC public fixture")]);
  contents(first);
  assert.deepEqual(first.archiveBytes, second.archiveBytes);
  assert.ok(Object.isFrozen(first));
  assert.equal(first.archiveBytes.length, first.payloadBytes + first.receiptBytes + 220);
});

test("browser global API matches CommonJS without DOM, network or storage", async () => {
  const context = { TextEncoder, Uint8Array, ArrayBuffer, DataView, PreparedPostInputParser: parser, crypto: webcrypto };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "../tools/prepared-post-workbench/publication-package.js"), "utf8"), context);
  assert.deepEqual(Buffer.from((await context.PreparedPostPackage.preparePackage(input())).archiveBytes), Buffer.from((await prepare([post()])).archiveBytes));
  assert.equal(typeof context.PreparedPostPackage.preparePackage, "function");
});
