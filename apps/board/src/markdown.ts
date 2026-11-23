// A small, safe Markdown renderer for chat messages: HTML is escaped first,
// then headings, lists, code, quotes, emphasis and links (http/https only)
// are recognised. Maths ($…$, $$…$$) is typeset with KaTeX when it has
// loaded (lazily, on the first message that has maths).

let katex: any = null;
let katexLoading: Promise<void> | null = null;
const listeners = new Set<() => void>();

/** Call `fn` once KaTeX is ready (re-render then). */
export function onMathReady(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function loadKatex() {
  if (katex || katexLoading) return;
  katexLoading = Promise.all([import("katex"), import("katex/dist/katex.min.css")])
    .then(([k]) => {
      katex = (k as any).default ?? k;
      for (const fn of listeners) fn();
    })
    .catch(() => { /* maths stays as text */ });
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function math(tex: string, display: boolean): string {
  if (!katex) {
    loadKatex();
    return `<code class="md-math">${esc(tex)}</code>`;
  }
  try {
    return katex.renderToString(tex, { displayMode: display, throwOnError: false, output: "html" });
  } catch {
    return `<code class="md-math">${esc(tex)}</code>`;
  }
}

/** Inline spans of one line of (unescaped) text. */
function inline(src: string): string {
  // Pull out code and maths first so their contents are left alone.
  const slots: string[] = [];
  const keep = (html: string) => `\u0000${slots.push(html) - 1}\u0000`;
  let s = src
    .replace(/`([^`]+)`/g, (_, c) => keep(`<code>${esc(c)}</code>`))
    .replace(/\$\$([^$]+)\$\$/g, (_, t) => keep(math(t, true)))
    .replace(/(^|[^\\$\w])\$([^\s$](?:[^$]*[^\s$])?)\$(?![\w$])/g, (_, pre, t) => pre + keep(math(t, false)));
  s = esc(s)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[^*\w])\*([^*\s][^*]*)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/(^|[^_\w])_([^_\s][^_]*)_(?!\w)/g, "$1<em>$2</em>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => slots[Number(i)]);
}

/** Render Markdown to HTML (safe to put in innerHTML). */
export function renderMarkdown(src: string): string {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const out: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = /^```(\w*)\s*$/.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^```\s*$/.test(lines[i])) body.push(lines[i++]);
      i++;
      out.push(`<pre><code>${esc(body.join("\n"))}</code></pre>`);
      continue;
    }
    if (/^\$\$\s*$/.test(line)) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^\$\$\s*$/.test(lines[i])) body.push(lines[i++]);
      i++;
      out.push(`<div class="md-display">${math(body.join("\n"), true)}</div>`);
      continue;
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) {
      const level = Math.min(4, h[1].length + 2); // h3..h4 inside a chat bubble
      out.push(`<h${Math.min(level, 4)}>${inline(h[2])}</h${Math.min(level, 4)}>`);
      i++;
      continue;
    }
    if (/^\s*([-*+]|\d+[.)])\s+/.test(line)) {
      const ordered = /^\s*\d+[.)]/.test(line);
      const items: string[] = [];
      while (i < lines.length && /^\s*([-*+]|\d+[.)])\s+/.test(lines[i])) {
        items.push(`<li>${inline(lines[i].replace(/^\s*([-*+]|\d+[.)])\s+/, ""))}</li>`);
        i++;
      }
      out.push(ordered ? `<ol>${items.join("")}</ol>` : `<ul>${items.join("")}</ul>`);
      continue;
    }
    if (/^>\s?/.test(line)) {
      const q: string[] = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) q.push(lines[i++].replace(/^>\s?/, ""));
      out.push(`<blockquote>${q.map(inline).join("<br>")}</blockquote>`);
      continue;
    }
    if (!line.trim()) { i++; continue; }
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() && !/^(```|\$\$\s*$|#{1,4}\s|\s*([-*+]|\d+[.)])\s|>)/.test(lines[i])) para.push(lines[i++]);
    out.push(`<p>${para.map(inline).join("<br>")}</p>`);
  }
  return out.join("");
}
