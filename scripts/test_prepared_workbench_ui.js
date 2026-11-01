"use strict";

// Integration of the real app/parser with a small DOM/File/Blob boundary.
// This is not a browser/layout/download-delivery test. Textarea newline
// normalization is deliberately modeled to exercise imported CR preservation.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { webcrypto, createHash } = require("node:crypto");
const ROOT = path.resolve(__dirname, "..");
const TOOL = path.join(ROOT, "tools/prepared-post-workbench");
const PARSER = path.join(TOOL, "input-parser.js");
const PACKAGE = path.join(TOOL, "publication-package.js");

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const encode = (wire) => new TextEncoder().encode(wire);
const wire = (posts) => JSON.stringify({ format_version: 1, posts }) + "\n";
const row = (target, title = "", body_text = "") => ({ target, title, body_text });
function file(text) {
  const bytes = encode(text);
  return { name: "selected.json", size: bytes.length, arrayBuffer: async () => bytes.buffer };
}

function harness(options = {}) {
  const ids = new Map();
  const downloads = [];
  const blobs = new Map();
  const timers = [];
  const windowEvents = new Map();
  const packageCalls = [];
  const document = { activeElement: null };
  class Element {
    constructor(tag) {
      this.tagName = tag.toLowerCase();
      this.children = [];
      this.parentElement = null;
      this.dataset = {};
      this.attributes = new Map();
      this.events = new Map();
      this.disabled = false;
      this.hidden = false;
      this.files = [];
      this._value = "";
      this._text = "";
      const classes = new Set();
      this.classList = { toggle: (name, enabled) => {
        if (enabled) classes.add(name); else classes.delete(name);
      } };
    }
    set value(value) {
      this._value = this.tagName === "textarea" ? String(value).replace(/\r\n?/gu, "\n") : String(value);
      if (this.tagName === "input" && this.type === "file" && value === "") this.files = [];
    }
    get value() { return this._value; }
    set textContent(value) { this._text = String(value); this.children = []; }
    get textContent() { return this._text + this.children.map((child) => child.textContent).join(""); }
    set innerHTML(_) { throw new Error("User text must not pass through innerHTML"); }
    append(...nodes) {
      for (const node of nodes) {
        if (node.tagName === "#fragment") { this.append(...node.children); continue; }
        node.remove();
        node.parentElement = this;
        this.children.push(node);
      }
    }
    replaceChildren(...nodes) { this.children.forEach((child) => { child.parentElement = null; }); this.children = []; this._text = ""; this.append(...nodes); }
    remove() {
      if (this.parentElement) this.parentElement.children = this.parentElement.children.filter((child) => child !== this);
      this.parentElement = null;
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
      if (name === "id") { this.id = String(value); ids.set(this.id, this); }
      if (name === "type") this.type = String(value);
      if (name === "disabled") this.disabled = true;
      if (name === "hidden") this.hidden = true;
    }
    addEventListener(name, callback) {
      if (!this.events.has(name)) this.events.set(name, []);
      this.events.get(name).push(callback);
    }
    dispatch(name) { return Promise.all((this.events.get(name) || []).map((callback) => callback({ type: name, target: this }))); }
    click() {
      if (this.disabled) return;
      if (this.tagName === "a") {
        assert.ok(blobs.has(this.href), "download must reference a live owned Blob");
        downloads.push({ blob: blobs.get(this.href), filename: this.download });
      }
      return this.dispatch("click");
    }
    focus() { document.activeElement = this; }
    querySelector(selector) {
      const match = /^\[data-post-id="(\d+)"\]$/u.exec(selector);
      assert.ok(match, "fake DOM implements only the app's article selector");
      const visit = (node) => node.dataset.postId === match[1] ? node : node.children.map(visit).find(Boolean);
      return visit(this) || null;
    }
  }
  document.createElement = (tag) => new Element(tag);
  document.createDocumentFragment = () => new Element("#fragment");
  document.getElementById = (id) => {
    assert.ok(ids.has(id), `actual HTML must define #${id}`);
    return ids.get(id);
  };
  document.body = new Element("body");
  // Build element ancestry from the actual markup so hidden staging controls
  // cannot be treated as user-clickable by the fake integration.
  const stack = [document.body];
  const markup = fs.readFileSync(path.join(TOOL, "index.html"), "utf8");
  const voidTags = new Set(["meta", "link", "input", "br", "hr"]);
  for (const token of markup.matchAll(/<(\/?)([a-zA-Z][a-zA-Z0-9-]*)([^>]*)>/gu)) {
    const tag = token[2].toLowerCase();
    if (token[1]) {
      while (stack.length > 1) if (stack.pop().tagName === tag) break;
      continue;
    }
    const element = new Element(tag);
    for (const attribute of token[3].matchAll(/([^\s=]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/gu)) {
      element.setAttribute(attribute[1], attribute[2] ?? attribute[3] ?? attribute[4] ?? "");
    }
    stack.at(-1).append(element);
    if (tag === "body") document.body = element;
    if (!voidTags.has(tag)) stack.push(element);
  }
  const sandbox = vm.createContext({
    document, Blob, TextEncoder, TextDecoder, Uint8Array, ArrayBuffer, DataView,
    crypto: Object.hasOwn(options, "crypto") ? options.crypto : webcrypto,
    URL: { createObjectURL: (blob) => { const name = `blob:owned-${blobs.size + 1}`; blobs.set(name, blob); return name; }, revokeObjectURL: (url) => blobs.delete(url) },
    window: { setTimeout: (callback) => { timers.push(callback); return timers.length; }, addEventListener: (name, callback) => windowEvents.set(name, callback) },
  });
  vm.runInContext(fs.readFileSync(PARSER, "utf8"), sandbox, { filename: "input-parser.js" });
  vm.runInContext(fs.readFileSync(PACKAGE, "utf8"), sandbox, { filename: "publication-package.js" });
  const actualPrepare = sandbox.PreparedPostPackage.preparePackage;
  sandbox.PreparedPostPackage = { preparePackage: (bytes, provider) => {
    packageCalls.push(bytes.slice());
    return options.prepare ? options.prepare(bytes, provider, actualPrepare) : actualPrepare(bytes, provider);
  } };
  vm.runInContext(fs.readFileSync(path.join(TOOL, "app.js"), "utf8"), sandbox, { filename: "app.js" });
  const el = (id) => document.getElementById(id);
  const visible = (element) => {
    for (let node = element; node; node = node.parentElement) if (node.hidden) return false;
    return true;
  };
  async function click(id) {
    const element = el(id);
    assert.equal(visible(element), true, `${id} must be visible to the user`);
    assert.equal(element.disabled, false, `${id} must be enabled`);
    await element.click();
  }
  return {
    el, click, visible, downloads, blobs, packageCalls,
    input: async (id, value) => { el(id).value = value; await el(id).dispatch("input"); },
    choose: (selected) => {
      el("import-file").files = selected ? [selected] : [];
      el("import-file")._value = selected ? "C:\\fakepath\\selected.json" : "";
      return el("import-file").dispatch("change");
    },
    select: async (index) => { const button = el("post-list").children[index].children[0]; assert.ok(button); await button.click(); },
    exported: async () => {
      await click("download");
      const download = downloads.at(-1);
      assert.equal(download.filename, "prepared-posts-input.json");
      const text = await download.blob.text();
      assert.ok(text.endsWith("\n"));
      return { text, value: JSON.parse(text) };
    },
    dispose: () => { windowEvents.get("pagehide")?.(); for (const timer of timers) timer(); },
  };
}

test("failed reads and invalid files preserve current values and undo state", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  await ui.input("target", "/original");
  await ui.input("title", "Original private <literal>");
  await ui.click("add-post"); await ui.input("target", "/deleted"); await ui.click("delete-post");
  const before = (await ui.exported()).text;
  assert.equal(ui.el("undo-delete").disabled, false);
  await ui.choose({ name: "private.json", size: 1, arrayBuffer: async () => { throw new Error("PRIVATE_READ_DETAIL"); } });
  assert.equal((await ui.exported()).text, before);
  assert.equal(ui.el("undo-delete").disabled, false);
  await ui.choose(file('{"format_version":1,"posts":[{"target":"/invalid"}]}'));
  assert.equal((await ui.exported()).text, before);
  assert.equal(ui.el("apply-import").disabled, true);
  assert.equal(ui.el("import-status").textContent.includes("PRIVATE_READ_DETAIL"), false);
});

