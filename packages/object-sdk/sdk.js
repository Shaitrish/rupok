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
  var SANS = '"Inter Variable", Inter, "Hind Siliguri", system-ui, -apple-system, "Segoe UI", Roboto, "Noto Sans", Arial, sans-serif';
  /* Light defaults. The host sends a full theme (including "scheme") that
   * follows the board's page colour, so objects match light and dark boards. */
  var DEFAULT_THEME = {
    "scheme": "light",
    "color.bg": "#ffffff",
    "color.surface": "#f4f5f4",
    "color.line": "rgba(17, 20, 24, 0.12)",
    "color.fg": "#111418",
    "color.accent": "#0ca678",
    /* accent-coloured text (4.5:1 on the page), text on an accent fill, highlight background */
    "color.accent-text": "#087f5b",
    "color.on-accent": "#04241b",
    "color.accent-soft": "#e6fcf5",
    "color.accent2": "#1c7ed6",
    "color.muted": "#5f6670",
    "color.success": "#2b8a3e",
    "color.danger": "#e03131",
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
    // Expose the scheme so authors can write :root[data-scheme="dark"] rules,
    // and so native controls (scrollbars, inputs) match.
    var scheme = theme.scheme === "dark" ? "dark" : "light";
    try {
      if (el.setAttribute) el.setAttribute("data-scheme", scheme);
      style.colorScheme = scheme;
    } catch (e) {
      /* ignore */
    }
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
    } else if (d.type === "snapshot") {
      onSnapshot(d);
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

  /* ---------------------------------------------------------- snapshot */

  /* The host asks for a picture of the object (for example to put it in an
   * exported PDF). The SDK draws the page into a PNG: the DOM through an SVG
   * foreignObject, with every canvas (2D or WebGL) copied in place; if the
   * browser refuses that, just the canvases over the page background. */
  var SNAPSHOT_MAX_PIXELS = 4096 * 4096;
  var SNAPSHOT_MAX_CHARS = 16 * 1024 * 1024;
  var SNAPSHOT_FRAME_MS = 150;
  var SNAPSHOT_ID_RE = /^[A-Za-z0-9._:-]{1,64}$/;

  function onSnapshot(d) {
    var id = d.id;
    var okId = (typeof id === "string" && SNAPSHOT_ID_RE.test(id)) || (typeof id === "number" && isFinite(id));
    if (!okId) return;
    var scale = Number(d.scale);
    if (!isFinite(scale) || scale <= 0) scale = 2;
    var reply = function (fields) {
      fields.type = "snapshot";
      fields.id = id;
      post(fields);
    };
    var p;
    try {
      p = snapshot(clamp(scale, 0.25, 4));
    } catch (e) {
      p = Promise.reject(e);
    }
    p.then(
      function (url) {
        reply({ dataUrl: url });
      },
      function (e) {
        reply({ error: format([e || "snapshot failed"]).slice(0, 200) });
      }
    );
  }

  /* Run fn inside the next animation frame, right after the object's own
   * frame callbacks: a WebGL drawing buffer can only be read then. Hidden
   * or throttled frames never paint, so a timer runs it anyway. */
  function inFrame(fn) {
    return new Promise(function (resolve, reject) {
      var done = false;
      function run() {
        if (done) return;
        done = true;
        try {
          resolve(fn());
        } catch (e) {
          reject(e);
        }
      }
      try {
        if (typeof root.requestAnimationFrame === "function") root.requestAnimationFrame(run);
      } catch (e) {
        /* no animation frames */
      }
      setTimeout(run, SNAPSHOT_FRAME_MS);
    });
  }

  function newCanvas(w, h) {
    var c = doc.createElement("canvas");
    c.width = Math.max(1, Math.round(w));
    c.height = Math.max(1, Math.round(h));
    return c;
  }

  function list(nodes) {
    return nodes ? Array.prototype.slice.call(nodes) : [];
  }

  function viewSize() {
    var el = doc.documentElement || {};
    var w = Number(root.innerWidth) || Number(el.clientWidth) || 0;
    var h = Number(root.innerHeight) || Number(el.clientHeight) || 0;
    return [Math.max(1, Math.round(w)), Math.max(1, Math.round(h))];
  }

  /* Copies of the canvases on screen, taken inside a frame. */
  function copyCanvases(w, h) {
    var out = [];
    var all = list(doc.querySelectorAll ? doc.querySelectorAll("canvas") : null);
    for (var i = 0; i < all.length; i++) {
      var el = all[i];
      try {
        var r = el.getBoundingClientRect();
        if (!(r.width > 0 && r.height > 0 && el.width > 0 && el.height > 0)) continue;
        if (r.right <= 0 || r.bottom <= 0 || r.left >= w || r.top >= h) continue;
        var cs = root.getComputedStyle ? root.getComputedStyle(el) : null;
        if (cs && (cs.visibility === "hidden" || cs.display === "none" || Number(cs.opacity) === 0)) continue;
        var copy = newCanvas(el.width, el.height);
        copy.getContext("2d").drawImage(el, 0, 0);
        out.push({ el: el, rect: { left: r.left, top: r.top, width: r.width, height: r.height }, copy: copy });
      } catch (e) {
        /* unreadable canvas: left out */
      }
    }
    return out;
  }

  function loadImage(src) {
    return new Promise(function (resolve, reject) {
      if (typeof root.Image !== "function") {
        reject(new Error("no Image"));
        return;
      }
      var img = new root.Image();
      img.onload = function () {
        resolve(img);
      };
      img.onerror = function () {
        reject(new Error("the page could not be drawn"));
      };
      img.src = src;
    });
  }

  /* The page as an SVG foreignObject image (canvases and form values
   * frozen into the copy). External stylesheets and fonts do not load
   * inside an SVG image; inline styles and inline SVG do. */
  function domImage(w, h, copies) {
    if (typeof root.XMLSerializer !== "function" || !doc.documentElement || !doc.documentElement.cloneNode) {
      return Promise.reject(new Error("no DOM serialiser"));
    }
    var html = doc.documentElement;
    var clone = html.cloneNode(true);
    var q = function (node, sel) {
      return list(node.querySelectorAll(sel));
    };
    // Live form values are properties, not attributes: copy them over.
    var srcFields = q(html, "input, textarea, select");
    var dstFields = q(clone, "input, textarea, select");
    for (var i = 0; i < srcFields.length && i < dstFields.length; i++) {
      var a = srcFields[i];
      var b = dstFields[i];
      var tag = String(a.tagName).toLowerCase();
      if (tag === "textarea") b.textContent = a.value;
      else if (tag === "select") {
        var opts = q(b, "option");
        for (var k = 0; k < opts.length; k++) {
          if (k === a.selectedIndex) opts[k].setAttribute("selected", "");
          else opts[k].removeAttribute("selected");
        }
      } else if (a.type === "checkbox" || a.type === "radio") {
        if (a.checked) b.setAttribute("checked", "");
        else b.removeAttribute("checked");
      } else b.setAttribute("value", a.value);
    }
    // Images the object can read become data URLs (others cannot load).
    var srcImgs = q(html, "img");
    var dstImgs = q(clone, "img");
    for (var m = 0; m < srcImgs.length && m < dstImgs.length; m++) {
      var im = srcImgs[m];
      if (!im.complete || !im.naturalWidth || /^data:/.test(im.src)) continue;
      try {
        var ic = newCanvas(im.naturalWidth, im.naturalHeight);
        ic.getContext("2d").drawImage(im, 0, 0);
        dstImgs[m].setAttribute("src", ic.toDataURL("image/png"));
      } catch (e) {
        /* cross-origin: left as is */
      }
    }
    // Canvases become images of their frozen copies, in place.
    var srcCanvases = q(html, "canvas");
    var dstCanvases = q(clone, "canvas");
    for (var n = 0; n < srcCanvases.length && n < dstCanvases.length; n++) {
      var found = null;
      for (var c = 0; c < copies.length; c++) if (copies[c].el === srcCanvases[n]) found = copies[c];
      var node = dstCanvases[n];
      if (!found) continue;
      var img = doc.createElement("img");
      var attrs = ["id", "class", "style"];
      for (var t = 0; t < attrs.length; t++) {
        var v = node.getAttribute(attrs[t]);
        if (v !== null) img.setAttribute(attrs[t], v);
      }
      img.style.width = found.rect.width + "px";
      img.style.height = found.rect.height + "px";
      img.setAttribute("src", found.copy.toDataURL("image/png"));
      node.parentNode.replaceChild(img, node);
    }
    var dead = q(clone, "script, iframe, noscript, object, embed");
    for (var z = 0; z < dead.length; z++) if (dead[z].parentNode) dead[z].parentNode.removeChild(dead[z]);
    clone.style.width = w + "px";
    clone.style.height = h + "px";
    var xml = new root.XMLSerializer().serializeToString(clone);
    var svg =
      '<svg xmlns="http://www.w3.org/2000/svg" width="' + w + '" height="' + h + '" viewBox="0 0 ' + w + " " + h + '">' +
      '<foreignObject x="0" y="0" width="' + w + '" height="' + h + '">' + xml + "</foreignObject></svg>";
    return loadImage("data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg));
  }

  function backgroundColor() {
    var picks = [doc.body, doc.documentElement];
    for (var i = 0; i < picks.length; i++) {
      try {
        var bg = picks[i] && root.getComputedStyle ? root.getComputedStyle(picks[i]).backgroundColor : "";
        if (bg && bg !== "transparent" && !/rgba\([^)]*,\s*0\)$/.test(bg)) return bg;
      } catch (e) {
        /* no computed style */
      }
    }
    return theme["color.bg"] || "#ffffff";
  }

  /* Draw `img` (the DOM picture) or, without it, the canvas copies over the
   * page background, at scale s. Halves the scale while the PNG is too big. */
  function encode(w, h, s, img, copies) {
    for (var tries = 0; tries < 4; tries++, s /= 2) {
      var out = newCanvas(w * s, h * s);
      var ctx = out.getContext("2d");
      if (img) ctx.drawImage(img, 0, 0, w * s, h * s);
      else {
        ctx.fillStyle = backgroundColor();
        ctx.fillRect(0, 0, w * s, h * s);
        for (var i = 0; i < copies.length; i++) {
          var r = copies[i].rect;
          ctx.drawImage(copies[i].copy, r.left * s, r.top * s, r.width * s, r.height * s);
        }
      }
      var url = out.toDataURL("image/png");
      if (typeof url === "string" && url.length <= SNAPSHOT_MAX_CHARS && /^data:image\/png[;,]/.test(url)) return url;
    }
    throw new Error("snapshot too large");
  }

  function snapshot(scale) {
    var size = viewSize();
    var w = size[0];
    var h = size[1];
    var s = Math.min(scale, Math.sqrt(SNAPSHOT_MAX_PIXELS / (w * h)));
    return inFrame(function () {
      return copyCanvases(w, h);
    }).then(function (copies) {
      return domImage(w, h, copies)
        .then(function (img) {
          return encode(w, h, s, img, copies);
        })
        .catch(function (e) {
          // Tainted, unsupported or broken: fall back to the canvases.
          if (!copies.length) throw e;
          return encode(w, h, s, null, copies);
        });
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

  /* ------------------------------------------------------------- maths */
  /* Built-in LaTeX (spec/smart-object.md, "Maths (LaTeX)"). Text written as
   * $…$, $$…$$, \(…\) or \[…\], and [data-tex] elements, is typeset with the
   * KaTeX the host vendors in rupoklib. KaTeX loads lazily: only when the page
   * contains maths or calls rupok.tex(). The host names the library base URL in
   * <meta name="rupok-lib" content="…/rupoklib/"> (written before the SDK).
   * <meta name="rupok-math" content="off"> turns automatic rendering off;
   * rupok.tex() and rupok.math.render() still work. \hl{…} highlights a term in
   * the theme accent. Maths inherits the text colour, so dark boards just work. */

  var MATH_SKIP = { SCRIPT: 1, STYLE: 1, TEXTAREA: 1, PRE: 1, CODE: 1, NOSCRIPT: 1, TEMPLATE: 1, OPTION: 1, SVG: 1, MATH: 1, CANVAS: 1 };
  var MATH_MACROS = { "\\hl": "\\htmlClass{rupok-hl}{#1}" };
  var MATH_CSS =
    /* \hl: accent-text on accent-soft, AA contrast in both schemes */
    ".rupok-hl{color:var(--rupok-color-accent-text,#087f5b);background:var(--rupok-color-accent-soft,#e6fcf5);" +
    "border-radius:.2em;padding:0 .12em;margin:0 -.02em}" +
    ".rupok-math-display{display:block;margin:.4em 0;text-align:center}";
  var mathState = 0; /* 0 idle, 1 loading, 2 ready, -1 unavailable */
  var mathWaiters = [];
  var mathStyled = false;
  var mathObserver = null;
  var mathDirty = [];
  var mathFlushQueued = false;

  function metaContent(name) {
    try {
      var m = typeof doc.querySelector === "function" ? doc.querySelector('meta[name="' + name + '"]') : null;
      return m && typeof m.getAttribute === "function" ? String(m.getAttribute("content") || "").trim() : "";
    } catch (e) {
      return "";
    }
  }

  function katexLib() {
    var k = root.katex;
    return k && typeof k.render === "function" ? k : null;
  }

  function styleMath() {
    if (mathStyled) return;
    mathStyled = true;
    try {
      var s = doc.createElement("style");
      s.setAttribute("data-rupok", "math");
      s.textContent = MATH_CSS;
      (doc.head || doc.documentElement).appendChild(s);
    } catch (e) {
      /* no DOM */
    }
  }

  function settleMath(ok) {
    mathState = ok ? 2 : -1;
    if (ok) {
      styleMath();
      /* Fetch the main maths fonts now, so a later formula doesn't flash. */
      try {
        if (doc.fonts && typeof doc.fonts.load === "function") {
          doc.fonts.load("1em KaTeX_Main");
          doc.fonts.load("italic 1em KaTeX_Math");
        }
      } catch (e) {
        /* optional */
      }
    }
    var w = mathWaiters;
    mathWaiters = [];
    for (var i = 0; i < w.length; i++) safeCall(w[i], ok);
  }

  /* Load KaTeX (JS + CSS; the CSS pulls its fonts) once. Resolves to whether it is usable. */
  function loadMath() {
    return new Promise(function (resolve) {
      if (katexLib()) {
        if (mathState !== 2) settleMath(true);
        resolve(true);
        return;
      }
      if (mathState === -1) {
        resolve(false);
        return;
      }
      mathWaiters.push(resolve);
      if (mathState === 1) return;
      var base = metaContent("rupok-lib");
      if (!base || typeof doc.createElement !== "function") {
        warn('maths: KaTeX is not available (no <meta name="rupok-lib"> from the host); LaTeX stays as source text.');
        settleMath(false);
        return;
      }
      if (base.charAt(base.length - 1) !== "/") base += "/";
      mathState = 1;
      var pending = 2;
      var scriptOk = false;
      var done = function () {
        if (--pending === 0) settleMath(scriptOk && !!katexLib());
      };
      try {
        var link = doc.createElement("link");
        link.rel = "stylesheet";
        link.href = base + "katex/katex.min.css";
        link.onload = link.onerror = done; /* the CSS is cosmetic; render either way */
        var script = doc.createElement("script");
        script.src = base + "katex/katex.min.js";
        script.onload = function () {
          scriptOk = true;
          done();
        };
        script.onerror = function () {
          warn("maths: could not load " + script.src);
          done();
        };
        var head = doc.head || doc.documentElement;
        head.appendChild(link);
        head.appendChild(script);
      } catch (e) {
        settleMath(false);
      }
    });
  }

  function mathOptions(display) {
    return {
      displayMode: !!display,
      throwOnError: false,
      strict: "ignore",
      macros: assign({}, MATH_MACROS),
      /* Only the \hl macro's class is trusted: no links, styles or other HTML. */
      trust: function (c) {
        return !!c && c.command === "\\htmlClass" && c["class"] === "rupok-hl";
      }
    };
  }

  function renderInto(el, latex, display) {
    var k = katexLib();
    if (!k) return false;
    try {
      k.render(latex, el, mathOptions(display));
      el.__rupokTex = (display ? "D" : "I") + latex;
      return true;
    } catch (e) {
      consoleCall("warn", ["[rupok] maths:", e && e.message ? e.message : e]);
      return false;
    }
  }

  /* Split text into plain strings and { tex, display } parts, or null if it has no maths.
   * $…$ follows the pandoc rule: no space just inside the dollars and no digit
   * right after the closing one, so "$5 and $10" stays text. \$ is a literal $. */
  function splitMath(s) {
    if (typeof s !== "string" || (s.indexOf("$") < 0 && s.indexOf("\\(") < 0 && s.indexOf("\\[") < 0)) return null;
    var out = [];
    var buf = "";
    var found = false;
    var escaped = false;
    var i = 0;
    var n = s.length;
    var push = function (tex, display, end) {
      if (buf) out.push(buf);
      buf = "";
      out.push({ tex: tex, display: display });
      found = true;
      i = end;
    };
    while (i < n) {
      var c = s.charAt(i);
      var d = s.charAt(i + 1);
      if (c === "\\" && d === "$") {
        buf += "$";
        escaped = true;
        i += 2;
        continue;
      }
      if (c === "\\" && (d === "(" || d === "[")) {
        var j = s.indexOf(d === "(" ? "\\)" : "\\]", i + 2);
        if (j > i + 2) {
          push(s.slice(i + 2, j), d === "[", j + 2);
          continue;
        }
      }
      if (c === "$" && d === "$") {
        var k = s.indexOf("$$", i + 2);
        if (k > i + 2) {
          push(s.slice(i + 2, k), true, k + 2);
          continue;
        }
      } else if (c === "$" && d && !/\s/.test(d)) {
        for (var m = i + 1; m < n; m++) {
          var e = s.charAt(m);
          if (e === "\\") {
            m++;
          } else if (e === "$") {
            if (!/\s/.test(s.charAt(m - 1)) && !/[0-9]/.test(s.charAt(m + 1))) break;
          } else if (e === "\n" && s.charAt(m + 1) === "\n") {
            m = n;
          }
        }
        if (m < n) {
          push(s.slice(i + 1, m), false, m + 1);
          continue;
        }
      }
      buf += c;
      i++;
    }
    if (buf) out.push(buf);
    return found || escaped ? out : null;
  }

  function attrOf(node, name) {
    try {
      return typeof node.getAttribute === "function" ? node.getAttribute(name) : null;
    } catch (e) {
      return null;
    }
  }

  function hasClass(node, name) {
    var c = attrOf(node, "class");
    return !!c && (" " + c + " ").indexOf(" " + name + " ") >= 0;
  }

  /* An element whose content the maths scanner leaves alone. */
  function mathOpaque(node) {
    var tag = String(node.tagName || "").toUpperCase();
    return !!(MATH_SKIP[tag] || hasClass(node, "katex") || hasClass(node, "rupok-math") ||
      attrOf(node, "data-no-math") !== null || node.isContentEditable === true);
  }

  function insideMath(node) {
    for (var p = node, depth = 0; p && p !== doc && depth < 64; p = p.parentNode, depth++) {
      if (p.nodeType === 1 && mathOpaque(p) && attrOf(p, "data-tex") === null) return true;
    }
    return false;
  }

  function scanMath(start, jobs) {
    var stack = [start];
    while (stack.length) {
      var node = stack.pop();
      if (!node) continue;
      if (node.nodeType === 3) {
        var parts = splitMath(node.nodeValue);
        if (parts) jobs.push({ node: node, parts: parts });
        continue;
      }
      if (node.nodeType !== 1 && node.nodeType !== 9 && node.nodeType !== 11) continue;
      if (node.nodeType === 1) {
        var tex = attrOf(node, "data-tex");
        if (tex !== null) {
          jobs.push({ el: node, tex: tex, display: attrOf(node, "data-display") !== null });
          continue;
        }
        if (mathOpaque(node)) continue;
      }
      var kids = node.childNodes || [];
      for (var i = kids.length - 1; i >= 0; i--) stack.push(kids[i]);
    }
  }

  function applyMath(jobs) {
    var n = 0;
    for (var i = 0; i < jobs.length; i++) {
      var job = jobs[i];
      if (job.el) {
        if (job.el.__rupokTex !== (job.display ? "D" : "I") + job.tex && renderInto(job.el, job.tex, job.display)) n++;
        continue;
      }
      var node = job.node;
      var parent = node.parentNode;
      if (!parent || node.nodeValue === null) continue;
      for (var j = 0; j < job.parts.length; j++) {
        var p = job.parts[j];
        if (typeof p === "string") {
          parent.insertBefore(doc.createTextNode(p), node);
          continue;
        }
        var span = doc.createElement("span");
        span.setAttribute("class", p.display ? "rupok-math rupok-math-display" : "rupok-math");
        if (renderInto(span, p.tex, p.display)) n++;
        else span.textContent = p.display ? "$$" + p.tex + "$$" : "$" + p.tex + "$";
        parent.insertBefore(span, node);
      }
      parent.removeChild(node);
    }
    return n;
  }

  /* Typeset the maths under `start` (the whole body by default). Resolves to the number of formulas rendered. */
  function renderMath(start) {
    var top = start || doc.body;
    if (!top) return Promise.resolve(0);
    var jobs = [];
    scanMath(top, jobs);
    if (!jobs.length) return Promise.resolve(0);
    if (katexLib()) {
      if (mathState !== 2) settleMath(true);
      return Promise.resolve(applyMath(jobs));
    }
    return loadMath().then(function (ok) {
      if (!ok) return 0;
      var again = [];
      scanMath(top, again); /* the page may have changed while KaTeX loaded */
      return applyMath(again);
    });
  }

  function tex(el, latex, options) {
    if (!el || typeof el !== "object" || typeof latex !== "string") {
      warn("tex(el, latex, options): el must be an element and latex a string");
      return Promise.resolve(false);
    }
    var display = !!(options && options.display);
    var key = (display ? "D" : "I") + latex;
    try {
      el.__rupokTexWant = key;
      if (katexLib()) {
        if (mathState !== 2) settleMath(true);
        return Promise.resolve(el.__rupokTex === key || renderInto(el, latex, display));
      }
    } catch (e) {
      return Promise.resolve(false);
    }
    /* Until KaTeX arrives the element keeps its current content (a fallback, if any). */
    return loadMath().then(function (ok) {
      if (!ok || el.__rupokTexWant !== key) return false;
      return el.__rupokTex === key || renderInto(el, latex, display);
    });
  }

  /* Re-typeset maths that appears later (innerHTML, textContent, data-tex changes). */
  function flushMathDirty() {
    mathFlushQueued = false;
    var list = mathDirty;
    mathDirty = [];
    for (var i = 0; i < list.length; i++) {
      var node = list[i];
      if (node && node.parentNode && !insideMath(node)) renderMath(node);
    }
  }

  function watchMath() {
    if (mathObserver || typeof root.MutationObserver !== "function" || !doc.body) return;
    try {
      mathObserver = new root.MutationObserver(function (records) {
        for (var i = 0; i < records.length; i++) {
          var r = records[i];
          if (r.type === "childList") {
            for (var j = 0; j < r.addedNodes.length; j++) mathDirty.push(r.addedNodes[j]);
          } else mathDirty.push(r.type === "characterData" ? r.target.parentNode : r.target);
        }
        if (!mathFlushQueued && mathDirty.length) {
          mathFlushQueued = true;
          later(flushMathDirty);
        }
      });
      mathObserver.observe(doc.body, { childList: true, characterData: true, subtree: true, attributes: true, attributeFilter: ["data-tex"] });
    } catch (e) {
      mathObserver = null;
    }
  }

  function startMath() {
    var go = function () {
      if (metaContent("rupok-math").toLowerCase() === "off") return;
      renderMath(doc.body);
      watchMath();
    };
    try {
      if (doc.readyState === "loading" && typeof doc.addEventListener === "function") doc.addEventListener("DOMContentLoaded", go);
      else later(go);
    } catch (e) {
      /* no DOM */
    }
  }

  api.tex = tex;
  api.math = {
    render: function (el) {
      try {
        return renderMath(el && typeof el === "object" ? el : null);
      } catch (e) {
        return Promise.resolve(0);
      }
    },
    load: function () {
      try {
        return loadMath();
      } catch (e) {
        return Promise.resolve(false);
      }
    }
  };
  startMath();

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
