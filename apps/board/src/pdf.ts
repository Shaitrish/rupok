// PDF import: every PDF page becomes a board page with the slide as an image
// you can write on. This is the "Claude made slides → PDF → class" workflow.

import type { Controller } from "./controller";
import { showToast } from "./state";

type PdfJs = typeof import("pdfjs-dist");

/**
 * pdf.js's modern build calls recent built-ins without polyfills
 * (`Math.sumPrecise`, `Map.prototype.getOrInsertComputed`: Safari 26.2,
 * Chrome 147, Firefox 144). Older browsers, including Safari and the macOS
 * app's WKWebView before macOS 26, get the legacy build, which polyfills them.
 */
export function needsLegacyPdfjs(): boolean {
  const has = (o: object, k: string) => typeof (o as Record<string, unknown>)[k] === "function";
  return !(has(Math, "sumPrecise") && has(Map.prototype, "getOrInsertComputed") && has(Promise, "try"));
}

let pdfjsLoad: Promise<PdfJs> | null = null;
function loadPdfjs(): Promise<PdfJs> {
  pdfjsLoad ??= (async () => {
    const [lib, worker] = needsLegacyPdfjs()
      ? await Promise.all([import("pdfjs-dist/legacy/build/pdf.mjs"), import("pdfjs-dist/legacy/build/pdf.worker.min.mjs?url")])
      : await Promise.all([import("pdfjs-dist"), import("pdfjs-dist/build/pdf.worker.min.mjs?url")]);
    lib.GlobalWorkerOptions.workerSrc = worker.default;
    return lib as PdfJs;
  })().catch((e) => {
    pdfjsLoad = null; // let the next import try again
    throw e;
  });
  return pdfjsLoad;
}

const FRAME_W = 1920;
const FRAME_H = 1080;
/** Render at 1.5× the frame so slides stay crisp when zoomed in on a 4K panel. */
const QUALITY = 1.5;

export async function importPdf(c: Controller, file: File) {
  const pdfjs = await loadPdfjs();
  const doc = await pdfjs.getDocument({ data: new Uint8Array(await file.arrayBuffer()) }).promise;
  const n = doc.numPages;
  const pages = JSON.parse(c.board.pages_json());
  const reuseCurrent = pages[c.board.page_index()]?.item_count === 0;
  for (let i = 1; i <= n; i++) {
    showToast(`Importing ${file.name}: page ${i} of ${n}…`, 60000);
    const page = await doc.getPage(i);
    const base = page.getViewport({ scale: 1 });
    const fit = Math.min(FRAME_W / base.width, FRAME_H / base.height);
    const vp = page.getViewport({ scale: fit * QUALITY });
    const canvas = document.createElement("canvas");
    canvas.width = Math.ceil(vp.width);
    canvas.height = Math.ceil(vp.height);
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    await page.render({ canvas, canvasContext: ctx, viewport: vp }).promise;
    const blob: Blob = await new Promise((r) => canvas.toBlob((b) => r(b!), "image/jpeg", 0.9));
    if (!(i === 1 && reuseCurrent)) c.board.add_page("plain");
    c.board.fit_page();
    const bytes = new Uint8Array(await blob.arrayBuffer());
    c.board.add_image(bytes, "jpg", Math.round(base.width * fit), Math.round(base.height * fit));
    c.board.select_json("[]");
    page.cleanup();
  }
  if (reuseCurrent || n > 0) {
    // Go back to the first imported page.
    const first = c.board.page_index() - (n - 1);
    c.board.goto_page(Math.max(0, first));
  }
  if (c.board.title() === "Untitled lesson") c.board.set_title(file.name.replace(/\.pdf$/i, ""));
  showToast(`Imported ${n} page${n === 1 ? "" : "s"} from ${file.name}`);
  await (doc as any).destroy?.();
}