test("valid staging keeps edits until explicit apply then clears old undo and staging", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  await ui.input("target", "/original");
  await ui.click("add-post"); await ui.input("target", "/old-deleted"); await ui.click("delete-post");
  const imported = [row("/chosen?b=2&a=1", "日本 🦀", "\n<script>literal</script>"), row("/empty")];
  await ui.choose(file(wire(imported)));
  assert.equal(ui.el("import-preview").hidden, false);
  assert.equal(ui.el("import-targets").textContent, "/chosen?b=2&a=1/empty");
  assert.equal((await ui.exported()).value.posts[0].target, "/original");
  await ui.input("title", "edited while staged");
  assert.equal((await ui.exported()).value.posts[0].title, "edited while staged");
  await ui.click("apply-import");
  assert.deepEqual((await ui.exported()).value, { format_version: 1, posts: imported });
  assert.equal(ui.el("undo-delete").disabled, true);
  assert.equal(ui.el("apply-import").disabled, true);
  assert.equal(ui.el("import-preview").hidden, true);
  assert.equal(ui.el("import-file").value, "");
  const applied = (await ui.exported()).text;
  await ui.el("undo-delete").click(); await ui.el("apply-import").click();
  assert.equal((await ui.exported()).text, applied);
});

test("latest file change wins and stale success cannot replace its staging", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  const pending = deferred();
  const old = ui.choose({ size: 100, arrayBuffer: () => pending.promise });
  const latest = [row("/latest", "latest")];
  await ui.choose(file(wire(latest)));
  pending.resolve(encode(wire([row("/old")])).buffer);
  await old;
  assert.equal(ui.el("import-targets").textContent, "/latest");
  await ui.click("apply-import");
  assert.deepEqual((await ui.exported()).value.posts, latest);
});

