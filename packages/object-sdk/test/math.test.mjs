// Tests for the SDK's built-in LaTeX (rupok.tex, rupok.math, auto-render).
// Runs the real sdk.js in a vm context with a small fake DOM.
// Run: node --test "packages/object-sdk/test/*.test.mjs"
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import vm from "node:vm";

const SRC = readFileSync(new URL("../sdk.js", import.meta.url), "utf8");
const KATEX = new URL("../../../apps/board/node_modules/katex/dist/katex.min.js", import.meta.url);
const tick = () => new Promise((r) => setTimeout(r, 0));

/* ----------------------------------------------------------- fake DOM */

class FakeNode {
  constructor(nodeType) {
    this.nodeType = nodeType;
    this.parentNode = null;
    this.childNodes = [];
  }
  get firstChild() {
    return this.childNodes[0] || null;
  }
  appendChild(n) {
    return this.insertBefore(n, null);
  }
  insertBefore(n, ref) {
    if (n.parentNode) n.parentNode.removeChild(n);
    const i = ref ? this.childNodes.indexOf(ref) : -1;
    if (i < 0) this.childNodes.push(n);
    else this.childNodes.splice(i, 0, n);
    n.parentNode = this;
    return n;
  }
  removeChild(n) {
    this.childNodes = this.childNodes.filter((c) => c !== n);
    n.parentNode = null;
    return n;
  }
  get textContent() {
    return this.nodeType === 3 ? this.nodeValue : this.childNodes.map((c) => c.textContent).join("");
  }
  set textContent(v) {
    if (this.nodeType === 3) this.nodeValue = v;
    else {
      this.childNodes = [];
      if (v) this.appendChild(text(String(v)));
    }
  }
}
class FakeElement extends FakeNode {
  constructor(tag, attrs = {}) {
    super(1);
    this.tagName = tag.toUpperCase();
    this.attrs = { ...attrs };
  }
  getAttribute(k) {
    return Object.prototype.hasOwnProperty.call(this.attrs, k) ? this.attrs[k] : null;
  }
  setAttribute(k, v) {
    this.attrs[k] = String(v);
  }
}
function text(v) {
  const n = new FakeNode(3);
  n.nodeValue = v;
  return n;
}
function el(tag, attrs, ...kids) {
  const e = new FakeElement(tag, attrs);
  for (const k of kids) e.appendChild(typeof k === "string" ? text(k) : k);
  return e;
}
/** Serialise a subtree: rendered formulas appear as ⟦tex⟧ (or ⟦⟦tex⟧⟧ for display). */
function show(n) {
  if (n.nodeType === 3) return n.nodeValue;
  if (n.rendered) return n.rendered.opts.displayMode ? `⟦⟦${n.rendered.tex}⟧⟧` : `⟦${n.rendered.tex}⟧`;
  return n.childNodes.map(show).join("");
}

/**
 * A fake object frame. `lib` writes <meta name="rupok-lib">, `mathOff` the
 * opt-out meta, `katex: true` provides window.katex up front.
 */
function makeEnv({ body = [], lib, mathOff = false, katex = false, standalone = true } = {}) {
  const head = el("head");
  if (lib) head.appendChild(el("meta", { name: "rupok-lib", content: lib }));
  if (mathOff) head.appendChild(el("meta", { name: "rupok-math", content: "off" }));
  const bodyEl = el("body", {}, ...body);
  const html = el("html", {}, head, bodyEl);
  const renders = [];
  const warns = [];
  const doc = new FakeNode(9);
  Object.assign(doc, {
    readyState: "complete",
    head,
    body: bodyEl,
    documentElement: Object.assign(html, { style: { setProperty() {}, removeProperty() {} } }),
    createElement: (t) => el(t),
    createTextNode: (v) => text(v),
    querySelector(sel) {
      const m = sel.match(/^meta\[name="([^"]+)"\]$/);
      return m ? head.childNodes.find((n) => n.tagName === "META" && n.getAttribute("name") === m[1]) || null : null;
    },
    addEventListener() {},
    removeEventListener() {},
  });
  const fakeKatex = {
    render(tex, target, opts) {
      renders.push({ tex, opts });
      target.childNodes = [];
      target.rendered = { tex, opts };
    },
  };
  const ctx = {
    console: { info() {}, log() {}, error() {}, warn: (...a) => warns.push(a.join(" ")) },
    setTimeout,
    clearTimeout,
    addEventListener() {},
    removeEventListener() {},
    document: doc,
    katex: katex ? fakeKatex : undefined,
  };
  ctx.window = ctx;
  ctx.parent = standalone ? ctx : { postMessage() {} };
  vm.createContext(ctx);
  vm.runInContext(SRC, ctx, { filename: "sdk.js" });
  return { ctx, rupok: ctx.rupok, doc, head, body: bodyEl, renders, warns, fakeKatex };
}

/* ----------------------------------------------------------- tests */

test("auto-render finds $…$, $$…$$, \\(…\\), \\[…\\] and [data-tex]", async () => {
  const env = makeEnv({
    katex: true,
    body: [
      el("p", {}, "Energy $E = mc^2$ and $$F = ma$$."),
      el("p", {}, "Inline \\(a^2\\) and display \\[b^2\\]"),
      el("span", { "data-tex": "\\frac{1}{2}", "data-display": "" }),
      el("code", {}, "$not maths$"),
      el("p", { "data-no-math": "" }, "$skip$"),
    ],
  });
  await tick();
  const [p1, p2, span, code, skip] = env.body.childNodes;
  assert.equal(show(p1), "Energy ⟦E = mc^2⟧ and ⟦⟦F = ma⟧⟧.");
  assert.equal(show(p2), "Inline ⟦a^2⟧ and display ⟦⟦b^2⟧⟧");
  assert.equal(show(span), "⟦⟦\\frac{1}{2}⟧⟧");
  assert.equal(show(code), "$not maths$", "code is left alone");
  assert.equal(show(skip), "$skip$", "data-no-math is left alone");
  const wrappers = p1.childNodes.filter((n) => n.nodeType === 1);
  assert.deepEqual(wrappers.map((w) => w.getAttribute("class")), ["rupok-math", "rupok-math rupok-math-display"]);
});

