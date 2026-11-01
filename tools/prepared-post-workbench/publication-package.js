"use strict";

// Fixed development PreparedPosts renderer and two-member STORE archive.
// No file/network access, HTML interpretation, publish, generic ZIP API or crypto fallback.
((root) => {
  const parser = typeof module === "object" && module.exports
    ? require("./input-parser.js") : root.PreparedPostInputParser;
  const MAX_BUNDLE = 8 * 1024 * 1024;
  const MAX_RECEIPT = 1024;
  const MAX_ARCHIVE = MAX_BUNDLE + MAX_RECEIPT + 220;
  const MESSAGE = "公開用パッケージを生成できません。入力・サイズ・ブラウザーの暗号機能を確認してください。";
  class PackageError extends Error {
    constructor() { super(MESSAGE); this.name = "PackageError"; }
  }
  const fail = () => { throw new PackageError(); };
  const encoder = new TextEncoder();
  const BEFORE_TITLE = '<!doctype html>\n<html><head><meta charset="utf-8"><title>';
  const BEFORE_HEADING = '</title></head><body><main><article><h1>';
  const BEFORE_BODY = '</h1><div class="post-body" style="white-space:pre-wrap">';
  const AFTER_BODY = '</div></article></main></body></html>\n';
  const SHELL_BYTES = BEFORE_TITLE.length + BEFORE_HEADING.length + BEFORE_BODY.length + AFTER_BODY.length;
  const BUNDLE_START = '{"format_version":1,"pages":[';
  const PAGE_START = '{"target":';
  const PAGE_MIDDLE = ',"status":200,"content_type":"text/html; charset=utf-8","body":';
  const BUNDLE_END = ']}\n';
  const ESCAPES = Object.freeze({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;", "\r": "&#13;" });

  function add(a, b, limit) {
    const sum = a + b;
    if (!Number.isSafeInteger(sum) || sum > limit) fail();
    return sum;
  }

  // Caller strings already passed the strict parser's complete surrogate validation.
  function stringBytes(value, json = false, html = false) {
    let count = json ? 2 : 0;
    for (const scalar of value) {
      const code = scalar.codePointAt(0);
      let size;
      if (html && Object.hasOwn(ESCAPES, scalar)) size = ESCAPES[scalar].length;
      else if (json && (code === 0x22 || code === 0x5c || code === 8 || code === 9 || code === 10 || code === 12 || code === 13)) size = 2;
      else if (json && code < 0x20) size = 6;
      else size = code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4;
      count = add(count, size, MAX_BUNDLE);
    }
    return count;
  }

  function html(value) {
    return value.replace(/[&<>"'\r]/gu, (character) => ESCAPES[character]);
  }

  function payloadFromPosts(posts) {
    // Preflight every complete HTML body and their aggregate before HTML emission.
    let bodyTotal = 0;
    for (const post of posts) {
      let length = add(SHELL_BYTES, stringBytes(post.title, false, true), MAX_BUNDLE);
      length = add(length, stringBytes(post.title, false, true), MAX_BUNDLE);
      length = add(length, stringBytes(post.body_text, false, true), MAX_BUNDLE);
      bodyTotal = add(bodyTotal, length, MAX_BUNDLE);
    }
    const bodies = posts.map((post) => BEFORE_TITLE + html(post.title) + BEFORE_HEADING
      + html(post.title) + BEFORE_BODY + html(post.body_text) + AFTER_BODY);
    // JSON escaping has a distinct final wire cap, including the terminal LF.
    let size = BUNDLE_START.length + BUNDLE_END.length;
    posts.forEach((post, index) => {
      size = add(size, (index ? 1 : 0) + PAGE_START.length + PAGE_MIDDLE.length + 1, MAX_BUNDLE);
      size = add(size, stringBytes(post.target, true), MAX_BUNDLE);
      size = add(size, stringBytes(bodies[index], true), MAX_BUNDLE);
    });
    const wire = BUNDLE_START + posts.map((post, index) => PAGE_START + JSON.stringify(post.target)
      + PAGE_MIDDLE + JSON.stringify(bodies[index]) + "}").join(",") + BUNDLE_END;
    const payload = encoder.encode(wire);
    if (payload.length !== size || payload.length > MAX_BUNDLE) fail();
    return payload;
  }

  function crc32(bytes) {
    let crc = 0xffffffff;
    for (const byte of bytes) {
      crc ^= byte;
      for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    }
    return (crc ^ 0xffffffff) >>> 0;
  }

  // APPNOTE 4.3.7/4.3.12/4.3.16: fixed ASCII names, method 0, flags 0,
  // DOS 1980-01-01 00:00, no descriptors/extras/comments/Zip64/encryption.
  function archive(payload, receipt) {
    const total = add(add(payload.length, receipt.length, MAX_ARCHIVE), 220, MAX_ARCHIVE);
    const bytes = new Uint8Array(total);
    const view = new DataView(bytes.buffer);
    const members = [["bundle.json", payload], ["receipt.json", receipt]].map(([name, data]) => ({ name: encoder.encode(name), data, crc: crc32(data), offset: 0 }));
    let at = 0;
    const u16 = (value) => { view.setUint16(at, value, true); at += 2; };
    const u32 = (value) => { view.setUint32(at, value, true); at += 4; };
    const put = (value) => { bytes.set(value, at); at += value.length; };
    for (const member of members) {
      member.offset = at;
      u32(0x04034b50); u16(20); u16(0); u16(0); u16(0); u16(33);
      u32(member.crc); u32(member.data.length); u32(member.data.length); u16(member.name.length); u16(0);
      put(member.name); put(member.data);
    }
    const centralStart = at;
    for (const member of members) {
      u32(0x02014b50); u16(20); u16(20); u16(0); u16(0); u16(0); u16(33);
      u32(member.crc); u32(member.data.length); u32(member.data.length); u16(member.name.length);
      u16(0); u16(0); u16(0); u16(0); u32(0); u32(member.offset); put(member.name);
    }
    const centralSize = at - centralStart;
    u32(0x06054b50); u16(0); u16(0); u16(2); u16(2); u32(centralSize); u32(centralStart); u16(0);
    if (at !== total) fail();
    return bytes;
  }

  async function preparePackage(inputBytes, cryptoProvider = root.crypto) {
    try {
      // Parse synchronously before the first await; later input mutations cannot
      // change these private string values or the finalized payload being hashed.
      const posts = Object.freeze(parser.parsePreparedInput(inputBytes).map((post) => Object.freeze(post)));
      const payload = payloadFromPosts(posts);
      if (!cryptoProvider || !cryptoProvider.subtle || typeof cryptoProvider.subtle.digest !== "function") fail();
      const digest = await cryptoProvider.subtle.digest("SHA-256", payload);
      if (!(digest instanceof ArrayBuffer) || digest.byteLength !== 32) fail();
      const sha256 = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
      const receipt = encoder.encode(JSON.stringify({ format_version: 1, payload_bytes: payload.length, sha256 }) + "\n");
      if (receipt.length > MAX_RECEIPT) fail();
      return Object.freeze({ archiveBytes: archive(payload, receipt), payloadBytes: payload.length,
        receiptBytes: receipt.length, sha256, postCount: posts.length });
    } catch {
      // No causes, parser/crypto/engine diagnostics, content or caller paths.
      throw new PackageError();
    }
  }

  const api = Object.freeze({ preparePackage, PackageError });
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.PreparedPostPackage = api;
})(globalThis);