test("stale rejected promise cannot clear a newer valid stage", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  const pending = deferred();
  const old = ui.choose({ size: 100, arrayBuffer: () => pending.promise });
  await ui.choose(file(wire([row("/newer")])));
  pending.reject(new Error("OLD_PRIVATE_ERROR")); await old;
  assert.equal(ui.el("apply-import").disabled, false);
  assert.equal(ui.el("import-targets").textContent, "/newer");
  await ui.click("apply-import");
  assert.equal((await ui.exported()).value.posts[0].target, "/newer");
});

test("visible pending cancel ignores late completion and keeps original data", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  const before = (await ui.exported()).text;
  const pending = deferred();
  const reading = ui.choose({ size: 100, arrayBuffer: () => pending.promise });
  assert.equal(ui.visible(ui.el("cancel-import")), true);
  await ui.click("cancel-import");
  pending.resolve(encode(wire([row("/cancelled")])).buffer); await reading;
  assert.equal((await ui.exported()).text, before);
  assert.equal(ui.el("apply-import").disabled, true);
  assert.equal(ui.el("import-preview").hidden, true);
  assert.equal(ui.el("import-file").value, "");
});

test("imported CR model survives selection and editing another field", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  const imported = [row("/cr?b=2&a=1", "title\rline", "\r\nfirst\rsecond\n\t日本 🦀 <b>literal</b>"), row("/other")];
  await ui.choose(file(wire(imported))); await ui.click("apply-import");
  assert.deepEqual((await ui.exported()).value.posts, imported);
  assert.equal(ui.el("body_text").value.includes("\r"), false, "textarea API normalizes displayed newlines");
  assert.equal(ui.el("preview-body").textContent, imported[0].body_text);
  await ui.select(1); await ui.input("title", "other changed"); await ui.select(0);
  await ui.input("target", "/cr?b=2&a=1&x=3");
  const exported = (await ui.exported()).value.posts;
  assert.equal(exported[0].body_text, imported[0].body_text);
  assert.equal(exported[0].title, imported[0].title);
  assert.equal(exported[0].target, "/cr?b=2&a=1&x=3");
  assert.equal(exported[1].title, "other changed");
});

test("editing invalidates download status and successful exports contain only source fields", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  await ui.exported();
  await ui.input("title", "new literal <script>&copy; 日本");
  assert.match(ui.el("status").textContent, /変更|書き出/u);
  assert.equal(ui.el("status").textContent.includes("ダウンロードを開始しました"), false);
  assert.equal(ui.el("preview-title").textContent, "new literal <script>&copy; 日本");
  const exported = (await ui.exported()).value;
  assert.deepEqual(Object.keys(exported).sort(), ["format_version", "posts"]);
  assert.deepEqual(Object.keys(exported.posts[0]).sort(), ["body_text", "target", "title"]);
});

