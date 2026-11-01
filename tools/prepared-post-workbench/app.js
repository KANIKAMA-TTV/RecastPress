"use strict";

(() => {
  const MAX_WIRE = 8 * 1024 * 1024;
  const encoder = new TextEncoder();
  const fields = ["target", "title", "body_text"];
  const labels = { target: "target", title: "タイトル", body_text: "本文" };
  const elements = Object.fromEntries(fields.map((name) => [name, document.getElementById(name)]));
  const byId = (id) => document.getElementById(id);
  let nextId = 2;
  let posts = [{
    id: 1,
    target: "/prepared-post",
    title: "公開用サンプル <literal>",
    body_text: "\n先頭の改行を含む公開用の固定サンプルです。\n<strong>このタグも文字として表示します。</strong>\n引用符: \"sample\" & 'example'"
  }];
  let selectedId = 1;
  let lastDeletion = null;
  let importSequence = 0;
  let pendingImport = null;
  let packageEpoch = 0;
  let packageBusy = false;
  let preparedPackage = null;
  const urls = new Set();
  const selected = () => posts.find((post) => post.id === selectedId);
  const bytes = (value) => encoder.encode(value).length;
  const number = (value) => value.toLocaleString("ja-JP");

  function wellFormed(value) {
    for (let i = 0; i < value.length; i += 1) {
      const code = value.charCodeAt(i);
      if (code >= 0xd800 && code <= 0xdbff) {
        const next = value.charCodeAt(i + 1);
        if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
        i += 1;
      } else if (code >= 0xdc00 && code <= 0xdfff) return false;
    }
    return true;
  }

  function validate() {
    const errors = posts.map(() => ({ target: [], title: [], body_text: [] }));
    const seen = new Map();
    let allWellFormed = true;
    posts.forEach((post, index) => {
      fields.forEach((field) => {
        if (!wellFormed(post[field])) {
          errors[index][field].push("不正な UTF-16 surrogate を含みます。値を置換せず修正してください。");
          allWellFormed = false;
        }
      });
      if (!post.target.startsWith("/") || post.target.startsWith("//")) errors[index].target.push("単一の / で始めてください。");
      if (/[\u0000-\u0020\u007f#\\]/u.test(post.target)) errors[index].target.push("制御文字・空白・DEL・#・バックスラッシュは使用できません。");
      if (wellFormed(post.target) && bytes(post.target) > 8192) errors[index].target.push("8,192 UTF-8 bytes を超えています。");
      const path = post.target.split("?", 1)[0];
      if (path === "/health/live" || path === "/health/ready") errors[index].target.push("このパスは予約されています（query 付きも不可）。");
      if (seen.has(post.target)) {
        errors[index].target.push("同じ target の記事があります。完全一致の重複は出力できません。");
        const first = seen.get(post.target);
        if (!errors[first].target.includes("同じ target の記事があります。完全一致の重複は出力できません。")) errors[first].target.push("同じ target の記事があります。完全一致の重複は出力できません。");
      } else seen.set(post.target, index);
      for (const field of ["title", "body_text"]) {
        if (post[field].includes("\0")) errors[index][field].push("NUL は使用できません。");
      }
    });
    let wire = null;
    let wireBytes = null;
    if (allWellFormed) {
      wire = JSON.stringify({ format_version: 1, posts: posts.map(({ target, title, body_text }) => ({ target, title, body_text })) }) + "\n";
      wireBytes = bytes(wire);
    }
    const count = errors.reduce((sum, post) => sum + fields.reduce((n, field) => n + post[field].length, 0), 0);
    return { errors, wire, wireBytes, valid: count === 0 && wireBytes !== null && wireBytes <= MAX_WIRE };
  }

  function announce(message) { byId("status").textContent = message; }

  function invalidatePackage() {
    packageEpoch += 1;
    preparedPackage = null;
    byId("package-status").textContent = "現在の内容で ZIP を準備してください。入力 JSON の書出しも引き続き利用できます。";
    byId("package-download").disabled = true;
  }

  function packageControls(valid) {
    byId("package-prepare").disabled = packageBusy || !valid;
    byId("package-download").disabled = packageBusy || preparedPackage === null;
    byId("package-panel").setAttribute("aria-busy", String(packageBusy));
  }

  function renderList(result) {
    const focusedId = document.activeElement?.dataset.postId;
    const fragment = document.createDocumentFragment();
    posts.forEach((post, index) => {
      const li = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.className = "post-button";
      button.dataset.postId = String(post.id);
      button.setAttribute("aria-current", String(post.id === selectedId));
      const lines = [["post-number", `POST ${String(index + 1).padStart(2, "0")}`], ["post-title", post.title || "無題の記事"], ["post-target", post.target || "target 未入力"]];
      if (fields.some((field) => result.errors[index][field].length)) lines.push(["post-state", "入力の確認が必要"]);
      lines.forEach(([className, text]) => {
        const span = document.createElement("span");
        span.className = className;
        span.textContent = text;
        button.append(span);
      });
      button.addEventListener("click", () => selectPost(post.id));
      li.append(button);
      fragment.append(li);
    });
    byId("post-list").replaceChildren(fragment);
    if (focusedId) byId("post-list").querySelector(`[data-post-id="${Number(focusedId)}"]`)?.focus();
    byId("post-count").textContent = String(posts.length);
  }

  function render() {
    const post = selected();
    const result = validate();
    const index = posts.indexOf(post);
    fields.forEach((field) => {
      byId(`${field}-bytes`).textContent = wellFormed(post[field]) ? `${number(bytes(post[field]))} UTF-8 bytes` : "UTF-8 計算不可";
      byId(`${field}-error`).textContent = result.errors[index][field].join(" ");
      elements[field].setAttribute("aria-invalid", String(result.errors[index][field].length > 0));
    });
    byId("preview-target").textContent = post.target;
    byId("preview-title").textContent = post.title;
    byId("preview-body").textContent = post.body_text;
    byId("preview-empty").textContent = !post.title && !post.body_text ? "タイトルと本文は空欄です（出力可）。" : "";
    byId("delete-post").disabled = posts.length === 1;
    byId("undo-delete").disabled = lastDeletion === null;
    byId("download").disabled = !result.valid;
    packageControls(result.valid);
    byId("wire-bytes").textContent = result.wireBytes === null ? "JSON サイズ：不正な UTF-16 のため計算不可" : `JSON サイズ：${number(result.wireBytes)} / ${number(MAX_WIRE)} bytes（UTF-8・末尾改行を含む）`;
    const summary = byId("validation-summary");
    const summaryText = result.valid ? "補助チェックを通過しました。最終受理は CLI で確認してください。" : "ダウンロードできません。次の入力を確認してください。";
    if (summary.textContent !== summaryText) summary.textContent = summaryText;
    summary.classList.toggle("invalid", !result.valid);
    const errorList = document.createDocumentFragment();
    result.errors.forEach((postErrors, i) => fields.forEach((field) => postErrors[field].forEach((error) => {
      const li = document.createElement("li");
      li.textContent = `記事 ${i + 1} / ${labels[field]}：${error}`;
      errorList.append(li);
    })));
    if (result.wireBytes > MAX_WIRE) {
      const li = document.createElement("li");
      li.textContent = "JSON サイズが 8 MiB を超えています。記事の文字量を減らしてください。";
      errorList.append(li);
    }
    byId("validation-errors").replaceChildren(errorList);
    renderList(result);
  }

  function selectPost(id) {
    selectedId = id;
    fields.forEach((field) => { elements[field].value = selected()[field]; });
    render();
  }

  fields.forEach((field) => elements[field].addEventListener("input", () => {
    selected()[field] = elements[field].value;
    invalidatePackage();
    render();
    announce("入力を変更しました。現在の内容を残すには、もう一度 JSON へ書き出してください。");
  }));

  function clearImport() {
    pendingImport = null;
    byId("apply-import").disabled = true;
    byId("import-preview").hidden = true;
    byId("import-targets").replaceChildren();
  }

  function importStatus(message, invalid = false) {
    byId("import-status").textContent = message;
    byId("import-status").classList.toggle("invalid", invalid);
  }

  byId("import-file").addEventListener("change", async () => {
    invalidatePackage();
    const sequence = ++importSequence;
    clearImport();
    const file = byId("import-file").files?.[0];
    byId("cancel-import").disabled = !file;
    if (!file) { importStatus("ファイルは未選択です。"); return; }
    if (file.size > MAX_WIRE) { importStatus("8 MiB を超えるファイルは読み込めません。編集中の内容は保持しています。", true); return; }
    importStatus("ファイルを読み込んでいます。編集中の内容は保持しています。");
    try {
      const buffer = await file.arrayBuffer();
      if (sequence !== importSequence) return;
      const imported = PreparedPostInputParser.parsePreparedInput(new Uint8Array(buffer));
      const fragment = document.createDocumentFragment();
      imported.slice(0, 5).forEach((post) => {
        const li = document.createElement("li");
        li.textContent = post.target;
        fragment.append(li);
      });
      byId("import-targets").replaceChildren(fragment);
      pendingImport = imported;
      byId("import-preview").hidden = false;
      byId("apply-import").disabled = false;
      importStatus(`${number(imported.length)} 件を確認しました。先頭 ${number(Math.min(imported.length, 5))} 件の target を表示しています。まだ適用していません。`);
    } catch {
      if (sequence !== importSequence) return;
      clearImport();
      importStatus("入力を読み込めませんでした。UTF-8・schema 1・重複キー・文字や target の条件を確認してください。編集中の内容は保持しています。", true);
    }
  });

  byId("cancel-import").addEventListener("click", () => {
    invalidatePackage();
    importSequence += 1;
    clearImport();
    byId("import-file").value = "";
    byId("cancel-import").disabled = true;
    importStatus("読込みを取り消しました。編集中の内容は保持しています。");
  });

  byId("apply-import").addEventListener("click", () => {
    if (!pendingImport) return;
    invalidatePackage();
    const imported = pendingImport.map((post) => ({ id: nextId++, ...post }));
    posts = imported;
    lastDeletion = null;
    selectPost(posts[0].id);
    importSequence += 1;
    clearImport();
    byId("import-file").value = "";
    byId("cancel-import").disabled = true;
    importStatus(`${number(posts.length)} 件を記事一覧に適用しました。`);
    announce("読み込んだ内容を適用しました。編集後は JSON へ書き出してください。CLI 受理は未確認です。");
  });

  byId("add-post").addEventListener("click", () => {
    invalidatePackage();
    const post = { id: nextId++, target: "", title: "", body_text: "" };
    posts.push(post);
    selectPost(post.id);
    elements.target.focus();
    announce("空の記事を追加しました。target を明示してください。");
  });
  byId("delete-post").addEventListener("click", () => {
    if (posts.length === 1) return;
    invalidatePackage();
    const index = posts.indexOf(selected());
    lastDeletion = { post: posts[index], index };
    posts.splice(index, 1);
    selectPost(posts[Math.min(index, posts.length - 1)].id);
    announce("記事を削除しました。直前の削除を取り消せます。");
  });
  byId("undo-delete").addEventListener("click", () => {
    if (!lastDeletion) return;
    invalidatePackage();
    const { post, index } = lastDeletion;
    posts.splice(Math.min(index, posts.length), 0, post);
    lastDeletion = null;
    selectPost(post.id);
    announce("直前に削除した記事を元の位置に戻しました。");
  });

  function revoke(url) { URL.revokeObjectURL(url); urls.delete(url); }
  byId("download").addEventListener("click", () => {
    const result = validate();
    if (!result.valid) { render(); announce("入力を確認してください。ダウンロードは開始していません。"); return; }
    let url;
    let anchor;
    try {
      url = URL.createObjectURL(new Blob([result.wire], { type: "application/json;charset=utf-8" }));
      urls.add(url);
      anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "prepared-posts-input.json";
      document.body.append(anchor);
      anchor.click();
      announce("ダウンロードを開始しました。保存先・ファイル名をブラウザーで確認してください。CLI 受理は未確認です。");
    } catch {
      if (url) revoke(url);
      announce("ダウンロードを開始できませんでした。入力はタブ内に残っています。");
    } finally {
      anchor?.remove();
      if (url && urls.has(url)) window.setTimeout(() => revoke(url), 1000);
    }
  });
  byId("package-prepare").addEventListener("click", async () => {
    // WebCrypto cannot be cancelled: even an invalidated old job must settle
    // before another preparation begins. Editing and JSON export stay available.
    if (packageBusy) return;
    const result = validate();
    if (!result.valid) { render(); return; }
    invalidatePackage();
    const epoch = packageEpoch;
    const snapshot = encoder.encode(result.wire);
    packageBusy = true;
    packageControls(result.valid);
    byId("package-status").textContent = "ZIP を準備しています。入力の編集は続けられます。";
    try {
      const output = await PreparedPostPackage.preparePackage(snapshot, globalThis.crypto);
      if (epoch !== packageEpoch) return;
      preparedPackage = output;
      byId("package-status").textContent = `記事 ${number(output.postCount)} 件の ZIP を準備しました。ダウンロード後に解凍し、CLI で確認できます。`;
    } catch {
      if (epoch !== packageEpoch) return;
      byId("package-status").textContent = "ZIP を準備できませんでした。入力は保持しています。JSON を書き出し、CLI で準備する方法も利用できます。";
    } finally {
      packageBusy = false;
      packageControls(validate().valid);
    }
  });

  byId("package-download").addEventListener("click", () => {
    if (packageBusy || preparedPackage === null) return;
    let url;
    let anchor;
    try {
      url = URL.createObjectURL(new Blob([preparedPackage.archiveBytes], { type: "application/zip" }));
      urls.add(url);
      anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "prepared-post-package.zip";
      document.body.append(anchor);
      anchor.click();
      byId("package-status").textContent = "ZIP のダウンロードを開始しました。ブラウザーの保存先を確認してください。";
    } catch {
      if (url) revoke(url);
      byId("package-status").textContent = "ZIP のダウンロードを開始できませんでした。入力はタブ内に残っています。";
    } finally {
      anchor?.remove();
      if (url && urls.has(url)) window.setTimeout(() => revoke(url), 1000);
    }
  });
  window.addEventListener("pagehide", () => {
    packageEpoch += 1;
    preparedPackage = null;
    for (const url of urls) revoke(url);
  });
  selectPost(selectedId);
})();
