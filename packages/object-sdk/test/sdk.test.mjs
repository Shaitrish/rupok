// Tests for sdk.js: runs the real file in a vm context with a fake window,
// parent frame and document. Run: node --test "packages/object-sdk/test/*.test.mjs"
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const SRC = readFileSync(new URL("../sdk.js", import.meta.url), "utf8");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// Copy values out of the vm realm so deepStrictEqual compares plain objects.
const plain = (v) => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));

/**
 * A fake browser frame. `standalone: true` makes window.parent === window.
 */
function makeEnv({ standalone = false } = {}) {
  const winListeners = {};
  const docListeners = {};
  const posted = [];
  const cssVars = {};
  const logs = { info: [], warn: [], error: [], log: [] };
  const parent = {
    postMessage(msg, target) {
      posted.push({ msg: plain(msg), target });
    },
  };
  const on = (map) => (type, fn) => (map[type] ||= []).push(fn);
  const off = (map) => (type, fn) => {
    map[type] = (map[type] || []).filter((f) => f !== fn);
  };
  const ctx = {
    console: Object.fromEntries(
      Object.keys(logs).map((k) => [k, (...a) => logs[k].push(a.map(String).join(" "))]),
    ),
    setTimeout,
    clearTimeout,
    addEventListener: on(winListeners),
    removeEventListener: off(winListeners),
    document: {
      documentElement: {
        style: {
          setProperty: (k, v) => (cssVars[k] = v),
          removeProperty: (k) => delete cssVars[k],
        },
      },
      addEventListener: on(docListeners),
      removeEventListener: off(docListeners),
    },
  };
  ctx.window = ctx;
  ctx.parent = standalone ? ctx : parent;
  vm.createContext(ctx);
  vm.runInContext(SRC, ctx, { filename: "sdk.js" });

  return {
    ctx,
    rupok: ctx.rupok,
    posted,
    cssVars,
    logs,
    parent,
    /** Deliver a message to the object as if posted by `source`. */
    fromHost(data, source = parent) {
      for (const fn of winListeners.message || []) fn({ data, source });
    },
    /** Press a key; returns the event so tests can check preventDefault. */
    key(key, target = { tagName: "BODY" }) {
      const ev = {
        key,
        target,
        defaultPrevented: false,
        preventDefault() {
          this.defaultPrevented = true;
        },
      };
      for (const fn of docListeners.keydown || []) fn(ev);
      return ev;
    },
    types: () => posted.map((p) => p.msg.type),
    last: (type) => [...posted].reverse().find((p) => p.msg.type === type)?.msg,
  };
}

const INIT = {
  rupok: 1,
  type: "init",
  id: "obj-1",
  step: 2,
  steps: 5,
  state: { a: 1 },
  theme: { "color.accent": "#ff0000" },
  mode: "edit",
};

test("hello is sent on load, init resolves ready", async () => {
  const env = makeEnv();
  assert.deepEqual(env.posted[0], { msg: { type: "hello", sdk: "1", rupok: 1 }, target: "*" });
  assert.equal(env.rupok.version, "1");
  assert.equal(env.rupok.mode, "present", "default mode before init");

  // Messages from other windows or without rupok:1 are ignored.
  env.fromHost(INIT, { postMessage() {} });
  env.fromHost({ ...INIT, rupok: 2 });
  env.fromHost({ type: "init", id: "x" });
  assert.equal(env.rupok.steps.count, 0);

  env.fromHost(INIT);
  const info = plain(await env.rupok.ready);
  assert.deepEqual(info, {
    id: "obj-1",
    step: 2,
    steps: 5,
    state: { a: 1 },
    theme: plain(env.rupok.theme.all()),
    mode: "edit",
    standalone: false,
  });
  assert.equal(env.rupok.mode, "edit");
  assert.equal(env.rupok.steps.current, 2);
  assert.equal(env.rupok.steps.count, 5);
  assert.deepEqual(plain(env.rupok.state.load()), { a: 1 });

  // load() returns a copy.
  env.rupok.state.load().a = 99;
  assert.deepEqual(plain(env.rupok.state.load()), { a: 1 });

  env.fromHost({ rupok: 1, type: "mode", mode: "present" });
  assert.equal(env.rupok.mode, "present");
  env.fromHost({ rupok: 1, type: "mode", mode: "bogus" });
  assert.equal(env.rupok.mode, "present");
});

