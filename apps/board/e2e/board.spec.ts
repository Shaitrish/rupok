// Key board flows in WebKit (Safari, WKWebView on macOS, WebKitGTK on Linux):
// load, draw with pointer events, undo with ⌘Z/Ctrl+Z, insert a Smart Object
// preset, Teach mode, save and reopen a lesson, the support card, and the
// website pages.
// Shortcuts use ControlOrMeta, so on macOS they are the ⌘ shortcuts.

import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

/** Uncaught page errors fail the test (console noise does not). */
function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("crash", () => errors.push("the page crashed"));
  return errors;
}

async function openBoard(page: Page) {
  await page.goto("/board/");
  await expect(page.locator(".loading")).toHaveCount(0, { timeout: 30_000 });
  await expect(page.locator("canvas.ink")).toBeVisible();
}

/** The page frame on screen: where strokes go. */
async function frameBox(page: Page) {
  const box = await page.locator(".page-frame").first().boundingBox();
  expect(box, "the page frame is on screen").not.toBeNull();
  return box!;
}

/** Draw a zig-zag across the middle of the page frame with the mouse. */
async function scribble(page: Page, dy = 0) {
  const b = await frameBox(page);
  const [x0, y0] = [b.x + b.width * 0.3, b.y + b.height * 0.5 + dy];
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  for (let i = 1; i <= 24; i++) {
    await page.mouse.move(x0 + i * (b.width * 0.016), y0 + (i % 2 ? -40 : 40), { steps: 2 });
  }
  await page.mouse.up();
}

/** A picture of the ink canvas (the engine draws it with WebGL2). */
const inkShot = (page: Page) => page.locator("canvas.ink").screenshot();

/** A one-page PDF with a red rectangle. */
function tinyPdf(): Buffer {
  const content = "0.9 0.2 0.2 rg 40 40 240 100 re f";
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 180] /Contents 4 0 R >>",
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
  ];
  let pdf = "%PDF-1.4\n";
  const offsets = objects.map((o, i) => {
    const at = pdf.length;
    pdf += `${i + 1} 0 obj\n${o}\nendobj\n`;
    return at;
  });
  const xref = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  pdf += offsets.map((o) => `${String(o).padStart(10, "0")} 00000 n \n`).join("");
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf, "latin1");
}

/** Open a PDF with ⌘O / Ctrl+O: every page becomes a slide image. */
async function importPdf(page: Page) {
  const chooser = page.waitForEvent("filechooser");
  await page.keyboard.press("ControlOrMeta+o");
  await (await chooser).setFiles({ name: "slides.pdf", mimeType: "application/pdf", buffer: tinyPdf() });
  await expect(page.getByRole("status")).toContainText("Imported 1 page");
  await expect(page.locator(".item.image").first()).toBeVisible();
}

