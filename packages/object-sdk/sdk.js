/*!
 * @rupok/object-sdk 0.1.0 | Apache-2.0 | (c) Shaitrish and rupok contributors
 *
 * The in-object SDK for rupok Smart Objects. Defines `window.rupok` and
 * implements bridge protocol v1 (spec/smart-object.md). Dependency-free,
 * ES2019, safe to load twice (the first copy wins), and no method throws.
 */
(function (root) {
  "use strict";

  /* Not a browser window (for example `require()` in Node): do nothing. */
  if (!root || typeof root !== "object" || !root.document) return;
  /* Already installed (host-injected copy plus an author copy). */
  if (root.rupok && root.rupok.__rupoksdk) return;

  var PROTOCOL = 1;
  var SDK_VERSION = "1";
  var INIT_TIMEOUT_MS = 600;
  var SAVE_DEBOUNCE_MS = 200;
  var MAX_STATE_BYTES = 256 * 1024;
  var MAX_STEPS = 500;
  var MAX_LOG_CHARS = 4000;
  var MAX_FORWARDED_ERRORS = 20;
  var GOTO_PENDING_MS = 1000;
  var MODES = ["edit", "present", "replay", "student"];
  var TOKEN_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
  var SANS = 'system-ui, -apple-system, "Segoe UI", Roboto, "Noto Sans", Arial, sans-serif';
  var DEFAULT_THEME = {
    "color.bg": "#ffffff",
    "color.fg": "#1d2433",
    "color.accent": "#2563eb",
    "color.accent2": "#7c3aed",
    "color.muted": "#64748b",
    "color.success": "#15803d",
    "color.danger": "#dc2626",
    "font.body": SANS,
    "font.heading": SANS,
    "font.mono": 'ui-monospace, "Cascadia Mono", "SF Mono", Menlo, Consolas, "Liberation Mono", monospace',
    "radius": "12px"
  };

  var doc = root.document;
  var con = root.console || {};
  var hasParent = false;
  try {
    hasParent = !!root.parent && root.parent !== root;
  } catch (e) {
    hasParent = false;
  }

  /* ------------------------------------------------------------- state */

  var ready = false;
  var standalone = false;
  var silent = false;
  var objectId = "";
  var mode = "present";
  var current = 0;
  var count = 0;
  var declaredCount = null;
  var savedJson; /* JSON text of the latest known state, or undefined */
  var theme = assign({}, DEFAULT_THEME);
  var appliedVars = {};
  var queue = [];
  var stepSubs = [];
  var themeSubs = [];
  var modeSubs = [];
  var pendingGoto = null;
  var pendingTimer = null;
  var pendingSave; /* JSON text waiting for the debounce, or undefined */
  var saveTimer = null;
  var initTimer = null;
  var keysOn = false;
  var forwardedErrors = 0;
  var resolveReady;
  var readyPromise = new Promise(function (resolve) {
    resolveReady = resolve;
  });

  /* ----------------------------------------------------------- helpers */

  function assign(target, src) {
    for (var k in src) if (Object.prototype.hasOwnProperty.call(src, k)) target[k] = src[k];
    return target;
  }

  function own(obj, key) {
    return Object.prototype.hasOwnProperty.call(obj, key);
  }

  function noop() {}

  function consoleCall(level, args) {
    try {
      var fn = con[level] || con.log;
      if (typeof fn === "function") fn.apply(con, args);
    } catch (e) {
      /* console unavailable */
    }
  }

  function warn(message) {
    consoleCall("warn", ["[rupok] " + message]);
  }

  function safeCall(fn, a, b) {
    try {
      fn(a, b);
    } catch (e) {
      consoleCall("error", ["[rupok] listener failed:", e]);
    }
  }

  function later(fn) {
    Promise.resolve().then(fn);
  }

  function toInt(n) {
    var v = Number(n);
    if (typeof n === "boolean" || n === null || n === "" || !isFinite(v)) return null;
    return Math.floor(v);
  }

  function clamp(v, lo, hi) {
    return v < lo ? lo : v > hi ? hi : v;
  }

  function isMode(m) {
    return MODES.indexOf(m) >= 0;
  }

  function stringify(v) {
    try {
      var s = JSON.stringify(v);
      return typeof s === "string" ? s : undefined;
    } catch (e) {
      return undefined;
    }
  }

  function utf8Length(s) {
    var n = 0;
    for (var i = 0; i < s.length; i++) {
      var c = s.charCodeAt(i);
      if (c < 0x80) n += 1;
      else if (c < 0x800) n += 2;
      else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length) {
        var d = s.charCodeAt(i + 1);
        if (d >= 0xdc00 && d <= 0xdfff) {
          n += 4;
          i++;
        } else n += 3;
      } else n += 3;
    }
    return n;
  }

  function format(args) {
    var parts = [];
    for (var i = 0; i < args.length; i++) {
      var a = args[i];
      if (typeof a === "string") parts.push(a);
      else if (a && typeof a === "object" && typeof a.message === "string" && typeof a.name === "string") {
        parts.push(a.name + ": " + a.message);
      } else {
        var s = stringify(a);
        parts.push(s === undefined ? String(a) : s);
      }
    }
    var msg = parts.join(" ");
    return msg.length > MAX_LOG_CHARS ? msg.slice(0, MAX_LOG_CHARS - 1) + "…" : msg;
  }

  function post(msg) {
    if (!hasParent || standalone) return;
    msg.rupok = PROTOCOL;
    try {
      root.parent.postMessage(msg, "*");
    } catch (e) {
      /* parent gone */
    }
  }

  function whenReady(fn) {
    if (ready) fn();
    else queue.push(fn);
  }

  function subscribe(list, cb, initial) {
    if (typeof cb !== "function") return noop;
    var entry = { cb: cb };
    list.push(entry);
    if (ready && initial) {
      later(function () {
        if (list.indexOf(entry) >= 0) initial(cb);
      });
    }
    return function unsubscribe() {
      var i = list.indexOf(entry);
      if (i >= 0) list.splice(i, 1);
    };
  }

  function emit(list, a, b) {
    var copy = list.slice();
    for (var i = 0; i < copy.length; i++) safeCall(copy[i].cb, a, b);
  }

  /* ------------------------------------------------------------- steps */

  function stepDetail(previous, source) {
    return { previous: previous, count: count, source: source };
  }

  function setSteps(nextCurrent, nextCount, source) {
    var prev = current;
    var prevCount = count;
    count = clamp(nextCount, 0, MAX_STEPS);
    current = clamp(nextCurrent, 0, count);
    if (ready && !silent && (current !== prev || count !== prevCount)) {
      emit(stepSubs, current, stepDetail(prev, source));
    }
  }

  function clearPending() {
    pendingGoto = null;
    if (pendingTimer) clearTimeout(pendingTimer);
    pendingTimer = null;
  }

  function requestStep(n) {
    var target = clamp(n, 0, count);
    if (standalone) {
      setSteps(target, count, "local");
      return;
    }
    if (target === current && pendingGoto === null) return;
    pendingGoto = target;
    if (pendingTimer) clearTimeout(pendingTimer);
    pendingTimer = setTimeout(clearPending, GOTO_PENDING_MS);
    post({ type: "goto", step: target });
  }

  function stepBase() {
    return pendingGoto !== null ? pendingGoto : current;
  }

  /* ------------------------------------------------------------- theme */

  function cssName(token) {
    return "--rupok-" + token.replace(/\./g, "-");
  }

  function applyThemeVars() {
    var el = doc.documentElement;
    var style = el && el.style;
    if (!style || typeof style.setProperty !== "function") return;
    var names = {};
    for (var k in theme) {
      if (!own(theme, k)) continue;
      var name = cssName(k);
      names[name] = true;
      try {
        style.setProperty(name, theme[k]);
      } catch (e) {
        /* invalid value */
      }
    }
    for (var old in appliedVars) {
      if (own(appliedVars, old) && !names[old] && typeof style.removeProperty === "function") {
        try {
          style.removeProperty(old);
        } catch (e) {
          /* ignore */
        }
      }
    }
    appliedVars = names;
  }

  function setTheme(t, notify) {
    var next = assign({}, DEFAULT_THEME);
    if (t && typeof t === "object") {
      for (var k in t) {
        if (own(t, k) && TOKEN_RE.test(k) && typeof t[k] === "string" && t[k].length <= 1024) next[k] = t[k];
      }
    }
    theme = next;
    applyThemeVars();
    if (notify && ready) emit(themeSubs, assign({}, theme));
  }

  /* -------------------------------------------------------------- mode */

  function setMode(m, notify) {
    if (!isMode(m) || m === mode) return;
    mode = m;
    if (notify && ready) emit(modeSubs, mode);
  }

  /* ------------------------------------------------------------- state */

  function flushSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = null;
    if (pendingSave === undefined) return;
    var json = pendingSave;
    pendingSave = undefined;
    post({ type: "state", state: JSON.parse(json) });
  }

  /* -------------------------------------------------------- standalone */

  /* Presenter keys a clicker or keyboard sends. When the object has focus it
   * swallows them, so forward the ones the object did not use itself. */
  var PRESENTER_KEYS = { PageDown: 1, PageUp: 1, ArrowRight: 1, ArrowLeft: 1, ArrowDown: 1, ArrowUp: 1, Escape: 1 };

  function onKey(e) {
    if (!e || e.ctrlKey || e.altKey || e.metaKey) return;
    var t = e.target;
    var tag = t && t.tagName ? String(t.tagName).toUpperCase() : "";
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (t && t.isContentEditable)) return;
    var k = e.key;
    if (!standalone) {
      if (!hasParent || !PRESENTER_KEYS[k]) return;
      // Let the object's own handlers run first (any target or phase); forward
      // only keys it left alone. Objects that use arrows call preventDefault().
      setTimeout(function () {
        if (!e.defaultPrevented) post({ type: "key", key: k });
      }, 0);
      return;
    }
    if (e.defaultPrevented) return;
    var space = k === " " || k === "Spacebar";
    if (space && tag === "BUTTON") return;
    if (k === "ArrowRight" || k === "PageDown" || space) {
      if (typeof e.preventDefault === "function") e.preventDefault();
      api.steps.next();
    } else if (k === "ArrowLeft" || k === "PageUp") {
      if (typeof e.preventDefault === "function") e.preventDefault();
      api.steps.prev();
    }
  }

  function keys(on) {
    if (on === keysOn) return;
    keysOn = on;
    try {
      if (on) doc.addEventListener("keydown", onKey);
      else doc.removeEventListener("keydown", onKey);
    } catch (e) {
      /* no document events */
    }
  }

  /* ------------------------------------------------------------- ready */

  function initInfo() {
    return {
      id: objectId,
      step: current,
      steps: count,
      state: api.state.load(),
      theme: assign({}, theme),
      mode: mode,
      standalone: standalone
    };
  }

  function applyInit(msg) {
    standalone = false;
    keys(true); // embedded: forward presenter keys to the host
    objectId = typeof msg.id === "string" ? msg.id : msg.id == null ? "" : String(msg.id);
    var steps = toInt(msg.steps);
    count = clamp(steps === null ? 0 : steps, 0, MAX_STEPS);
    var step = toInt(msg.step);
    current = clamp(step === null ? 0 : step, 0, count);
    savedJson = msg.state === null || msg.state === undefined ? undefined : stringify(msg.state);
    if (isMode(msg.mode)) mode = msg.mode;
    setTheme(msg.theme, false);
  }

  function becomeReady(msg) {
    if (initTimer) clearTimeout(initTimer);
    initTimer = null;
    if (msg) {
      applyInit(msg);
    } else {
      standalone = true;
      objectId = "standalone";
      keys(true);
      consoleCall("info", [
        "[rupok] standalone mode (no rupok host): ArrowRight/Space/PageDown = next step, ArrowLeft/PageUp = previous."
      ]);
    }
    ready = true;
    var q = queue;
    queue = [];
    silent = true;
    for (var i = 0; i < q.length; i++) {
      try {
        q[i]();
      } catch (e) {
        consoleCall("error", ["[rupok] queued call failed:", e]);
      }
    }
    silent = false;
    resolveReady(initInfo());
    emit(stepSubs, current, stepDetail(current, "init"));
    emit(themeSubs, assign({}, theme));
  }

  /* An init after we are already ready: a late host, or a host re-init. */
  function adoptInit(msg) {
    var prev = current;
    var prevMode = mode;
    clearPending();
    applyInit(msg);
    if (declaredCount !== null && declaredCount !== count) {
      count = declaredCount;
      current = clamp(current, 0, count);
      post({ type: "steps", count: count });
    }
    emit(stepSubs, current, stepDetail(prev, "init"));
    emit(themeSubs, assign({}, theme));
    if (mode !== prevMode) emit(modeSubs, mode);
  }

  function onMessage(ev) {
    if (!hasParent || !ev) return;
    var source;
    try {
      source = ev.source;
    } catch (e) {
      return;
    }
    if (source !== root.parent) return;
    var d = ev.data;
    if (!d || typeof d !== "object" || d.rupok !== PROTOCOL || typeof d.type !== "string") return;
    if (d.type === "init") {
      if (ready) adoptInit(d);
      else becomeReady(d);
    } else if (d.type === "step") {
      var n = toInt(d.step);
      if (ready && !standalone && n !== null) {
        clearPending();
        setSteps(n, count, "host");
      }
    } else if (d.type === "theme") {
      if (!standalone) setTheme(d.theme, true);
    } else if (d.type === "mode") {
      if (!standalone) setMode(d.mode, true);
    }
  }

  function forwardError(err, prefix, line) {
    if (forwardedErrors >= MAX_FORWARDED_ERRORS) return;
    forwardedErrors++;
    var message = prefix + format([err]) + (line ? " (line " + line + ")" : "");
    whenReady(function () {
      post({ type: "log", level: "error", message: message.slice(0, MAX_LOG_CHARS) });
    });
  }

  /* --------------------------------------------------------------- API */

  var api = {
    version: SDK_VERSION,
    ready: readyPromise,
    get mode() {
      return mode;
    },
    onModeChange: function (cb) {
      return subscribe(modeSubs, cb, null);
    },
    steps: {
      get current() {
        return current;
      },
      get count() {
        return count;
      },
      onChange: function (cb) {
        return subscribe(stepSubs, cb, function (fn) {
          safeCall(fn, current, stepDetail(current, "init"));
        });
      },
      next: function () {
        whenReady(function () {
          var base = stepBase();
          if (base < count) requestStep(base + 1);
        });
      },
      prev: function () {
        whenReady(function () {
          var base = stepBase();
          if (base > 0) requestStep(base - 1);
        });
      },
      goto: function (n) {
        var v = toInt(n);
        if (v === null) {
          warn("steps.goto(n): n must be a number");
          return;
        }
        whenReady(function () {
          requestStep(v);
        });
      },
      setCount: function (n) {
        var v = toInt(n);
        if (v === null) {
          warn("steps.setCount(n): n must be a number");
          return;
        }
        v = clamp(v, 0, MAX_STEPS);
        declaredCount = v;
        whenReady(function () {
          setSteps(current, v, "local");
          post({ type: "steps", count: v });
        });
      }
    },
    state: {
      load: function () {
        if (!ready || savedJson === undefined) return undefined;
        try {
          return JSON.parse(savedJson);
        } catch (e) {
          return undefined;
        }
      },
      save: function (value) {
        if (!ready) {
          warn("state.save() before rupok.ready resolved is ignored. Load the saved state first, then save.");
          return;
        }
        var json = stringify(value === undefined ? null : value);
        if (json === undefined) {
          warn("state.save(): value is not JSON-serialisable; ignored.");
          return;
        }
        var bytes = utf8Length(json);
        if (bytes > MAX_STATE_BYTES) {
          warn("state.save(): state is " + bytes + " bytes, over the " + MAX_STATE_BYTES + " byte limit; dropped.");
          return;
        }
        savedJson = json === "null" ? undefined : json;
        if (standalone) return;
        pendingSave = json;
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(flushSave, SAVE_DEBOUNCE_MS);
      }
    },
    theme: {
      get: function (token, fallback) {
        return typeof token === "string" && own(theme, token) ? theme[token] : fallback;
      },
      all: function () {
        return assign({}, theme);
      },
      onChange: function (cb) {
        return subscribe(themeSubs, cb, function (fn) {
          safeCall(fn, assign({}, theme));
        });
      }
    },
    quiz: {
      submit: function (payload) {
        if (!payload || typeof payload !== "object") {
          warn("quiz.submit(payload): payload must be an object");
          return;
        }
        var qid = payload.questionId;
        if (typeof qid === "number" && isFinite(qid)) qid = String(qid);
        if (typeof qid !== "string" || !qid) {
          warn("quiz.submit(payload): payload.questionId (string) is required");
          return;
        }
        var answer = stringify(payload.answer === undefined ? null : payload.answer);
        if (answer === undefined) {
          warn("quiz.submit(payload): payload.answer is not JSON-serialisable");
          return;
        }
        var p = { questionId: qid, answer: JSON.parse(answer) };
        if (typeof payload.correct === "boolean") p.correct = payload.correct;
        if (typeof payload.score === "number" && isFinite(payload.score)) p.score = payload.score;
        whenReady(function () {
          if (standalone) consoleCall("info", ["[rupok] quiz", p]);
          else post({ type: "quiz", payload: p });
        });
      }
    },
    resize: function (w, h) {
      var nw = Number(w);
      var nh = Number(h);
      if (!isFinite(nw) || !isFinite(nh) || nw <= 0 || nh <= 0) {
        warn("resize(w, h): w and h must be positive numbers");
        return;
      }
      whenReady(function () {
        post({ type: "resize", w: nw, h: nh });
      });
    },
    log: function (level) {
      var args = Array.prototype.slice.call(arguments, 1);
      var lv = level;
      if (lv !== "info" && lv !== "warn" && lv !== "error") {
        args.unshift(level);
        lv = "info";
      }
      consoleCall(lv, args);
      var message = format(args);
      whenReady(function () {
        post({ type: "log", level: lv, message: message });
      });
    }
  };

  try {
    Object.defineProperty(api, "__rupoksdk", { value: SDK_VERSION });
  } catch (e) {
    api.__rupoksdk = SDK_VERSION;
  }
  root.rupok = api;

  /* ------------------------------------------------------------- start */

  applyThemeVars();
  try {
    root.addEventListener("message", onMessage);
    root.addEventListener("pagehide", flushSave);
    root.addEventListener("error", function (e) {
      forwardError(e && (e.error || e.message), "", e && e.lineno);
    });
    root.addEventListener("unhandledrejection", function (e) {
      forwardError(e && e.reason, "Unhandled promise rejection: ", 0);
    });
  } catch (e) {
    /* no window events */
  }

  if (hasParent) {
    keys(true);
    post({ type: "hello", sdk: SDK_VERSION });
    initTimer = setTimeout(function () {
      if (!ready) becomeReady(null);
    }, INIT_TIMEOUT_MS);
  } else {
    becomeReady(null);
  }
})(typeof window !== "undefined" ? window : undefined);
