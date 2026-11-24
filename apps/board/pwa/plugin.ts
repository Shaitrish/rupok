// Build-time PWA files (vite build only; dev has no service worker):
//
//   /sw.js         pwa/sw.js with this build's id and precache list
//   /version.json  { build, version, date }: open tabs poll it to learn that a
//                  new build is live (src/updates/updates.ts)
//   <meta name="rupok-build">  the same id in every HTML page
//
// The build id is a hash of the precached files' (content-hashed) names, so it
// changes exactly when the board changes. docs/offline.md.

import type { Plugin } from "vite";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

/** HTML entries whose whole module graph is precached (input names in vite.config.ts). */
const SHELL_ENTRIES: Record<string, string> = { board: "/board/", open: "/open/", "bn-open": "/bn/open/" };
/** Static files from public/ the app shell uses. */
const STATIC = ["/manifest.webmanifest", "/favicon.svg", "/icon-192.png", "/icon-512.png", "/icon-maskable-512.png", "/apple-touch-icon.png"];
/** Font fallbacks browsers skip when woff2 loads; source maps. */
const SKIP = /\.(woff|ttf|eot|map)$/;

/** The offline Smart Object libraries (public/rupoklib, from `bun run vendor`),
 * minus licences and font fallbacks: what an installed app caches in the background. */
function libraryFiles(): string[] {
  const root = fileURLToPath(new URL("../public/rupoklib/", import.meta.url));
  if (!existsSync(root)) return [];
  const out: string[] = [];
  const walk = (dir: string, rel: string) => {
    for (const d of readdirSync(dir, { withFileTypes: true })) {
      const r = rel ? `${rel}/${d.name}` : d.name;
      if (d.isDirectory()) walk(join(dir, d.name), r);
      else if (d.isFile() && !/^(LICENSE|README)/i.test(d.name) && !SKIP.test(d.name)) out.push(`/rupoklib/${r}`);
    }
  };
  walk(root, "");
  return out.sort();
}

export function rupokPwa(): Plugin {
  const version: string = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;
  return {
    name: "rupok-pwa",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const files = new Set<string>();
      const seen = new Set<string>();
      const addCssRefs = (name: string) => {
        const css = bundle[name];
        if (!css || css.type !== "asset") return;
        for (const m of String(css.source).matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/g)) {
          const ref = m[1].replace(/^\//, "").replace(/^(\.\.?\/)+/, "assets/");
          if (bundle[ref]) files.add(ref);
        }
      };
      const walk = (name: string) => {
        if (seen.has(name)) return;
        seen.add(name);
        const item = bundle[name];
        if (!item) return;
        files.add(name);
        if (item.type !== "chunk") return;
        const meta = (item as any).viteMetadata as { importedCss?: Set<string>; importedAssets?: Set<string> } | undefined;
        for (const c of meta?.importedCss ?? []) {
          files.add(c);
          addCssRefs(c);
        }
        for (const a of meta?.importedAssets ?? []) files.add(a);
        for (const r of item.referencedFiles ?? []) files.add(r);
        for (const i of [...item.imports, ...item.dynamicImports]) walk(i);
      };

      const pages: string[] = [];
      for (const item of Object.values(bundle)) {
        if (item.type === "chunk" && item.isEntry && SHELL_ENTRIES[item.name]) walk(item.fileName);
      }
      // The pages themselves, and what their HTML links (CSS, preloads).
      for (const url of Object.values(SHELL_ENTRIES)) {
        const htmlName = `${url.slice(1)}index.html`;
        const html = bundle[htmlName];
        if (!html || html.type !== "asset") continue;
        pages.push(url);
        for (const m of String(html.source).matchAll(/(?:src|href)="\/([^"]+)"/g)) {
          if (bundle[m[1]]) {
            files.add(m[1]);
            if (m[1].endsWith(".css")) addCssRefs(m[1]);
          }
        }
      }
      const assets = [...files].filter((f) => !f.endsWith(".html") && !SKIP.test(f)).sort();
      const precache = [...pages, ...STATIC, ...assets.map((f) => `/${f}`)];
      const build = createHash("sha256")
        .update(assets.join("\n"))
        .update(pages.map((p) => { const h = bundle[`${p.slice(1)}index.html`]; return h?.type === "asset" ? String(h.source) : ""; }).join("\n"))
        .digest("hex")
        .slice(0, 12);

      // Every page carries the id (the board compares it with /version.json).
      for (const item of Object.values(bundle)) {
        if (item.type === "asset" && item.fileName.endsWith(".html")) {
          item.source = String(item.source).replace("</head>", `  <meta name="rupok-build" content="${build}" />\n  </head>`);
        }
      }
      const sw = readFileSync(new URL("./sw.js", import.meta.url), "utf8")
        .replace("__RUPOK_BUILD__", build)
        .replace("__RUPOK_PRECACHE__", JSON.stringify(precache, null, 0))
        .replace("__RUPOK_LIBRARY__", JSON.stringify(libraryFiles(), null, 0));
      this.emitFile({ type: "asset", fileName: "sw.js", source: sw });
      this.emitFile({ type: "asset", fileName: "version.json", source: JSON.stringify({ build, version, date: new Date().toISOString() }) + "\n" });
      const bytes = assets.reduce((n, f) => {
        const it = bundle[f];
        return n + (it ? (it.type === "chunk" ? it.code.length : (it.source as string | Uint8Array).length) : 0);
      }, 0);
      this.info(`service worker: build ${build}, ${precache.length} files precached (${(bytes / 1e6).toFixed(1)} MB before compression)`);
    },
  };
}
