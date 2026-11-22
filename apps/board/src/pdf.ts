// PDF import: every PDF page becomes a board page with the slide as an image
// you can write on. This is the "Claude made slides → PDF → class" workflow.

import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import type { Controller } from "./controller";
import { showToast } from "./state";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

const FRAME_W = 1920;
const FRAME_H = 1080;
/** Render at 1.5× the frame so slides stay crisp when zoomed in on a 4K panel. */
const QUALITY = 1.5;

export async function importPdf(c: Controller, file: File) {
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