test("steps: initial call, host step messages, goto/next requests, setCount", async () => {
  const env = makeEnv();
  const seen = [];
  const unsub = env.rupok.steps.onChange((cur, change) => seen.push([cur, plain(change)]));
  env.fromHost(INIT);
  await env.rupok.ready;
  assert.deepEqual(seen, [[2, { previous: 2, count: 5, source: "init" }]]);

  env.fromHost({ rupok: 1, type: "step", step: 3 });
  assert.deepEqual(seen.at(-1), [3, { previous: 2, count: 5, source: "host" }]);
  assert.equal(env.rupok.steps.current, 3);

  // next() only asks; current changes when the host replies.
  env.rupok.steps.next();
  assert.deepEqual(env.last("goto"), { type: "goto", step: 4, rupok: 1 });
  assert.equal(env.rupok.steps.current, 3);
  // A second quick next() counts from the pending request.
  env.rupok.steps.next();
  assert.deepEqual(env.last("goto"), { type: "goto", step: 5, rupok: 1 });
  env.fromHost({ rupok: 1, type: "step", step: 5 });
  assert.equal(env.rupok.steps.current, 5);
  const gotos = env.types().filter((t) => t === "goto").length;
  env.rupok.steps.next(); // at count: no-op
  assert.equal(env.types().filter((t) => t === "goto").length, gotos);

  // goto clamps; host steps beyond count are clamped.
  env.rupok.steps.goto(99);
  assert.equal(env.types().filter((t) => t === "goto").length, gotos, "goto(current) sends nothing");
  env.rupok.steps.goto(-4);
  assert.deepEqual(env.last("goto"), { type: "goto", step: 0, rupok: 1 });
  env.fromHost({ rupok: 1, type: "step", step: 0 });
  env.fromHost({ rupok: 1, type: "step", step: 42 });
  assert.equal(env.rupok.steps.current, 5);

  // setCount declares at runtime and clamps current.
  env.rupok.steps.setCount(3);
  assert.deepEqual(env.last("steps"), { type: "steps", count: 3, rupok: 1 });
  assert.equal(env.rupok.steps.count, 3);
  assert.equal(env.rupok.steps.current, 3);
  assert.deepEqual(seen.at(-1), [3, { previous: 5, count: 3, source: "local" }]);
  env.rupok.steps.setCount(9999);
  assert.equal(env.last("steps").count, 500);

  // Subscribing after ready gets an initial call (microtask).
  const late = [];
  env.rupok.steps.onChange((cur, change) => late.push([cur, change.source]));
  assert.equal(late.length, 0);
  await Promise.resolve();
  assert.deepEqual(late, [[3, "init"]]);

  // Unsubscribe stops delivery; a throwing listener does not break others.
  unsub();
  env.rupok.steps.onChange(() => {
    throw new Error("boom");
  });
  const n = seen.length;
  env.fromHost({ rupok: 1, type: "step", step: 1 });
  assert.equal(seen.length, n);
  assert.deepEqual(late.at(-1), [1, "host"]);
  assert.ok(env.logs.error.some((l) => l.includes("listener failed")));
});

test("state.save is debounced by 200 ms and drops oversize payloads", async () => {
  const env = makeEnv();
  env.rupok.state.save({ early: true }); // before ready: ignored
  assert.ok(env.logs.warn.some((l) => l.includes("before rupok.ready")));
  env.fromHost({ ...INIT, state: null });
  await env.rupok.ready;
  assert.equal(env.rupok.state.load(), undefined);

  const value = { n: 1 };
  env.rupok.state.save(value);
  value.n = 2; // snapshot at call time
  env.rupok.state.save({ n: 3 });
  env.rupok.state.save(value);
  assert.equal(env.types().includes("state"), false, "nothing sent before the debounce");
  await sleep(120);
  assert.equal(env.types().includes("state"), false);
  await sleep(150);
  const states = env.posted.filter((p) => p.msg.type === "state");
  assert.equal(states.length, 1);
  assert.deepEqual(states[0].msg, { type: "state", state: { n: 2 }, rupok: 1 });
  assert.deepEqual(plain(env.rupok.state.load()), { n: 2 });

  // Oversize (> 256 KiB of UTF-8 JSON) is dropped with a warning.
  env.rupok.state.save({ big: "é".repeat(140 * 1024) }); // 280 KiB in UTF-8
  assert.ok(env.logs.warn.some((l) => l.includes("byte limit")));
  // Unserialisable values too.
  const cyclic = {};
  cyclic.self = cyclic;
  env.rupok.state.save(cyclic);
  assert.ok(env.logs.warn.some((l) => l.includes("not JSON-serialisable")));
  await sleep(250);
  assert.equal(env.posted.filter((p) => p.msg.type === "state").length, 1);
  assert.deepEqual(plain(env.rupok.state.load()), { n: 2 });

  // Exactly at the limit is accepted.
  const fits = "x".repeat(256 * 1024 - '{"s":""}'.length);
  env.rupok.state.save({ s: fits });
  await sleep(250);
  assert.equal(env.posted.filter((p) => p.msg.type === "state").length, 2);
});