// Only inspect the fixed two STORE local entries; the package module's own
// contracts validate the central directory and CRC. This checks what the real
// UI handed to Blob without running a browser or claiming delivery to disk.
function members(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const result = new Map();
  let at = 0;
  for (let i = 0; i < 2; i += 1) {
    assert.equal(view.getUint32(at, true), 0x04034b50);
    assert.equal(view.getUint16(at + 8, true), 0);
    const length = view.getUint32(at + 18, true);
    const nameLength = view.getUint16(at + 26, true);
    assert.equal(view.getUint16(at + 28, true), 0);
    const name = new TextDecoder().decode(bytes.subarray(at + 30, at + 30 + nameLength));
    at += 30 + nameLength;
    assert.equal(result.has(name), false);
    result.set(name, bytes.slice(at, at + length));
    at += length;
  }
  assert.deepEqual([...result.keys()], ["bundle.json", "receipt.json"]);
  assert.equal(view.getUint32(at, true), 0x02014b50);
  return result;
}

test("real package preparation downloads one exact ZIP Blob with matching payload and receipt", async (t) => {
  let produced;
  const ui = harness({ prepare: async (bytes, provider, actual) => { produced = await actual(bytes, provider); return produced; } });
  t.after(ui.dispose);
  const imported = [row("/zip?x=1&y=2", "日本 <&\"'🦀", "\r\n<script>x</script> & '\"\t"), row("/second", "", "")];
  await ui.choose(file(wire(imported))); await ui.click("apply-import");
  const input = (await ui.exported()).text;
  await ui.click("package-prepare");
  assert.equal(ui.el("package-download").disabled, false);
  assert.match(ui.el("package-status").textContent, /ダウンロード後に解凍/u);
  assert.equal(new TextDecoder().decode(ui.packageCalls[0]), input);
  await ui.click("package-download");
  assert.equal(ui.downloads.length, 2, "one JSON download and one ZIP, not two independent package members");
  const downloaded = ui.downloads.at(-1);
  assert.equal(downloaded.filename, "prepared-post-package.zip");
  assert.equal(downloaded.blob.type, "application/zip");
  const archive = new Uint8Array(await downloaded.blob.arrayBuffer());
  assert.deepEqual(archive, produced.archiveBytes, "UI Blob must contain the returned byte snapshot exactly");
  const entries = members(archive);
  const payload = entries.get("bundle.json");
  const bundle = JSON.parse(new TextDecoder().decode(payload));
  const receipt = JSON.parse(new TextDecoder().decode(entries.get("receipt.json")));
  const title = "日本 &lt;&amp;&quot;&#39;🦀";
  assert.equal(bundle.pages[0].body, '<!doctype html>\n<html><head><meta charset="utf-8"><title>' + title + '</title></head><body><main><article><h1>' + title + '</h1><div class="post-body" style="white-space:pre-wrap">&#13;\n&lt;script&gt;x&lt;/script&gt; &amp; &#39;&quot;\t</div></article></main></body></html>\n');
  assert.deepEqual(bundle.pages.map((page) => page.target), imported.map((post) => post.target));
  assert.equal(bundle.pages[0].status, 200);
  assert.equal(bundle.pages[0].content_type, "text/html; charset=utf-8");
  assert.deepEqual(Object.keys(receipt).sort(), ["format_version", "payload_bytes", "sha256"]);
  assert.equal(receipt.format_version, 1);
  assert.equal(receipt.payload_bytes, payload.length);
  assert.equal(receipt.sha256, createHash("sha256").update(payload).digest("hex"));
  assert.match(ui.el("package-status").textContent, /開始/u);
  assert.match(ui.el("package-status").textContent, /保存先を確認/u);
  assert.equal((await ui.exported()).text, input, "ZIP preparation never replaces source JSON");
  ui.dispose();
  assert.equal(ui.blobs.size, 0, "pagehide/timers release every owned ObjectURL");
});

test("selection alone keeps the prepared package, while every data and import action invalidates it", async (t) => {
  const actions = [
    async (ui) => ui.input("title", "changed"),
    async (ui) => ui.click("add-post"),
    async (ui) => ui.click("delete-post"),
    async (ui) => ui.click("undo-delete"),
    async (ui) => ui.choose(file(wire([row("/new")]))),
    async (ui) => { await ui.choose(file(wire([row("/new")]))); await ui.click("cancel-import"); },
    async (ui) => { await ui.choose(file(wire([row("/new")]))); await ui.click("apply-import"); },
  ];
  for (let index = 0; index < actions.length; index += 1) {
    const ui = harness(); t.after(ui.dispose);
    await ui.choose(file(wire([row("/first"), row("/second"), row("/third")]))); await ui.click("apply-import");
    await ui.select(2); await ui.click("delete-post");
    await ui.click("package-prepare");
    await ui.select(0);
    assert.equal(ui.el("package-download").disabled, false, "selection changes no exported field");
    await actions[index](ui);
    assert.equal(ui.el("package-download").disabled, true, `mutation ${index} must discard the old package`);
    const count = ui.downloads.length;
    await ui.el("package-download").dispatch("click");
    assert.equal(ui.downloads.length, count, "even a direct handler dispatch cannot download discarded bytes");
  }
});