test.describe("board in WebKit", () => {
  test("loads with a working renderer", async ({ page }) => {
    const errors = trackErrors(page);
    await openBoard(page);
    await expect(page.locator(".page-frame").first()).toBeVisible();
    // The renderer error replaces the loading screen's tagline when the GPU fails.
    await expect(page.locator(".loading .err")).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("draws with pointer events and undoes with the platform shortcut", async ({ page }) => {
    const errors = trackErrors(page);
    await openBoard(page);
    await page.keyboard.press("p");
    const blank = await inkShot(page);
    await scribble(page);
    await expect.poll(async () => (await inkShot(page)).equals(blank), { message: "the stroke shows up" }).toBe(false);
    const undo = page.getByRole("button", { name: "Undo" }).first();
    await expect(undo).toBeEnabled();

    await page.keyboard.press("ControlOrMeta+z");
    await expect(undo).toBeDisabled();
    await page.keyboard.press("ControlOrMeta+Shift+z");
    await expect(undo).toBeEnabled();
    expect(errors).toEqual([]);
  });

  test("selects, copies and pastes with the platform shortcuts", async ({ page }) => {
    const errors = trackErrors(page);
    await openBoard(page);
    await page.keyboard.press("p");
    await scribble(page);
    await page.keyboard.press("ControlOrMeta+a");
    await expect(page.locator(".sel-frame")).toBeVisible();
    await page.keyboard.press("ControlOrMeta+c");
    await expect(page.getByRole("status")).toContainText("Copied 1 item");
    const before = await inkShot(page);
    await page.keyboard.press("ControlOrMeta+v");
    await expect.poll(async () => (await inkShot(page)).equals(before), { message: "the copy shows up" }).toBe(false);
    expect(errors).toEqual([]);
  });

  test("inserts a Smart Object preset that runs", async ({ page }) => {
    const errors = trackErrors(page);
    await openBoard(page);
    // The top bar's "Smart Object" button, or "Smart Object…" in an Insert menu.
    const direct = page.locator('button[title^="Add a Smart Object"]');
    if (await direct.count()) {
      await direct.first().click();
    } else {
      await page.locator('button[title^="Insert"]').first().click();
      await page.getByText("Smart Object…", { exact: true }).click();
    }
    const card = page.locator(".preset-card").first();
    await expect(card).toBeVisible();
    await card.click();

    const object = page.locator(".item.object").first();
    await expect(object).toBeVisible();
    // The object runs in a sandboxed srcdoc iframe and draws its own content.
    const frame = page.frameLocator(".item.object iframe").first();
    await expect.poll(() => frame.locator("body *").count(), { message: "the object renders" }).toBeGreaterThan(0);
    expect(errors).toEqual([]);
  });

  test("Teach mode presents the slide and Esc leaves it", async ({ page }) => {
    const errors = trackErrors(page);
    await openBoard(page);
    await page.locator('[data-mode="teach"]').first().click();
    await expect(page.locator("html")).toHaveAttribute("data-mode", "teach");
    // Writing on the slide works in Teach.
    const before = await inkShot(page);
    await scribble(page);
    await expect.poll(async () => (await inkShot(page)).equals(before)).toBe(false);
    await page.keyboard.press("Escape");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "prepare");
    // F5 presents too.
    await page.keyboard.press("F5");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "teach");
    expect(errors).toEqual([]);
  });

  test("imports a PDF as slides", async ({ page }) => {
    const errors = trackErrors(page);
    await openBoard(page);
    await importPdf(page);
    expect(errors).toEqual([]);
  });

  test("imports a PDF on an older WebKit (pdf.js legacy build)", async ({ page }) => {
    // Safari before 26.2 (every Mac before macOS 26, and the macOS app's
    // WKWebView there) lacks these; pdf.js's modern build needs them.
    await page.addInitScript(() => {
      delete (Math as any).sumPrecise;
      delete (Map.prototype as any).getOrInsertComputed;
      delete (Promise as any).try;
    });
    const errors = trackErrors(page);
    await openBoard(page);
    await importPdf(page);
    expect(errors).toEqual([]);
  });

  test("saves a lesson and opens it again", async ({ page, browser }, info) => {
    const errors = trackErrors(page);
    await openBoard(page);
    await page.keyboard.press("p");
    await scribble(page);
    await expect(page.getByRole("button", { name: "Undo" }).first()).toBeEnabled();

    // No File System Access API in WebKit: saving downloads a copy.
    const download = page.waitForEvent("download");
    await page.keyboard.press("ControlOrMeta+s");
    const file = await download;
    expect(file.suggestedFilename()).toMatch(/\.rpk$/);
    const path = info.outputPath(file.suggestedFilename());
    await file.saveAs(path);
    const bytes = await readFile(path);
    expect(bytes.subarray(0, 2).toString("latin1"), "a .rpk is a zip package").toBe("PK");
    expect(errors).toEqual([]);

    // A fresh browser profile (no autosave), then ⌘O / Ctrl+O.
    const context = await browser.newContext();
    const fresh = await context.newPage();
    const freshErrors = trackErrors(fresh);
    await openBoard(fresh);
    const blank = await inkShot(fresh);
    const chooser = fresh.waitForEvent("filechooser");
    await fresh.keyboard.press("ControlOrMeta+o");
    await (await chooser).setFiles(path);
    await expect(fresh.getByRole("status")).toContainText("Opened");
    await expect.poll(async () => (await inkShot(fresh)).equals(blank), { message: "the stroke came back" }).toBe(false);
    expect(freshErrors).toEqual([]);
    await context.close();
  });
});

// The desktop app's "Support Rupok" card (donate.ts); `?donate-card` turns
// it on in a browser, as if this were the desktop app.
test.describe("support card", () => {
  const card = (page: Page) => page.getByTestId("donate-card");
  const openWithCard = async (page: Page) => {
    await page.goto("/board/?donate-card");
    await expect(page.locator(".loading")).toHaveCount(0, { timeout: 30_000 });
  };

  test("shows on first launch, Not now hides it, and it doesn't come back at once", async ({ page }) => {
    const errors = trackErrors(page);
    await openWithCard(page);
    await expect(card(page)).toBeVisible();
    await expect(card(page)).toContainText("Rupok is free and open source");
    await card(page).getByRole("button", { name: "Not now" }).last().click();
    await expect(card(page)).toHaveCount(0);
    await openWithCard(page);
    await page.waitForTimeout(2500);
    await expect(card(page)).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("Don't show again is remembered", async ({ page }) => {
    await openWithCard(page);
    await card(page).getByRole("button", { name: "Don't show again" }).click();
    await expect(card(page)).toHaveCount(0);
    const saved = await page.evaluate(() => JSON.parse(localStorage.getItem("rupok.donate") ?? "null"));
    expect(saved?.never).toBe(true);
  });

  test("never appears in Teach", async ({ page }) => {
    await openWithCard(page);
    await page.keyboard.press("F5");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "teach");
    await page.waitForTimeout(2500);
    await expect(card(page)).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "prepare");
    await expect(card(page)).toBeVisible();
  });

  test("is off in the browser without the flag", async ({ page }) => {
    await openBoard(page);
    await page.waitForTimeout(2500);
    await expect(card(page)).toHaveCount(0);
  });
});

test.describe("website in WebKit (Safari)", () => {
  for (const path of ["/", "/open/", "/bn/", "/compare/", "/donate/", "/bn/donate/"]) {
    test(`${path} loads without errors`, async ({ page }) => {
      const errors = trackErrors(page);
      const res = await page.goto(path);
      expect(res?.ok()).toBe(true);
      await expect(page.locator("body")).toBeVisible();
      await page.waitForLoadState("networkidle");
      expect(errors).toEqual([]);
    });
  }
});