test("theme tokens are mirrored to CSS variables, with defaults before init", async () => {
  const env = makeEnv();
  assert.equal(env.cssVars["--rupok-color-accent"], "#2563eb");
  assert.equal(env.cssVars["--rupok-color-bg"], "#ffffff");
  assert.equal(env.cssVars["--rupok-radius"], "12px");
  assert.ok(env.cssVars["--rupok-font-body"].includes("system-ui"));
  assert.equal(env.rupok.theme.get("color.accent"), "#2563eb");

  const themes = [];
  env.rupok.theme.onChange((t) => themes.push(plain(t)));
  env.fromHost(INIT);
  await env.rupok.ready;
  assert.equal(env.cssVars["--rupok-color-accent"], "#ff0000");
  assert.equal(env.cssVars["--rupok-color-fg"], "#1d2433", "omitted tokens keep defaults");
  assert.equal(env.rupok.theme.get("color.accent", "#000"), "#ff0000");
  assert.equal(env.rupok.theme.get("nope", "fallback"), "fallback");
  assert.equal(themes.length, 1);
  assert.equal(themes[0]["color.accent"], "#ff0000");

  env.fromHost({
    rupok: 1,
    type: "theme",
    theme: { "color.bg": "#0b1020", "color.warning": "#f59e0b", "bad token!": "x", "color.fg": 42 },
  });
  assert.equal(env.cssVars["--rupok-color-bg"], "#0b1020");
  assert.equal(env.cssVars["--rupok-color-warning"], "#f59e0b", "extra tokens are mirrored");
  assert.equal(env.cssVars["--rupok-color-accent"], "#2563eb", "full theme: accent back to default");
  assert.equal(env.cssVars["--rupok-color-fg"], "#1d2433", "non-string values ignored");
  assert.equal(Object.keys(env.cssVars).some((k) => k.includes("bad")), false);
  assert.equal(themes.length, 2);

  env.fromHost({ rupok: 1, type: "theme", theme: {} });
  assert.equal(env.cssVars["--rupok-color-warning"], undefined, "dropped tokens are removed");
});

test("calls before ready never throw and are flushed after init", async () => {
  const env = makeEnv();
  assert.doesNotThrow(() => {
    env.rupok.steps.next();
    env.rupok.steps.prev();
    env.rupok.steps.goto(1);
    env.rupok.steps.setCount(4);
    env.rupok.steps.goto("nope");
    env.rupok.quiz.submit({ questionId: "q1", answer: "B", correct: true, score: 1, extra: "dropped" });
    env.rupok.quiz.submit({ answer: "no id" });
    env.rupok.quiz.submit(null);
    env.rupok.resize(800, 450);
    env.rupok.resize(-1, NaN);
    env.rupok.log("warn", "careful", { x: 1 });
    env.rupok.log("plain message");
    env.rupok.state.load();
    env.rupok.theme.all();
  });
  assert.deepEqual(env.types(), ["hello"], "nothing but hello before init");

  env.fromHost({ ...INIT, step: 0 });
  await env.rupok.ready;
  const msgs = env.posted.slice(1).map((p) => p.msg);
  assert.deepEqual(msgs, [
    { type: "goto", step: 1, rupok: 1 }, // next()
    { type: "goto", step: 0, rupok: 1 }, // prev(), counted from the pending request
    { type: "goto", step: 1, rupok: 1 }, // goto(1)
    { type: "steps", count: 4, rupok: 1 },
    { type: "quiz", payload: { questionId: "q1", answer: "B", correct: true, score: 1 }, rupok: 1 },
    { type: "resize", w: 800, h: 450, rupok: 1 },
    { type: "log", level: "warn", message: 'careful {"x":1}', rupok: 1 },
    { type: "log", level: "info", message: "plain message", rupok: 1 },
  ]);
  assert.equal(env.rupok.steps.count, 4);
  assert.ok(env.logs.warn.some((l) => l.includes("questionId")));
});

