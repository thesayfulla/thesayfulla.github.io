// Telegraph-style writing page, built on Quill 2.
(function () {
  const initial = JSON.parse(document.getElementById("ed-initial").textContent);
  const csrf = document.querySelector("[name=csrfmiddlewaretoken]").value;
  const titleEl = document.getElementById("ed-title");
  const bodyWrap = document.querySelector(".ed-body-wrap");
  const side = document.getElementById("ed-side");
  const hint = document.getElementById("ed-hint");
  const publishBtn = document.getElementById("ed-publish");
  const deleteBtn = document.getElementById("ed-delete");
  const cancelBtn = document.getElementById("ed-cancel");
  const statusEl = document.getElementById("ed-status");
  const fileInput = document.getElementById("ed-file");
  const DRAFT_KEY = "draft:new-post";

  const esc = (s) =>
    String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

  // ---------- Embeds ----------
  function youtubeId(url) {
    const m = url.match(/(?:youtube\.com\/(?:watch\?(?:.*&)?v=|shorts\/|embed\/|live\/)|youtu\.be\/)([\w-]{6,})/);
    return m && m[1];
  }
  function vimeoId(url) {
    const m = url.match(/vimeo\.com\/(?:video\/)?(\d+)/);
    return m && m[1];
  }
  const isGist = (url) => /^https:\/\/gist\.github\.com\/[\w-]+\/[0-9a-f]+\/?$/.test(url);
  const isTweet = (url) => /^https:\/\/(?:www\.)?(?:x|twitter)\.com\/\w+\/status\/\d+/.test(url);
  const isKnownEmbed = (url) => !!(youtubeId(url) || vimeoId(url) || isGist(url) || isTweet(url));
  const isUrl = (s) => /^https?:\/\/\S+$/.test(s);

  // ---------- Custom blocks ----------
  const BlockEmbed = Quill.import("blots/block/embed");

  class FigureBlot extends BlockEmbed {
    static blotName = "figure";
    static tagName = "FIGURE";
    static className = "ed-figure";

    static create(value) {
      const node = super.create();
      node.setAttribute("contenteditable", "false");
      node.dataset.src = value.src;
      node.dataset.caption = value.caption || "";
      const img = document.createElement("img");
      img.src = value.src;
      const cap = document.createElement("input");
      cap.className = "ed-caption";
      cap.placeholder = "Caption (optional)";
      cap.value = value.caption || "";
      // Typing a caption updates the attribute, which Quill notices and saves.
      cap.addEventListener("input", () => (node.dataset.caption = cap.value));
      cap.addEventListener("keydown", (e) => {
        e.stopPropagation();
        if (e.key === "Enter") {
          e.preventDefault();
          const blot = Quill.find(node);
          if (blot) quill.setSelection(quill.getIndex(blot) + 1, 0, "user");
        }
      });
      cap.addEventListener("paste", (e) => e.stopPropagation());
      node.append(img, cap);
      return node;
    }
    static value(node) {
      return { src: node.dataset.src, caption: node.dataset.caption || "" };
    }
    html() {
      const v = FigureBlot.value(this.domNode);
      return `<figure><img src="${esc(v.src)}" alt="${esc(v.caption)}">${
        v.caption ? `<figcaption>${esc(v.caption)}</figcaption>` : ""
      }</figure>`;
    }
  }

  class EmbedBlot extends BlockEmbed {
    static blotName = "embed";
    static tagName = "DIV";
    static className = "ed-embed";

    static create(url) {
      const node = super.create();
      node.setAttribute("contenteditable", "false");
      node.dataset.url = url;
      const yt = youtubeId(url), vm = vimeoId(url);
      if (yt || vm) {
        node.classList.add("video");
        const iframe = document.createElement("iframe");
        iframe.src = yt ? `https://www.youtube-nocookie.com/embed/${yt}` : `https://player.vimeo.com/video/${vm}`;
        iframe.allowFullscreen = true;
        iframe.referrerPolicy = "strict-origin-when-cross-origin"; // YouTube refuses to play without a referrer
        node.append(iframe);
      } else {
        const label = isGist(url) ? "GitHub Gist" : isTweet(url) ? "Post on X" : "Embed";
        node.innerHTML = `<div class="ed-embed-card"><b>${label}</b><span>${esc(url)}</span></div>`;
      }
      return node;
    }
    static value(node) {
      return node.dataset.url;
    }
    html() {
      return `<div data-embed="${esc(this.domNode.dataset.url)}"></div>`;
    }
  }

  Quill.register(FigureBlot);
  Quill.register(EmbedBlot);

  // ---------- Getting out of blocks ----------
  // Code blocks, images and embeds can sit at the very top or bottom of a post
  // (or right next to each other) with no text line around them. These helpers
  // open an empty paragraph above/below such a block so you can keep writing.
  const Delta = Quill.import("delta");
  const BLOCK_TOPS = new Set(["code-block-container", "figure", "embed"]);

  const topOf = (line) => (line && line.statics.blotName === "code-block" ? line.parent : line);
  const isBlock = (top) => !!top && BLOCK_TOPS.has(top.statics.blotName);

  function openLineAt(index) {
    quill.updateContents(new Delta().retain(index).insert("\n"), "user");
    quill.setSelection(index, 0, "user");
  }
  function openLineAbove(top) { openLineAt(quill.getIndex(top)); }
  function openLineBelow(top) {
    const end = quill.getIndex(top) + top.length();
    if (end >= quill.getLength()) {
      quill.insertText(quill.getLength(), "\n", "user");
      quill.formatLine(quill.getLength() - 1, 1, { "code-block": false }, "user");
      quill.setSelection(quill.getLength() - 1, 0, "user");
    } else {
      openLineAt(end);
    }
  }

  // Is `line` the first (or last) line of a block with nothing writable before (after) it?
  function stuckAbove(line) {
    const top = topOf(line);
    if (!isBlock(top)) return null;
    if (line.statics.blotName === "code-block" && line.prev) return null;
    return !top.prev || isBlock(top.prev) ? top : null;
  }
  function stuckBelow(line) {
    const top = topOf(line);
    if (!isBlock(top)) return null;
    if (line.statics.blotName === "code-block" && line.next) return null;
    return !top.next || isBlock(top.next) ? top : null;
  }

  // ---------- Markdown-style shortcuts ----------
  const inCode = (ctx) => !!ctx.format["code-block"];
  const bindings = {
    escapeUp: {
      key: "ArrowUp",
      collapsed: true,
      handler(range, ctx) {
        const top = stuckAbove(ctx.line);
        if (!top) return true;
        openLineAbove(top);
        return false;
      },
    },
    escapeDown: {
      key: "ArrowDown",
      collapsed: true,
      handler(range, ctx) {
        const top = stuckBelow(ctx.line);
        if (!top) return true;
        openLineBelow(top);
        return false;
      },
    },
    codeTopEnter: {
      key: "Enter",
      collapsed: true,
      offset: 0,
      format: ["code-block"],
      handler(range, ctx) {
        const top = stuckAbove(ctx.line);
        if (!top) return true;
        openLineAbove(top);
        return false;
      },
    },
    mdHeading: {
      key: " ",
      collapsed: true,
      prefix: /^#{2,3}$/,
      handler(range, ctx) {
        if (inCode(ctx)) return true;
        const start = range.index - ctx.prefix.length;
        quill.deleteText(start, ctx.prefix.length, "user");
        quill.formatLine(start, 1, "header", ctx.prefix.length === 2 ? 3 : 4, "user");
        return false;
      },
    },
    mdQuote: {
      key: " ",
      collapsed: true,
      prefix: /^>$/,
      handler(range, ctx) {
        if (inCode(ctx)) return true;
        quill.deleteText(range.index - 1, 1, "user");
        quill.formatLine(range.index - 1, 1, "blockquote", true, "user");
        return false;
      },
    },
    mdCode: {
      key: "Enter",
      collapsed: true,
      prefix: /^```\w*$/,
      handler(range, ctx) {
        if (inCode(ctx)) return true;
        const lang = ctx.prefix.slice(3).toLowerCase();
        const start = range.index - ctx.prefix.length;
        quill.deleteText(start, ctx.prefix.length, "user");
        quill.formatLine(start, 1, "code-block", LANG_KEYS.has(lang) ? lang : "plain", "user");
        return false;
      },
    },
    linkEmbed: {
      key: "Enter",
      collapsed: true,
      handler(range, ctx) {
        if (inCode(ctx) || ctx.offset === 0) return true;
        const text = ctx.line.domNode.textContent.trim();
        const wanted = awaitingEmbed || isKnownEmbed(text);
        if (!isUrl(text) || !wanted || ctx.suffix) return true;
        const start = quill.getIndex(ctx.line);
        quill.deleteText(start, ctx.line.length() - 1, "user");
        quill.insertEmbed(start, "embed", text, "user");
        quill.setSelection(start + 1, 0, "user");
        awaitingEmbed = false;
        return false;
      },
    },
    linkShortcut: {
      key: "k",
      shortKey: true,
      handler() {
        kit.openLink();
        return false;
      },
    },
  };

  const LANGUAGES = [
    { key: "plain", label: "Plain" },
    { key: "python", label: "Python" },
    { key: "javascript", label: "JavaScript" },
    { key: "typescript", label: "TypeScript" },
    { key: "xml", label: "HTML" },
    { key: "css", label: "CSS" },
    { key: "bash", label: "Bash" },
    { key: "json", label: "JSON" },
    { key: "yaml", label: "YAML" },
    { key: "sql", label: "SQL" },
    { key: "go", label: "Go" },
    { key: "rust", label: "Rust" },
    { key: "java", label: "Java" },
    { key: "kotlin", label: "Kotlin" },
    { key: "swift", label: "Swift" },
    { key: "c", label: "C" },
    { key: "cpp", label: "C++" },
    { key: "csharp", label: "C#" },
    { key: "php", label: "PHP" },
    { key: "ruby", label: "Ruby" },
    { key: "diff", label: "Diff" },
  ];
  const LANG_KEYS = new Set(LANGUAGES.map((l) => l.key));

  const quill = new Quill("#ed-body", {
    placeholder: "Your story…",
    formats: ["bold", "italic", "link", "code", "header", "blockquote", "list", "code-block", "figure", "embed"],
    modules: {
      toolbar: false,
      syntax: { hljs: window.hljs, languages: LANGUAGES },
      keyboard: { bindings },
      clipboard: { matchVisual: false },
    },
  });

  // ---------- Load existing post / local draft ----------
  let dirty = false;
  if (initial.id) {
    titleEl.value = initial.title;
    if (initial.delta && initial.delta.ops) quill.setContents(initial.delta, "silent");
  } else {
    try {
      const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) || "null");
      if (draft) {
        titleEl.value = draft.title || "";
        if (draft.delta) quill.setContents(draft.delta, "silent");
      }
    } catch (e) {}
  }

  function autosize() {
    titleEl.style.height = "auto";
    titleEl.style.height = titleEl.scrollHeight + "px";
  }
  autosize();
  if (!titleEl.value) titleEl.focus();

  let draftTimer;
  function changed() {
    dirty = true;
    setStatus("");
    if (initial.id) return;
    clearTimeout(draftTimer);
    draftTimer = setTimeout(() => {
      try {
        localStorage.setItem(DRAFT_KEY, JSON.stringify({ title: titleEl.value, delta: quill.getContents() }));
      } catch (e) {}
    }, 400);
  }

  titleEl.addEventListener("input", () => { autosize(); changed(); });
  titleEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || (e.key === "ArrowDown" && titleEl.selectionStart === titleEl.value.length)) {
      e.preventDefault();
      goToStart();
    }
  });

  function goToStart() {
    const [first] = quill.getLine(0);
    const top = stuckAbove(first);
    if (top) openLineAbove(top);
    else quill.setSelection(0, 0, "user");
  }

  // Clicking the gap under the date, or the empty space under the last block.
  bodyWrap.addEventListener("mousedown", (e) => {
    if (e.target !== bodyWrap) return;
    if (e.clientY < quill.root.getBoundingClientRect().top) {
      e.preventDefault();
      goToStart();
    }
  });
  quill.root.addEventListener("mousedown", (e) => {
    if (e.target !== quill.root) return;
    const last = quill.root.lastElementChild;
    if (!last || e.clientY <= last.getBoundingClientRect().bottom) return;
    const top = stuckBelow(quill.getLines().pop());
    if (!top) return;
    e.preventDefault();
    openLineBelow(top);
  });
  quill.on("text-change", (_d, _o, source) => { if (source === "user") changed(); });
  window.addEventListener("beforeunload", (e) => { if (dirty && initial.id) e.preventDefault(); });

  // ---------- Side buttons (empty line) ----------
  let awaitingEmbed = false;
  let sideIndex = 0;

  function emptyLineAt(range) {
    if (!range || range.length) return null;
    const [line] = quill.getLine(range.index);
    if (!line || line.statics.blotName !== "block" || line.length() !== 1) return null;
    return line;
  }

  function placeAtLine(el, line) {
    const top = line.domNode.offsetTop;
    el.style.top = top + "px";
    el.hidden = false;
  }

  function updateSide(range) {
    const line = emptyLineAt(range);
    if (!line) {
      side.hidden = true;
      hint.hidden = true;
      awaitingEmbed = false;
      return;
    }
    sideIndex = range.index;
    if (awaitingEmbed) {
      side.hidden = true;
      placeAtLine(hint, line);
    } else {
      hint.hidden = true;
      placeAtLine(side, line);
    }
  }

  side.addEventListener("mousedown", (e) => e.preventDefault()); // keep editor focus
  side.addEventListener("click", (e) => {
    const btn = e.target.closest("button");
    if (!btn) return;
    const action = btn.dataset.action;
    if (action === "image") {
      fileInput.click();
    } else if (action === "embed") {
      awaitingEmbed = true;
      quill.setSelection(sideIndex, 0, "user");
      updateSide(quill.getSelection());
    } else if (action === "code") {
      quill.formatLine(sideIndex, 1, "code-block", "plain", "user");
      quill.setSelection(sideIndex, 0, "user");
    }
  });

  // ---------- Images ----------
  async function uploadImage(file, index) {
    if (!file.type.startsWith("image/")) return;
    setStatus("Uploading image…");
    const form = new FormData();
    form.append("file", file);
    try {
      const res = await fetch("/write/upload/", { method: "POST", body: form, headers: { "X-CSRFToken": csrf } });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Upload failed");
      const [line] = quill.getLine(index);
      // Replace the empty line we were on, otherwise insert after the current line
      // (never split a paragraph or code block in two).
      if (line && line.statics.blotName === "block" && line.length() === 1) {
        quill.insertEmbed(index, "figure", { src: data.url, caption: "" }, "user");
        quill.deleteText(index + 1, 1, "user");
      } else {
        // Inside a code block, go below the whole block, not just this line.
        const after = line && line.statics.blotName === "code-block" ? line.parent : line;
        index = after ? quill.getIndex(after) + after.length() : quill.getLength() - 1;
        quill.insertEmbed(index, "figure", { src: data.url, caption: "" }, "user");
      }
      if (quill.getLength() <= index + 1) quill.insertText(quill.getLength(), "\n", "user");
      quill.setSelection(index + 1, 0, "user");
      setStatus("");
    } catch (err) {
      setStatus(err.message, true);
    }
  }

  fileInput.addEventListener("change", () => {
    const file = fileInput.files[0];
    fileInput.value = "";
    if (file) uploadImage(file, sideIndex);
  });

  function filesFrom(dt) {
    return dt ? [...dt.files].filter((f) => f.type.startsWith("image/")) : [];
  }
  quill.root.addEventListener("paste", (e) => {
    const files = filesFrom(e.clipboardData);
    if (!files.length) return;
    e.preventDefault();
    e.stopPropagation();
    const range = quill.getSelection(true);
    files.forEach((f) => uploadImage(f, range.index));
  }, true);
  quill.root.addEventListener("drop", (e) => {
    const files = filesFrom(e.dataTransfer);
    if (!files.length) return;
    e.preventDefault();
    e.stopPropagation();
    const range = quill.getSelection(true) || { index: quill.getLength() - 1 };
    files.forEach((f) => uploadImage(f, range.index));
  }, true);

  // ---------- Toolbar for selected text + link card ----------
  const kit = EditorKit.attach(quill, {
    buttons: ["bold", "italic", "link", "code", "|", "h3", "h4", "blockquote"],
    hideIn: ["code-block"],
  });

  quill.on("editor-change", () => {
    const range = quill.getSelection();
    if (!range) return;
    updateSide(range);
  });
  quill.on("selection-change", (range) => {
    if (!range) side.hidden = true;
  });
  window.addEventListener("resize", () => {
    const range = quill.getSelection();
    if (range) updateSide(range);
  });

  // ---------- Publish / delete ----------
  function setStatus(text, isError) {
    statusEl.textContent = text;
    statusEl.classList.toggle("error", !!isError);
  }

  function semanticHtml() {
    // Quill turns spaces into &nbsp; here; the server normalises them back.
    return quill.getSemanticHTML();
  }

  async function publish() {
    const title = titleEl.value.trim();
    if (!title) { titleEl.focus(); return setStatus("Add a title first.", true); }
    if (quill.getLength() <= 1) { quill.focus(); return setStatus("Write something first.", true); }
    publishBtn.disabled = true;
    setStatus(initial.id ? "Saving…" : "Publishing…");
    try {
      const res = await fetch("/write/save/", {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-CSRFToken": csrf },
        body: JSON.stringify({ id: initial.id, title, delta: quill.getContents(), html: semanticHtml() }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Something went wrong.");
      dirty = false;
      try { localStorage.removeItem(DRAFT_KEY); } catch (e) {}
      window.location.href = data.url;
    } catch (err) {
      setStatus(err.message, true);
      publishBtn.disabled = false;
    }
  }
  publishBtn.addEventListener("click", publish);
  document.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") publish();
    if ((e.metaKey || e.ctrlKey) && e.key === "s") { e.preventDefault(); if (initial.id) publish(); }
  });

  // Leave without saving: back to the post when editing, otherwise to where you came from.
  cancelBtn.addEventListener("click", () => {
    if (initial.id) {
      if (dirty && !confirm("Discard your changes?")) return;
      dirty = false;
      window.location.href = initial.url;
      return;
    }
    const hasContent = titleEl.value.trim() || quill.getLength() > 1;
    if (hasContent && !confirm("Discard this draft?")) return;
    clearTimeout(draftTimer);
    try { localStorage.removeItem(DRAFT_KEY); } catch (e) {}
    dirty = false;
    let back = "/blog/";
    try {
      const ref = new URL(document.referrer);
      if (ref.origin === location.origin && !ref.pathname.startsWith("/write/") && !ref.pathname.startsWith("/login/")) {
        back = ref.pathname + ref.search;
      }
    } catch (e) {}
    window.location.href = back;
  });

  if (deleteBtn) {
    deleteBtn.addEventListener("click", async () => {
      if (!confirm("Delete this post? This can't be undone.")) return;
      const res = await fetch(`/write/${initial.id}/delete/`, { method: "POST", headers: { "X-CSRFToken": csrf } });
      const data = await res.json();
      dirty = false;
      if (res.ok) window.location.href = data.url;
      else setStatus(data.error || "Couldn't delete.", true);
    });
  }
})();