test("edits during preparation keep one active job and discard its stale success before retry", async (t) => {
  const pending = deferred();
  let first = true;
  const ui = harness({ prepare: async (bytes, provider, actual) => {
    if (first) { first = false; await pending.promise; }
    return actual(bytes, provider);
  } });
  t.after(ui.dispose);
  const preparing = ui.click("package-prepare");
  assert.equal(ui.el("package-panel").attributes.get("aria-busy"), "true");
  assert.equal(ui.el("package-prepare").disabled, true);
  await ui.input("title", "new generation");
  const currentStatus = ui.el("package-status").textContent;
  await ui.el("package-prepare").dispatch("click");
  assert.equal(ui.packageCalls.length, 1, "an invalidated WebCrypto job cannot overlap another job");
  assert.equal(ui.el("download").disabled, false, "manual JSON fallback stays available while pending");
  assert.equal((await ui.exported()).value.posts[0].title, "new generation");
  pending.resolve(); await preparing;
  assert.equal(ui.el("package-status").textContent, currentStatus);
  assert.equal(ui.el("package-download").disabled, true);
  assert.equal(ui.el("package-prepare").disabled, false);
  assert.equal(ui.el("package-panel").attributes.get("aria-busy"), "false");
  await ui.click("package-prepare"); await ui.click("package-download");
  const bundle = JSON.parse(new TextDecoder().decode(members(new Uint8Array(await ui.downloads.at(-1).blob.arrayBuffer())).get("bundle.json")));
  assert.ok(bundle.pages[0].body.includes("new generation"));
  assert.equal(ui.packageCalls.length, 2);
});

test("new preparation immediately discards a previous ready ZIP and stale failure cannot overwrite imported state", async (t) => {
  const pending = deferred();
  let calls = 0;
  const ui = harness({ prepare: async (bytes, provider, actual) => {
    calls += 1;
    if (calls === 2) { await pending.promise; throw new Error("PRIVATE_OLD_CRYPTO_DETAIL"); }
    return actual(bytes, provider);
  } });
  t.after(ui.dispose);
  await ui.click("package-prepare");
  assert.equal(ui.el("package-download").disabled, false);
  const preparing = ui.click("package-prepare");
  assert.equal(ui.el("package-download").disabled, true);
  await ui.choose(file(wire([row("/imported", "import wins", "\rkeep")]))); await ui.click("apply-import");
  const status = ui.el("package-status").textContent;
  pending.resolve(); await preparing;
  assert.equal(ui.el("package-status").textContent, status);
  assert.equal(ui.el("package-download").disabled, true);
  assert.deepEqual((await ui.exported()).value.posts, [row("/imported", "import wins", "\rkeep")]);
  assert.equal(ui.el("package-status").textContent.includes("PRIVATE"), false);
});

test("missing or rejected WebCrypto is closed and preserves manual JSON export without private diagnostics", async (t) => {
  for (const provider of [undefined, { subtle: { digest: async () => { throw new Error("PRIVATE_CRYPTO_DETAIL"); } } }]) {
    const ui = harness({ crypto: provider }); t.after(ui.dispose);
    const before = (await ui.exported()).text;
    await ui.click("package-prepare");
    assert.equal(ui.el("package-download").disabled, true);
    assert.equal(ui.el("package-prepare").disabled, false);
    assert.match(ui.el("package-status").textContent, /JSON/u);
    assert.equal(ui.el("package-status").textContent.includes("PRIVATE"), false);
    assert.equal((await ui.exported()).text, before);
  }
});

test("invalid input never starts packaging and pagehide invalidates an unresolved job", async (t) => {
  const ui = harness(); t.after(ui.dispose);
  await ui.input("target", "/health/ready");
  assert.equal(ui.el("package-prepare").disabled, true);
  await ui.el("package-prepare").dispatch("click");
  assert.equal(ui.packageCalls.length, 0);
  const pending = deferred();
  const active = harness({ prepare: async (bytes, provider, actual) => { await pending.promise; return actual(bytes, provider); } });
  t.after(active.dispose);
  const preparing = active.click("package-prepare");
  const status = active.el("package-status").textContent;
  active.dispose(); pending.resolve(); await preparing;
  assert.equal(active.el("package-status").textContent, status);
  assert.equal(active.el("package-download").disabled, true);
  assert.equal(active.downloads.length, 0);
});
