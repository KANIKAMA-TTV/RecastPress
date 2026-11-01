"use strict";

// Schema-directed input reader. Only the fixed two object shapes, one array,
// string values and the literal version 1 are admitted; no recursive JSON walk.
((root) => {
  const MAX_INPUT_BYTES = 8 * 1024 * 1024;
  const MESSAGE = "入力 JSON を読み込めません。形式・文字・サイズを確認してください。";
  class InputError extends Error {
    constructor() { super(MESSAGE); this.name = "InputError"; }
  }
  const fail = () => { throw new InputError(); };
  const encoder = new TextEncoder();

  function wellFormed(value) {
    for (let i = 0; i < value.length; i += 1) {
      const code = value.charCodeAt(i);
      if (code >= 0xd800 && code <= 0xdbff) {
        const next = value.charCodeAt(++i);
        if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      } else if (code >= 0xdc00 && code <= 0xdfff) return false;
    }
    return true;
  }

  function parsePreparedInput(bytes) {
    try {
      if (!(bytes instanceof Uint8Array) || bytes.byteLength > MAX_INPUT_BYTES) fail();
      if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) fail();
      const text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
      let at = 0;
      const whitespace = () => {
        while (at < text.length && (text[at] === " " || text[at] === "\t" || text[at] === "\r" || text[at] === "\n")) at += 1;
      };
      const take = (character) => {
        whitespace();
        if (text[at] !== character) fail();
        at += 1;
      };
      const string = () => {
        take('"');
        let start = at;
        const parts = [];
        while (at < text.length) {
          const code = text.charCodeAt(at);
          if (code < 0x20) fail();
          if (text[at] === '"') {
            parts.push(text.slice(start, at));
            at += 1;
            const value = parts.join("");
            if (!wellFormed(value)) fail();
            return value;
          }
          if (text[at] === "\\") {
            parts.push(text.slice(start, at));
            at += 1;
            const escaped = text[at++];
            if (escaped === "u") {
              const hex = text.slice(at, at + 4);
              if (!/^[0-9a-fA-F]{4}$/u.test(hex)) fail();
              parts.push(String.fromCharCode(Number.parseInt(hex, 16)));
              at += 4;
            } else {
              const escapes = { '"': '"', "\\": "\\", "/": "/", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t" };
              if (!Object.hasOwn(escapes, escaped)) fail();
              parts.push(escapes[escaped]);
            }
            start = at;
          } else at += 1;
        }
        fail();
      };
      const object = (names, readValue) => {
        take("{");
        const seen = new Set();
        const values = Object.create(null);
        whitespace();
        if (text[at] === "}") fail();
        while (true) {
          const key = string();
          if (!names.includes(key) || seen.has(key)) fail();
          seen.add(key);
          take(":");
          values[key] = readValue(key);
          whitespace();
          if (text[at] === "}") { at += 1; break; }
          take(",");
        }
        if (seen.size !== names.length) fail();
        return values;
      };
      const seenTargets = new Set();
      const post = () => {
        const value = object(["target", "title", "body_text"], () => string());
        const { target, title, body_text } = value;
        if (!target.startsWith("/") || target.startsWith("//") || /[\u0000-\u0020\u007f#\\]/u.test(target) || encoder.encode(target).length > 8192) fail();
        const path = target.split("?", 1)[0];
        if (path === "/health/live" || path === "/health/ready" || seenTargets.has(target)) fail();
        if (title.includes("\0") || body_text.includes("\0")) fail();
        seenTargets.add(target);
        return { target, title, body_text };
      };
      const posts = () => {
        take("[");
        whitespace();
        // The workbench retains at least one article; no invented count maximum.
        if (text[at] === "]") fail();
        const values = [];
        while (true) {
          values.push(post());
          whitespace();
          if (text[at] === "]") { at += 1; return values; }
          take(",");
        }
      };
      const parsed = object(["format_version", "posts"], (key) => {
        if (key === "posts") return posts();
        take("1");
        return 1;
      });
      whitespace();
      if (at !== text.length) fail();
      return parsed.posts;
    } catch {
      // Decode/grammar failures never retain the source or engine diagnostics.
      throw new InputError();
    }
  }

  const api = Object.freeze({ parsePreparedInput, InputError, MAX_INPUT_BYTES });
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.PreparedPostInputParser = api;
})(globalThis);
