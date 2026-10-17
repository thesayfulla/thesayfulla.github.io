// ---- Theme toggle ----
(function () {
  const root = document.documentElement;
  const isDark = () =>
    root.dataset.theme
      ? root.dataset.theme === "dark"
      : matchMedia("(prefers-color-scheme: dark)").matches;
  const btn = document.getElementById("theme-toggle");
  if (!btn) return;
  btn.addEventListener("click", () => {
    const next = isDark() ? "light" : "dark";
    root.dataset.theme = next;
    try { localStorage.setItem("theme", next); } catch (e) {}
  });
})();

// ---- Halftone dot hero (home page only) ----
(function () {
  const canvas = document.getElementById("hero-canvas");
  if (!canvas) return;
  const root = document.documentElement;
  const ctx = canvas.getContext("2d");
  const video = document.getElementById("hero-video");
  const sample = document.createElement("canvas");
  const sctx = sample.getContext("2d", { willReadFrequently: true });
  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const CELL = 7; // device pixels per dot cell
  let cols = 0, rows = 0;
  let useVideo = false;

  function resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const r = canvas.getBoundingClientRect();
    canvas.width = Math.round(r.width * dpr);
    canvas.height = Math.round(r.height * dpr);
    cols = Math.ceil(canvas.width / ((CELL * dpr) / 1.4));
    rows = Math.ceil(canvas.height / ((CELL * dpr) / 1.4));
    sample.width = cols;
    sample.height = rows;
  }

  // Generated fallback: drifting, pulsing jellyfish-like blobs.
  const blobs = Array.from({ length: 5 }, (_, i) => ({
    x: 0.12 + i * 0.19 + 0.05 * Math.random(),
    y: i % 2 ? 0.3 + 0.15 * Math.random() : 0.55 + 0.15 * Math.random(),
    r: 0.12 + 0.1 * Math.random(),
    sx: 0.02 + 0.03 * Math.random(),
    sy: 0.015 + 0.02 * Math.random(),
    p: Math.random() * Math.PI * 2,
  }));

  function fieldAt(u, v, t) {
    let s = 0;
    for (const b of blobs) {
      const bx = b.x + Math.sin(t * b.sx * 6 + b.p) * 0.12;
      const by = b.y + Math.cos(t * b.sy * 6 + b.p) * 0.1;
      const pulse = 1 + 0.18 * Math.sin(t * 1.6 + b.p);
      const dx = (u - bx) * 1.78, dy = (v - by) * 1.15;
      const d2 = (dx * dx + dy * dy) / (b.r * b.r * pulse * pulse);
      // Bell with a brighter rim, like a jellyfish bell.
      s += Math.exp(-d2 * 1.4) * 0.55 + Math.exp(-Math.pow(Math.sqrt(d2) - 0.8, 2) * 18) * 0.5;
      // Trailing tentacles below each bell.
      if (v > by && Math.abs(dx) < b.r * 1.6) {
        const tent = Math.pow(Math.max(0, Math.sin((dx / b.r) * 9 + Math.sin(v * 22 - t * 2) * 1.2)), 12);
        s += tent * Math.exp(-(v - by) * 5) * 0.35;
      }
    }
    // Faint drifting particles / water.
    const n = Math.sin(u * 13 + v * 7 + t * 0.4) + Math.sin(u * 5 - v * 11 - t * 0.3);
    s += 0.08 * Math.pow(Math.max(0, n / 2), 6);
    return Math.min(1, s);
  }

  function draw(t) {
    const w = canvas.width, h = canvas.height;
    ctx.clearRect(0, 0, w, h);
    const dot = getComputedStyle(root).getPropertyValue("--dot").trim();
    const cw = w / cols, ch = h / rows;
    const maxR = Math.min(cw, ch) * 0.62;

    let data = null;
    if (useVideo) {
      sctx.drawImage(video, 0, 0, cols, rows);
      data = sctx.getImageData(0, 0, cols, rows).data;
    }

    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < cols; x++) {
        let b;
        if (data) {
          const i = (y * cols + x) * 4;
          b = (0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2]) / 255;
        } else {
          b = fieldAt(x / cols, y / rows, t);
        }
        if (b < 0.06) continue;
        ctx.fillStyle = `rgba(${dot},${0.25 + 0.75 * b})`;
        ctx.beginPath();
        ctx.arc(x * cw + cw / 2, y * ch + ch / 2, maxR * Math.sqrt(b), 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  const start = performance.now();
  function loop(now) {
    draw((now - start) / 1000);
    if (!reduceMotion) requestAnimationFrame(loop);
  }

  if (video) {
    video.addEventListener("loadeddata", () => { useVideo = true; });
    if (video.dataset.src) video.src = video.dataset.src;
  }
  window.addEventListener("resize", resize);
  resize();
  canvas.classList.add("ready");
  requestAnimationFrame(loop);
})();
