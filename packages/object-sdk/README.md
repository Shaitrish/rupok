# @rupok/object-sdk

The in-object SDK for [rupok](https://github.com/shaitrish/rupok) Smart
Objects: live HTML (diagrams, simulations, quizzes, games) placed on lesson
pages. It defines `window.rupok` and speaks the Rupok bridge protocol v1
with the host app.

- One dependency-free file (`sdk.js`, ES2019, about 5 KB gzipped).
- Steps (slide builds), saved state, theme tokens as CSS variables, quiz
  answers, logging.
- **Standalone mode**: open an object in a normal browser and step through it
  with the arrow keys.
- No method ever throws, even before the host has answered.

The full contract is in [`spec/smart-object.md`](../../spec/smart-object.md).

## You usually don't install this

The Rupok host **injects the SDK** into every Smart Object before the
object's own scripts run. An object just uses the global:

```html
<script>
  rupok.steps.setCount(3);
  rupok.steps.onChange((step) => render(step));
</script>
```

Use this package to:

- preview an object outside Rupok: add `<script src="path/to/sdk.js"></script>`
  while developing (a host-injected copy wins if both load);
- get editor types: `/// <reference types="@rupok/object-sdk" />`;
- write your own host or tests against the reference implementation.

## API

| Member | What it does |
|---|---|
| `rupok.version` | `"1"` |
| `rupok.ready` | `Promise<InitInfo>`: resolves when the host sends `init`, or with defaults in standalone mode. Never rejects. |
| `rupok.mode` | `"edit"`, `"present"`, `"replay"` or `"student"`. `"present"` until `init`. |
| `rupok.onModeChange(cb)` | `cb(mode)` on change. Returns an unsubscribe function. |
| `rupok.steps.current` / `.count` | Current step in `[0, count]`; `count = 0` is static. |
| `rupok.steps.onChange(cb)` | `cb(current, { previous, count, source })`: once with the initial step, then on every change. Returns an unsubscribe function. |
| `rupok.steps.next()` / `.prev()` / `.goto(n)` | Ask the host for a step change (applied locally in standalone mode). |
| `rupok.steps.setCount(n)` | Declare the step count at run time (0–500). |
| `rupok.state.load()` | Saved JSON state, or `undefined`. |
| `rupok.state.save(value)` | Persist a JSON value. Debounced 200 ms; over 256 KiB is dropped with a warning; ignored before `ready`. |
| `rupok.theme.get(token, fallback)` | e.g. `get("color.accent", "#0ca678")` |
| `rupok.theme.all()` / `.onChange(cb)` | Whole theme; change notifications. |
| `rupok.quiz.submit({ questionId, answer, correct?, score? })` | Record an answer with the host. |
| `rupok.resize(w, h)` | Ask for a new size (usually honoured only in edit mode). |
| `rupok.log(level, ...args)` | Console plus host log. |
| `rupok.tex(el, latex, { display })` | Typeset LaTeX into `el` with KaTeX. `\hl{…}` highlights a term. Re-rendering the same formula is free. |
| `rupok.math.render(root)` / `.load()` | Typeset `$…$`, `$$…$$`, `\(…\)`, `\[…\]` and `[data-tex]` under `root`; load KaTeX now. |

**Maths is built in.** Write `$E = mc^2$` (inline) or `$$\frac{a}{b}$$` (display)
in your HTML and the SDK typesets it with the KaTeX the host ships, loading it
only when the page has maths. Text in `<code>`, `<pre>` and `[data-no-math]` is left
alone; `$5 and $10` stays text. Opt out with `<meta name="rupok-math" content="off">`.

Theme tokens are also CSS variables on `:root`: `color.accent` becomes
`--rupok-color-accent`, and so on. Defaults apply before `init`. Always give a
fallback so the file renders without the SDK:

```css
button { background: var(--rupok-color-accent, #0ca678); color: var(--rupok-color-on-accent, #04241b); }
.note { color: var(--rupok-color-accent-text, #087f5b); }   /* accent text: never color.accent */
mark { background: var(--rupok-color-accent-soft, #e6fcf5); color: var(--rupok-color-accent-text, #087f5b); }
```

## Patterns

**Render each step from scratch.** The presenter can jump, go back or reload
mid-lesson:

```js
function render(step) {
  items.forEach((el, i) => el.classList.toggle("shown", i < step));
}
rupok.steps.setCount(items.length);
rupok.steps.onChange(render);
```

**Load, then save:**

```js
rupok.ready.then(() => {
  const saved = rupok.state.load() || { answers: {} };
  draw(saved);
  button.onclick = () => {
    saved.answers.q1 = "B";
    rupok.state.save(saved);
  };
});
```

## Standalone mode

With no parent window, or no `init` within 600 ms, `ready` resolves with
`standalone: true` and the SDK drives steps itself: ArrowRight, Space and
PageDown go forward; ArrowLeft and PageUp go back. State is kept in memory.
If a host answers late, the SDK switches over to it.

## Develop

```sh
node --test "packages/object-sdk/test/*.test.mjs"   # from the repo root
# or: cd packages/object-sdk && npm test
```

The tests run the real `sdk.js` in a `node:vm` context with a fake window,
parent and document.

## License

Apache-2.0. See [LICENSE](LICENSE). The Rupok app itself is AGPL-3.0-or-later;
this SDK, the specs and the format crate are Apache-2.0 so anyone can build
compatible objects and tools. See [COPYING.md](../../COPYING.md).
