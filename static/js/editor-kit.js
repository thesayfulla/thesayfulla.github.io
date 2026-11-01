// Shared editor UI for Quill: a floating toolbar for selected text and a
// link card that opens when you hover a link or put the cursor in one.
//
//   const kit = EditorKit.attach(quill, { buttons: ["bold", "italic", "link"] });
//   kit.openLink();  // e.g. from a ⌘K shortcut
(function () {
  const ICONS = {
    bold: "<b>B</b>",
    italic: "<i>i</i>",
    link: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/></svg>',
    code: '<span class="mono">&lt;/&gt;</span>',
    h3: '<span class="t-big">T</span>',
    h4: '<span class="t-small">T</span>',
    blockquote: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M6 17c-1.7 0-3-1.3-3-3 0-3.3 2-6 5-7l.6 1.2C6.8 9 6 10.3 6 11.5c.2 0 .5-.1.8-.1 1.6 0 2.7 1.2 2.7 2.8S8.1 17 6 17zm9 0c-1.7 0-3-1.3-3-3 0-3.3 2-6 5-7l.6 1.2C15.8 9 15 10.3 15 11.5c.2 0 .5-.1.8-.1 1.6 0 2.7 1.2 2.7 2.8S17.1 17 15 17z"/></svg>',
  };
  const TITLES = {
    bold: "Bold (⌘B)", italic: "Italic (⌘I)", link: "Link (⌘K)", code: "Inline code",
    h3: "Heading", h4: "Subheading", blockquote: "Quote",
  };
  const INLINE_FORMATS = ["bold", "italic", "code", "underline", "strike"];

  function el(html) {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  }

  function normalizeUrl(url) {
    url = url.trim();
    if (!url) return "";
    if (/^[^\s@/]+@[^\s@]+\.[^\s@]+$/.test(url)) return "mailto:" + url;
    if (!/^(https?:|mailto:|\/|#)/i.test(url)) return "https://" + url;
    return url;
  }

  function placeNear(node, quill, index, length, where) {
    node.hidden = false;
    const b = quill.getBounds(index, length);
    const r = quill.root.getBoundingClientRect();
    const w = node.offsetWidth;
    let left = r.left + window.scrollX + b.left + b.width / 2 - w / 2;
    left = Math.max(8, Math.min(left, document.documentElement.clientWidth - w - 8));
    const top = where === "above"
      ? r.top + window.scrollY + b.top - node.offsetHeight - 10
      : r.top + window.scrollY + b.top + b.height + 8;
    node.style.left = left + "px";
    node.style.top = top + "px";
  }

  function attach(quill, options = {}) {
    const buttons = options.buttons || ["bold", "italic", "link"];
    const hideIn = options.hideIn || [];

    // ---------- Toolbar for selected text ----------
    const bar = el(`<div class="ek-bar" hidden></div>`);
    buttons.forEach((name) => {
      if (name === "|") return bar.append(el('<span class="ek-sep"></span>'));
      bar.append(el(`<button type="button" data-format="${name}" title="${TITLES[name]}" aria-label="${TITLES[name]}">${ICONS[name]}</button>`));
    });
    document.body.append(bar);

    // ---------- Link card ----------
    const card = el(`
      <div class="ek-link" hidden>
        <label><span>Text</span><input class="ek-link-text" type="text" spellcheck="false"></label>
        <label><span>Link</span><input class="ek-link-url" type="text" placeholder="https://… or email" spellcheck="false" autocapitalize="none"></label>
        <div class="ek-link-actions">
          <button type="button" class="ek-save">Save</button>
          <a class="ek-open" target="_blank" rel="noopener noreferrer">Open ↗</a>
          <button type="button" class="ek-remove">Remove link</button>
        </div>
      </div>`);
    document.body.append(card);
    const textInput = card.querySelector(".ek-link-text");
    const urlInput = card.querySelector(".ek-link-url");
    const openLinkEl = card.querySelector(".ek-open");
    const removeBtn = card.querySelector(".ek-remove");

    let current = null; // { index, length, text, url, isNew }
    let hideTimer = null;
    let hoverTimer = null;
    let quietUntil = 0;

    const cardActive = () => !card.hidden && (card.contains(document.activeElement) || card.matches(":hover"));

    // The link blot at a position (cursor just after a link counts too).
    function linkAt(index) {
      // getLeaf(i) is the leaf holding character i; check the character
      // before the cursor first, then the one after.
      for (const i of [index - 1, index]) {
        if (i < 0) continue;
        let [blot] = quill.getLeaf(i);
        while (blot && blot.statics.blotName !== "link") blot = blot.parent;
        if (blot && blot.domNode.tagName === "A") return blot;
      }
      return null;
    }

    function showCard(info, focus) {
      clearTimeout(hideTimer);
      current = info;
      textInput.value = info.text;
      urlInput.value = info.url;
      openLinkEl.href = info.url || "#";
      openLinkEl.hidden = !info.url;
      removeBtn.hidden = info.isNew;
      bar.hidden = true;
      placeNear(card, quill, info.index, info.length, "below");
      if (focus === "url") urlInput.focus();
      if (focus === "text") textInput.select();
    }

    function editLink(blot, focus) {
      const index = quill.getIndex(blot);
      const length = blot.length();
      showCard({
        index, length, isNew: false,
        text: quill.getText(index, length),
        url: blot.domNode.getAttribute("href") || "",
      }, focus);
    }

    function hideCard() {
      clearTimeout(hideTimer);
      card.hidden = true;
      current = null;
    }

    function scheduleHide() {
      clearTimeout(hideTimer);
      hideTimer = setTimeout(() => { if (!cardActive()) hideCard(); }, 350);
    }

    function save() {
      if (!current) return;
      const { index, length, isNew } = current;
      const url = normalizeUrl(urlInput.value);
      const text = textInput.value.trim() ? textInput.value : current.text;
      quietUntil = Date.now() + 400;
      if (!url) {
        if (!isNew) quill.formatText(index, length, "link", false, "user");
      } else if (text === current.text) {
        quill.formatText(index, length, "link", url, "user");
      } else {
        const all = quill.getFormat(index, length);
        const keep = {};
        INLINE_FORMATS.forEach((f) => { if (all[f]) keep[f] = all[f]; });
        quill.deleteText(index, length, "user");
        quill.insertText(index, text, { ...keep, link: url }, "user");
      }
      const end = index + (url && text !== current.text ? text.length : length);
      hideCard();
      quill.setSelection(end, 0, "silent");
    }

    function remove() {
      if (!current) return;
      quietUntil = Date.now() + 400;
      quill.formatText(current.index, current.length, "link", false, "user");
      const end = current.index + current.length;
      hideCard();
      quill.setSelection(end, 0, "silent");
    }

    card.querySelector(".ek-save").addEventListener("click", save);
    removeBtn.addEventListener("click", remove);
    card.addEventListener("keydown", (e) => {
      if (e.key === "Enter") { e.preventDefault(); save(); }
      if (e.key === "Escape") {
        e.preventDefault();
        const end = current ? current.index + current.length : null;
        hideCard();
        quietUntil = Date.now() + 400;
        if (end !== null) quill.setSelection(end, 0, "silent");
      }
    });
    urlInput.addEventListener("input", () => {
      const url = normalizeUrl(urlInput.value);
      openLinkEl.href = url || "#";
      openLinkEl.hidden = !url;
    });
    card.addEventListener("mouseenter", () => clearTimeout(hideTimer));
    card.addEventListener("mouseleave", scheduleHide);
    card.addEventListener("focusout", () => setTimeout(() => {
      if (!card.contains(document.activeElement) && !card.matches(":hover")) hideCard();
    }, 0));

    // Hover a link -> show its card.
    quill.root.addEventListener("mouseover", (e) => {
      const a = e.target.closest("a");
      if (!a || !quill.root.contains(a)) return;
      clearTimeout(hideTimer);
      clearTimeout(hoverTimer);
      hoverTimer = setTimeout(() => {
        if (cardActive() && current && !current.isNew) return;
        const blot = Quill.find(a);
        if (blot) editLink(blot);
      }, 200);
    });
    quill.root.addEventListener("mouseout", (e) => {
      const a = e.target.closest("a");
      if (!a || (e.relatedTarget && a.contains(e.relatedTarget))) return;
      clearTimeout(hoverTimer);
      scheduleHide();
    });

    // ---------- Toolbar behaviour ----------
    function openLink() {
      const range = quill.getSelection();
      if (!range) return;
      const existing = linkAt(range.index);
      if (existing && quill.getFormat(range).link) return editLink(existing, "url");
      if (!range.length) return;
      showCard({
        index: range.index, length: range.length, isNew: true,
        text: quill.getText(range.index, range.length), url: "",
      }, "url");
    }

    bar.addEventListener("mousedown", (e) => e.preventDefault()); // keep the selection
    bar.addEventListener("click", (e) => {
      const btn = e.target.closest("button");
      const range = quill.getSelection();
      if (!btn || !range) return;
      const fmt = quill.getFormat(range);
      const f = btn.dataset.format;
      if (f === "link") return openLink();
      if (f === "h3" || f === "h4") {
        const level = f === "h3" ? 3 : 4;
        quill.format("header", fmt.header === level ? false : level, "user");
      } else {
        quill.format(f, !fmt[f], "user");
      }
      update();
    });

    function update() {
      if (Date.now() < quietUntil) return;
      const range = quill.getSelection();
      if (!range) return;

      if (range.length) {
        if (cardActive()) return;
        hideCard();
        const fmt = quill.getFormat(range);
        if (hideIn.some((f) => fmt[f])) { bar.hidden = true; return; }
        bar.querySelectorAll("button").forEach((b) => {
          const f = b.dataset.format;
          b.classList.toggle("on", f === "h3" ? fmt.header === 3 : f === "h4" ? fmt.header === 4 : !!fmt[f]);
        });
        placeNear(bar, quill, range.index, range.length, "above");
        return;
      }

      bar.hidden = true;
      // Cursor inside a link -> show its card (without stealing focus).
      const blot = quill.getFormat(range).link ? linkAt(range.index) : null;
      if (blot) {
        if (!(current && quill.getIndex(blot) === current.index && !card.hidden)) editLink(blot);
      } else if (!cardActive()) {
        hideCard();
      }
    }

    quill.on("editor-change", update);
    quill.on("selection-change", (range) => {
      if (!range) {
        bar.hidden = true;
        setTimeout(() => { if (!cardActive()) hideCard(); }, 0);
      }
    });
    window.addEventListener("resize", () => {
      bar.hidden = true;
      if (current && !card.hidden) placeNear(card, quill, current.index, current.length, "below");
    });

    return { openLink, hide() { bar.hidden = true; hideCard(); } };
  }

  window.EditorKit = { attach };
})();
