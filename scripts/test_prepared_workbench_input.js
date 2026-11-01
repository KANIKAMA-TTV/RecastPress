"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { parsePreparedInput, InputError, MAX_INPUT_BYTES } = require("../tools/prepared-post-workbench/input-parser.js");
const encode = (text) => new TextEncoder().encode(text);
const row = (overrides = {}) => ({ target: "/chosen?b=2&a=1", title: "", body_text: "", ...overrides });
const wire = (posts = [row()]) => JSON.stringify({ format_version: 1, posts });
const read = (text) => parsePreparedInput(encode(text));
const refusal = (input) => assert.throws(() => typeof input === "string" ? read(input) : parsePreparedInput(input), (error) => {
  assert.equal(error.constructor, InputError);
  assert.equal(error.name, "InputError");
  assert.equal(error.message, "入力 JSON を読み込めません。形式・文字・サイズを確認してください。");
  assert.equal(error.cause, undefined);
  assert.equal(String(error).includes("PRIVATE_CANARY"), false);
  return true;
});

test("preserves schema values, empty fields, CR/LF, escapes and Unicode without IDs", () => {
  const expected = [row({ title: '日本 🦀 <script> & "', body_text: "\r\n\t<&copy;>\\\b\f/" }), row({ target: "/empty" })];
  assert.deepEqual(read(wire(expected)), expected);
  assert.deepEqual(read('{"posts":[{"body_text":"\\r","title":"","target":"/"}],"format_version":1}'), [{ target: "/", title: "", body_text: "\r" }]);
  assert.equal(Object.hasOwn(read(wire())[0], "id"), false);
  assert.deepEqual(read(wire([row({ target: "/a" }), row({ target: "/A" })])).map((post) => post.target), ["/a", "/A"]);
});

test("rejects duplicate decoded keys and unknown fields instead of silently discarding", () => {
  for (const text of [
    '{"format_version":1,"format_vers\\u0069on":1,"posts":[]}',
    '{"format_version":1,"posts":[{"target":"/","t\\u0061rget":"/x","title":"","body_text":""}]}',
    wire().replace('"title":""', '"title":"","password":"PRIVATE_CANARY"'),
    wire().replace('"posts":', '"private":"PRIVATE_CANARY","posts":'),
    wire().replace('"title":""', '"title":"","__proto__":"PRIVATE_CANARY"')
  ]) refusal(text);
});

test("fixed shapes refuse missing, null, scalar, nested/deep and trailing JSON", () => {
  for (const text of ["null", "[]", "1", "{}", '{"posts":[]}', '{"format_version":1,"posts":[]}',
    ...[0, 2, 1.0, true, null, "1"].filter((value) => value !== 1).map((value) => wire().replace('"format_version":1', `"format_version":${JSON.stringify(value)}`)),
    wire().replace('"format_version":1', '"format_version":1.0'),
    wire().replace('"format_version":1', '"format_version":1e0'),
    wire().replace('"format_version":1', '"format_version":01'),
    wire().replace('"posts":[', '"posts":['.repeat(2000)),
    wire().replace('"title":""', '"title":null'),
    wire().replace('"title":""', '"title":[]'),
    wire().replace('"title":"",', ""),
    wire() + "null", wire().slice(0, -1), wire().replace("}]", "},]"), wire().replace('"title":""', '"title":"",')
  ]) refusal(text);
});

test("strict string grammar, fatal UTF-8, BOM and surrogate rejection", () => {
  for (const fragment of ['"\\x41"', '"\\uXYZ0"', '"\\uD800"', '"\\uDC00"', '"\\uD800x"', '"a\nb"', '"\\"', '"PRIVATE_CANARY\\q"']) {
    refusal(wire().replace('"title":""', `"title":${fragment}`));
  }
  for (const bytes of [new Uint8Array([0xff]), new Uint8Array([0xc0, 0xaf]), new Uint8Array([0xed, 0xa0, 0x80]), new Uint8Array([0xf0, 0x9f]), new Uint8Array([0xef, 0xbb, 0xbf, ...encode(wire())])]) refusal(bytes);
  refusal("\ufeff" + wire());
  refusal(wire() + "\u00a0");
  assert.equal(read(wire([row({ title: "\ufeff" })]))[0].title, "\ufeff");
  assert.equal(read(wire().replace('"title":""', '"title":"\\ud83e\\udd80"'))[0].title, "🦀");
  refusal("not a Uint8Array");
  refusal(new ArrayBuffer(2));
  assert.throws(() => parsePreparedInput(wire()), InputError);
  assert.throws(() => parsePreparedInput(new DataView(new ArrayBuffer(2))), InputError);
});

test("target byte boundary and exact route restrictions; title/body NUL only", () => {
  const exact = "/" + "é".repeat(4095) + "a";
  assert.equal(encode(exact).length, 8192);
  assert.equal(read(wire([row({ target: exact })]))[0].target, exact);
  for (const target of [exact + "a", "", "a", "//a", "/a b", "/a\t", "/a\n", "/a\u007f", "/a#b", "/a\\b", "/\0", "/health/live", "/health/ready?x"]) refusal(wire([row({ target })]));
  refusal(wire([row(), row()]));
  for (const field of ["title", "body_text"]) refusal(wire([row({ [field]: "PRIVATE_CANARY\0" })]));
  assert.deepEqual(read(wire([row({ target: "/health/live/", body_text: "\u0001\r" })]))[0], row({ target: "/health/live/", body_text: "\u0001\r" }));
});

test("actual byte cap accepts exact 8 MiB and rejects one byte over", () => {
  const overhead = encode(wire()).length;
  const text = wire([row({ body_text: "x".repeat(MAX_INPUT_BYTES - overhead) })]);
  assert.equal(encode(text).length, MAX_INPUT_BYTES);
  assert.equal(read(text)[0].body_text.length, MAX_INPUT_BYTES - overhead);
  refusal(encode(text + " "));
  const unicode = wire([row({ body_text: "é".repeat(MAX_INPUT_BYTES / 2) })]);
  assert.ok(unicode.length < MAX_INPUT_BYTES);
  refusal(encode(unicode));
});
