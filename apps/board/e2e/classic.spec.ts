// The Classic layout (src/layout.ts, components/classic/): three islands
// along the bottom, the ☰ menu, the pen panel with illustrated pen types,
// the page grid, the edge swipe's slide panel, the upright selection
// toolbar, switching back to Modern, and the agent's apply_dock_preset.

import { expect, test, type Page } from "@playwright/test";

function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("crash", () => errors.push("the page crashed"));
  return errors;
}

async function openClassic(page: Page) {
  await page.addInitScript(() => {
    try { localStorage.setItem("rupok.tour", "1"); } catch { /* storage blocked */ }
  });
  await page.goto("/board/");
  await expect(page.locator(".loading")).toHaveCount(0, { timeout: 30_000 });
  await expect(page.locator("canvas.ink")).toBeVisible();
  // Classic is the default on a new board; set it only if it is not on yet.
  await page.evaluate(() => (window as any).rupokLayout.get() !== "classic" && (window as any).rupokLayout.set("classic"));
  await expect(page.locator(".island")).toHaveCount(3);
}

const island = (page: Page, id: string) => page.locator(`.island[data-island="${id}"]`);

async function scribble(page: Page) {
  const b = (await page.locator(".page-frame").first().boundingBox())!;
  const [x0, y0] = [b.x + b.width * 0.3, b.y + b.height * 0.45];
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  for (let i = 1; i <= 20; i++) await page.mouse.move(x0 + i * 12, y0 + (i % 2 ? -30 : 30), { steps: 2 });
  await page.mouse.up();
}