test("standalone: no parent resolves ready at once and keys drive steps", async () => {
  const env = makeEnv({ standalone: true });
  assert.equal(env.posted.length, 0, "never posts without a parent");
  const info = plain(await env.rupok.ready);
  assert.equal(info.standalone, true);
  assert.equal(info.id, "standalone");
  assert.equal(info.mode, "present");
  assert.equal(info.step, 0);

  const seen = [];
  env.rupok.steps.setCount(3);
  env.rupok.steps.onChange((cur, change) => seen.push([cur, change.source]));
  await Promise.resolve();
  assert.deepEqual(seen, [[0, "init"]]);

  assert.ok(env.key("ArrowRight").defaultPrevented);
  env.key(" ");
  env.key("PageDown");
  env.key("ArrowRight"); // already at count
  assert.equal(env.rupok.steps.current, 3);
  env.key("ArrowLeft");
  env.key("PageUp");
  assert.equal(env.rupok.steps.current, 1);
  assert.deepEqual(seen.slice(1), [
    [1, "local"],
    [2, "local"],
    [3, "local"],
    [2, "local"],
    [1, "local"],
  ]);

  // Typing in inputs and Space on buttons do not step.
  env.key("ArrowRight", { tagName: "INPUT" });
  env.key(" ", { tagName: "BUTTON" });
  assert.equal(env.rupok.steps.current, 1);

  // goto works locally; state lives in memory; quiz does not post.
  env.rupok.steps.goto(3);
  assert.equal(env.rupok.steps.current, 3);
  env.rupok.state.save({ local: true });
  assert.deepEqual(plain(env.rupok.state.load()), { local: true });
  env.rupok.quiz.submit({ questionId: "q", answer: 1 });
  await sleep(250);
  assert.equal(env.posted.length, 0);
});

test("standalone fallback after 600 ms without init, then a late init is adopted", async () => {
  const env = makeEnv();
  let resolved = false;
  env.rupok.ready.then(() => (resolved = true));
  env.rupok.steps.setCount(2);
  await sleep(400);
  assert.equal(resolved, false, "still waiting for init at 400 ms");
  await sleep(350);
  assert.equal(resolved, true, "fell back by ~600 ms");
  const info = plain(await env.rupok.ready);
  assert.equal(info.standalone, true);
  assert.equal(env.rupok.steps.count, 2);
  assert.deepEqual(env.types(), ["hello"], "no messages to a silent parent");

  env.key("ArrowRight");
  assert.equal(env.rupok.steps.current, 1);

  // A late host init takes over: values applied, keys stop, count re-declared.
  const seen = [];
  env.rupok.steps.onChange((cur, change) => seen.push([cur, change.source]));
  await Promise.resolve();
  env.fromHost({ ...INIT, step: 0, steps: 6, mode: "present" });
  assert.equal(env.rupok.steps.count, 2, "declared count wins over the host default");
  assert.deepEqual(env.last("steps"), { type: "steps", count: 2, rupok: 1 });
  assert.deepEqual(seen.at(-1), [0, "init"]);
  env.key("ArrowRight");
  assert.equal(env.rupok.steps.current, 0, "keys no longer step once a host is present");
  env.rupok.steps.next();
  assert.deepEqual(env.last("goto"), { type: "goto", step: 1, rupok: 1 });
});

test("loading the SDK twice keeps the first instance; Node require is a no-op", () => {
  const env = makeEnv();
  const first = env.ctx.rupok;
  vm.runInContext(SRC, env.ctx, { filename: "sdk-again.js" });
  assert.equal(env.ctx.rupok, first);
  assert.equal(env.types().filter((t) => t === "hello").length, 1);

  const bare = vm.createContext({});
  assert.doesNotThrow(() => vm.runInContext(SRC, bare));
  assert.equal(bare.rupok, undefined);
});

test("embedded: presenter keys the object ignores are forwarded to the host", async () => {
  const env = makeEnv();
  env.fromHost(INIT);
  await env.rupok.ready;
  env.key("PageDown");
  env.key("ArrowLeft");
  const used = env.key("ArrowRight");
  used.preventDefault(); // the object handled this one itself
  env.key("a"); // not a presenter key
  env.key("PageDown", { tagName: "INPUT" }); // typing in a field
  await sleep(5);
  const keys = env.posted.filter((p) => p.msg.type === "key").map((p) => p.msg.key);
  assert.deepEqual(keys, ["PageDown", "ArrowLeft"]);
});
