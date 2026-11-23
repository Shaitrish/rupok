// KaTeX for typeset maths on the board, loaded on first use from the offline
// copy in public/rupoklib/katex (no network).

interface Katex {
  render(src: string, el: HTMLElement, opts: Record<string, unknown>): void;
  renderToString(src: string, opts: Record<string, unknown>): string;
}

let loading: Promise<Katex> | null = null;
const BASE = `${import.meta.env.BASE_URL}rupoklib/katex/`;

export function loadKatex(): Promise<Katex> {
  const w = window as unknown as { katex?: Katex };
  if (w.katex) return Promise.resolve(w.katex);
  loading ??= new Promise<Katex>((resolve, reject) => {
    if (!document.querySelector(`link[data-katex]`)) {
      const css = document.createElement("link");
      css.rel = "stylesheet";
      css.href = `${BASE}katex.min.css`;
      css.dataset.katex = "1";
      document.head.appendChild(css);
    }
    const s = document.createElement("script");
    s.src = `${BASE}katex.min.js`;
    s.onload = () => (w.katex ? resolve(w.katex) : reject(new Error("KaTeX did not load")));
    s.onerror = () => {
      loading = null;
      reject(new Error("KaTeX did not load"));
    };
    document.head.appendChild(s);
  });
  return loading;
}

/** Typeset `latex` into `el` (inline style); falls back to the source text. */
export async function renderTex(el: HTMLElement, latex: string, display = false): Promise<void> {
  try {
    const k = await loadKatex();
    k.render(latex, el, { throwOnError: false, displayMode: display, output: "htmlAndMathml", strict: "ignore" });
  } catch {
    el.textContent = latex;
  }
}