test("money and escaped dollars stay text", async () => {
  const env = makeEnv({ katex: true, body: [el("p", {}, "It costs $5 and $10, or \\$3."), el("p", {}, "Use $ x $ loosely")] });
  await tick();
  assert.equal(show(env.body.childNodes[0]), "It costs $5 and $10, or $3.");
  assert.equal(show(env.body.childNodes[1]), "Use $ x $ loosely");
  assert.equal(env.renders.length, 0);
});

test('<meta name="rupok-math" content="off"> disables auto-render, not the API', async () => {
  const p = el("p", {}, "$x^2$");
  const env = makeEnv({ katex: true, mathOff: true, body: [p] });
  await tick();
  assert.equal(show(p), "$x^2$");
  assert.equal(await env.rupok.math.render(p), 1, "explicit render still works");
  assert.equal(show(p), "⟦x^2⟧");
});

test("tex() renders, skips identical re-renders and handles bad input", async () => {
  const env = makeEnv({ katex: true });
  const target = el("div");
  assert.equal(await env.rupok.tex(target, "v = \\hl{9.8}"), true);
  assert.equal(target.rendered.tex, "v = \\hl{9.8}");
  assert.equal(target.rendered.opts.displayMode, false);
  assert.equal(await env.rupok.tex(target, "v = \\hl{9.8}"), true);
  assert.equal(env.renders.length, 1, "same formula is not re-rendered");
  target.childNodes = [text("replaced by the page")];
  await env.rupok.tex(target, "v = \\hl{9.8}");
  assert.equal(env.renders.length, 2, "re-rendered after the page replaced the content");
  await env.rupok.tex(target, "x", { display: true });
  assert.equal(target.rendered.opts.displayMode, true);
  assert.equal(await env.rupok.tex(null, "x"), false);
  assert.equal(await env.rupok.tex(target, 42), false);
});

test("\\hl is a macro for a narrowly trusted \\htmlClass", async () => {
  const env = makeEnv({ katex: true });
  await env.rupok.tex(el("div"), "\\hl{a}");
  const { opts } = env.renders[0];
  assert.equal(opts.macros["\\hl"], "\\htmlClass{rupok-hl}{#1}");
  assert.equal(opts.throwOnError, false);
  assert.equal(opts.trust({ command: "\\htmlClass", class: "rupok-hl" }), true);
  assert.equal(opts.trust({ command: "\\htmlClass", class: "evil" }), false);
  assert.equal(opts.trust({ command: "\\href", url: "https://example.com" }), false);
  assert.equal(opts.trust({ command: "\\htmlStyle", style: "color:red" }), false);
  const style = env.head.childNodes.find((n) => n.tagName === "STYLE");
  assert.match(style.textContent, /\.rupok-hl\{color:var\(--rupok-color-accent-text/, "highlight text uses accent-text");
  assert.match(style.textContent, /background:var\(--rupok-color-accent-soft/, "on accent-soft");
});

test("KaTeX loads lazily from the host's <meta name=\"rupok-lib\">", async () => {
  const env = makeEnv({ lib: "https://board.example/rupoklib/", body: [el("p", {}, "no maths here")] });
  await tick();
  assert.equal(env.head.childNodes.filter((n) => n.tagName === "SCRIPT").length, 0, "nothing loads without maths");

  const target = el("div", {}, "fallback");
  const done = env.rupok.tex(target, "a+b");
  const script = env.head.childNodes.find((n) => n.tagName === "SCRIPT");
  const link = env.head.childNodes.find((n) => n.tagName === "LINK");
  assert.equal(script.src, "https://board.example/rupoklib/katex/katex.min.js");
  assert.equal(link.href, "https://board.example/rupoklib/katex/katex.min.css");
  assert.equal(show(target), "fallback", "content stays until KaTeX arrives");
  env.ctx.katex = env.fakeKatex;
  link.onload();
  script.onload();
  assert.equal(await done, true);
  assert.equal(show(target), "⟦a+b⟧");
});

test("without KaTeX or a lib URL, maths stays as source and tex() resolves false", async () => {
  const p = el("p", {}, "$x$");
  const env = makeEnv({ body: [p] });
  await tick();
  assert.equal(show(p), "$x$");
  assert.equal(await env.rupok.tex(el("div"), "y"), false);
  assert.ok(env.warns.some((w) => w.includes("rupok-lib")));
});

test("real KaTeX renders \\hl with the rupok-hl class", { skip: !existsSync(KATEX) && "KaTeX not installed (bun install in apps/board)" }, async () => {
  const env = makeEnv({});
  const sandbox = { self: {} };
  vm.runInNewContext(readFileSync(KATEX, "utf8"), sandbox);
  const real = sandbox.self.katex || sandbox.katex;
  env.ctx.katex = { render: (tex, target, opts) => (target.html = real.renderToString(tex, opts)) };
  const a = el("div");
  await env.rupok.tex(a, "F = \\hl{m}a");
  assert.match(a.html, /class="enclosing rupok-hl"|class="rupok-hl"|rupok-hl/);
  assert.doesNotMatch(a.html, /katex-error/);
  const b = el("div");
  await env.rupok.tex(b, "\\htmlClass{evil}{x}");
  assert.doesNotMatch(b.html, /class="[^"]*evil/, "other classes are not trusted");
});