test.describe("Classic layout", () => {
  test("three islands with big buttons, no top bar", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await expect(page.locator(".topbar")).toHaveCount(0);
    for (const name of ["Main menu", "Mode", "Tools", "Zoom", "Ask AI"]) await expect(island(page, "left").getByRole("button", { name })).toBeVisible();
    // The AI is on the island, not floating over the slide.
    await expect(page.locator(".ai-float")).toHaveCount(0);
    for (const name of ["Select & move", "Pen", "Eraser", "Shapes", "Hand", "Recognise ink", "Stamps", "Undo", "Redo"]) {
      await expect(island(page, "centre").getByRole("button", { name, exact: true })).toBeVisible();
    }
    for (const name of ["New page", "Previous page", "Next page"]) await expect(island(page, "right").getByRole("button", { name })).toBeVisible();
    // 56 px targets (44 px or more everywhere).
    const pen = await island(page, "centre").getByRole("button", { name: "Pen", exact: true }).boundingBox();
    expect(pen!.width).toBeGreaterThanOrEqual(52);
    expect(errors).toEqual([]);
  });

  test("the arrow tab swaps the side islands, and the agent can too", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    const leftOf = async (id: string) => (await island(page, id).boundingBox())!.x;
    expect(await leftOf("left")).toBeLessThan(await leftOf("right"));
    await island(page, "left").getByRole("button", { name: "Swap the side islands" }).click();
    await expect.poll(async () => (await leftOf("left")) > (await leftOf("right"))).toBe(true);
    // The menu island now has its tab on the far right.
    await expect(island(page, "left").locator(".isl-swap.right")).toHaveCount(1);
    const r = await page.evaluate(() => (window as any).rupokBoardOp({ op: "swap_islands", swapped: false }));
    expect(r.menu_island).toBe("left");
    await expect.poll(async () => (await leftOf("left")) < (await leftOf("right"))).toBe(true);
    expect(errors).toEqual([]);
  });

  test("a new board starts in Classic, without asking under automation", async ({ page }) => {
    const errors = trackErrors(page);
    await page.addInitScript(() => {
      try { localStorage.setItem("rupok.tour", "1"); } catch { /* storage blocked */ }
    });
    await page.goto("/board/");
    await expect(page.locator(".loading")).toHaveCount(0, { timeout: 30_000 });
    await expect(page.locator(".island")).toHaveCount(3);
    await expect(page.locator(".topbar")).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("the picker recommends Classic and Esc keeps it", async ({ page }) => {
    await page.addInitScript(() => {
      try { localStorage.setItem("rupok.tour", "1"); } catch { /* storage blocked */ }
    });
    await page.goto("/board/?layout");
    const dialog = page.locator(".lc-dialog");
    await expect(dialog).toBeVisible({ timeout: 30_000 });
    await expect(dialog.locator(".lc-option").first()).toHaveAttribute("data-layout", "classic");
    await expect(dialog.locator('[data-layout="classic"]')).toHaveAttribute("aria-checked", "true");
    await expect(dialog.locator(".lc-badge")).toHaveText("Recommended");
    await expect(dialog.getByRole("button", { name: "Use Classic" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".island")).toHaveCount(3);
  });

  test("hold an island's background to move it; the agent moves and folds them", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    const right = island(page, "right");
    const b = (await right.boundingBox())!;
    // Press the island's own padding (not a button), hold, drag up, let go.
    await page.mouse.move(b.x + b.width / 2, b.y + 2);
    await page.mouse.down();
    await page.waitForTimeout(600);
    await expect(page.locator(".isl-target")).toHaveCount(6);
    await page.mouse.move(b.x + b.width / 2, 60, { steps: 8 });
    await page.mouse.up();
    await expect(page.locator(".islands-row.top .island[data-island=\"right\"]")).toHaveCount(1);
    // All to the bottom, then fold one.
    let r = await page.evaluate(() => (window as any).rupokBoardOp({ op: "move_island", island: "all", edge: "bottom" }));
    expect(r.islands.every((i: any) => i.place.startsWith("bottom"))).toBe(true);
    r = await page.evaluate(() => (window as any).rupokBoardOp({ op: "move_island", island: "centre", collapsed: true }));
    await expect(page.locator('.isl-pill[data-island="centre"]')).toBeVisible();
    await page.locator('.isl-pill[data-island="centre"]').click();
    await expect(island(page, "centre")).toBeVisible();
    r = await page.evaluate(() => (window as any).rupokBoardOp({ op: "move_island", reset: true }));
    expect(r.reset).toBe(true);
    expect(errors).toEqual([]);
  });

  test("the ☰ menu has big rows and a File submenu", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await page.getByRole("button", { name: "Main menu" }).click();
    const menu = page.getByRole("menu", { name: "Main menu" });
    await expect(menu).toBeVisible();
    for (const row of ["File", "Import", "Export", "Print", "Settings", "Help", "Mode"]) await expect(menu.locator(`[data-row]`, { hasText: row }).first()).toBeVisible();
    for (const tile of ["new", "open", "save", "save-as"]) await expect(menu.locator(`[data-tile="${tile}"]`)).toBeVisible();
    await menu.locator('[data-row="import"]').click();
    await expect(menu.locator('[data-tile="pdf"]')).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("the pen panel shows illustrated pen types and 15 colours", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await island(page, "centre").getByRole("button", { name: "Pen", exact: true }).click();
    const panel = page.locator(".cl-ink");
    await expect(panel).toBeVisible();
    await expect(panel.locator(".cl-brush")).toHaveCount(13); // 12 pen styles + the highlighter
    await expect(panel.locator(".cl-swatch:not(.cl-drop)")).toHaveCount(15);
    await panel.locator('[data-brush="chalk"]').click();
    await expect(panel.locator('[data-brush="chalk"]')).toHaveAttribute("aria-checked", "true");
    await expect(panel.locator(".cl-title")).toContainText("Chalk");
    expect(errors).toEqual([]);
  });

  test("the page button opens a thumbnail grid", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await island(page, "right").getByRole("button", { name: "New page" }).click();
    await island(page, "right").getByRole("button", { name: "New page" }).click();
    await island(page, "right").locator(".viewer-count").click();
    const grid = page.getByRole("listbox", { name: "Pages" });
    await expect(grid.getByRole("option")).toHaveCount(3);
    await grid.getByRole("option", { name: "Page 1" }).click();
    await expect(island(page, "right").locator(".viewer-count")).toHaveText("1 / 3");
    expect(errors).toEqual([]);
  });

  test("a swipe in from the right edge opens the slide panel", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await page.keyboard.press("p");
    await page.evaluate(() => {
      const el = document.querySelector(".input-layer")!;
      const W = innerWidth;
      const at = (type: string, x: number) =>
        el.dispatchEvent(new PointerEvent(type, { pointerId: 41, pointerType: "touch", clientX: x, clientY: 420, bubbles: true, isPrimary: true }));
      at("pointerdown", W - 6);
      for (let x = W - 18; x > W - 120; x -= 12) at("pointermove", x);
      at("pointerup", W - 120);
    });
    const panel = page.locator(".pages.drawer.edge-right");
    await expect(panel).toBeVisible();
    await expect(panel.getByRole("button", { name: "Organise pages" })).toBeVisible();
    await expect(panel.getByRole("button", { name: "Star page" })).toHaveCount(1);
    // The swipe drew nothing: undo has nothing to take back.
    await expect(island(page, "centre").getByRole("button", { name: "Undo" })).toBeDisabled();
    // Synthetic touches have no real pointer to capture (Chromium: "setPointerCapture…",
    // WebKit: "The object can not be found here.").
    expect(errors.filter((e) => !/setPointerCapture|can not be found/.test(e))).toEqual([]);
  });

  test("a selection gets the upright toolbar beside it", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await page.keyboard.press("p");
    await scribble(page);
    await page.keyboard.press("ControlOrMeta+a");
    await expect(page.locator(".sel-layer.classic .sel-frame")).toBeVisible();
    const bar = page.locator(".sel-bar.vertical");
    await expect(bar).toBeVisible();
    const [f, b] = [await page.locator(".sel-frame").boundingBox(), await bar.boundingBox()];
    expect(b!.height).toBeGreaterThan(b!.width); // upright
    expect(b!.x >= f!.x + f!.width || b!.x + b!.width <= f!.x).toBe(true); // beside, not over
    await bar.getByRole("button", { name: "Delete" }).click();
    await expect(page.locator(".sel-frame")).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("Settings switches back to Modern, and the agent switches to Classic", async ({ page }) => {
    const errors = trackErrors(page);
    await openClassic(page);
    await page.evaluate(() => (window as any).rupokSettings.open("appearance.layout"));
    await page.getByRole("radio", { name: "Modern" }).click();
    await page.keyboard.press("Escape");
    await expect(page.locator(".island")).toHaveCount(0);
    await expect(page.locator(".topbar")).toBeVisible();
    // The agent's apply_dock_preset (the same op the MCP tool sends).
    const r = await page.evaluate(() => (window as any).rupokBoardOp({ op: "apply_dock_preset", name: "Classic" }));
    expect(r.applied).toBe("Classic");
    await expect(page.locator(".island")).toHaveCount(3);
    // And set_dock_layout rearranges the islands.
    await page.evaluate(() => (window as any).rupokBoardOp({ op: "set_dock_layout", mode: "prepare", layout: { style: "islands", groups: [{ items: ["menu"] }, { items: ["pen", "eraser", "undo"] }] } }));
    await expect(page.locator(".island")).toHaveCount(2);
    expect(errors).toEqual([]);
  });
});
