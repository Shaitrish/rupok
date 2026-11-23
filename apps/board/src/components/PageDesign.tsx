// Page design: the look of the pages (built-in looks, colour, pattern,
// background image) and the lesson's branding (logo, school, teacher,
// class, date). A sheet at the side, so the page stays in view and shows
// every change at once; on phones a bottom sheet.
//
// Every change is one engine call, so one undo step (the sheet's Undo, or
// Ctrl+Z). Sliders and the logo drag preview live (design.ts →
// designPreview) and are written on release.

import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { BOARD_COLORS, canUndo, pageIndex, pages, showToast } from "../state";
import {
  type BgFit, type Branding, type Corner, type PageBackground, type PageStyle, type Preset, CORNERS, DEFAULT_BRANDING, MAX_BRAND_TEXT, MAX_LOGO_W,
  MIN_LOGO_W, PRESETS, brandingFor, clamp, cornerOf, cornerPos, cssColor, designDefault, designPreview, examPaperSvg, frameOf, imageSize,
  lessonBranding, logoRect, prepareImage, presetStyle, resizeLogo, setDesignDefault, setDesignPreview, styleOf,
} from "../design";
import { designDefaultStore } from "../storage";
import { cssPx, isPhone } from "../viewport";
import { Icon } from "./Icon";
import { PageBackdrop } from "./PageBackdrop";
import "./PageDesign.css";

const [open, setOpen] = createSignal(false);
export const openPageDesign = () => setOpen(true);
export const pageDesignOpen = open;

/** Mounted once in App. */
export function PageDesignHost() {
  return (
    <Show when={open()}>
      <PageDesign onClose={() => setOpen(false)} />
    </Show>
  );
}

const FITS: { id: BgFit; label: string; hint: string }[] = [
  { id: "fit", label: "Fit", hint: "The whole image, as large as fits" },
  { id: "fill", label: "Fill", hint: "Covers the page, cropped" },
  { id: "stretch", label: "Stretch", hint: "Exactly the page (may distort)" },
  { id: "tile", label: "Tile", hint: "Repeated, like wallpaper" },
  { id: "center", label: "Centre", hint: "Actual size, in the middle" },
];
const PATTERNS = ["plain", "grid", "dots", "lines"] as const;
const ACCEPT = "image/png,image/jpeg,image/svg+xml,image/webp,.svg";

function pickImage(): Promise<File | undefined> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ACCEPT;
    input.onchange = () => resolve(input.files?.[0]);
    input.click();
  });
}

function PageDesign(props: { onClose: () => void }) {
  const page = () => pages()[pageIndex()];
  const frame = () => frameOf(page());
  const style = () => styleOf(page());
  const count = () => pages().length;
  const [bgScope, setBgScope] = createSignal<"page" | "all">("page");
  const ownBranding = () => (page()?.branding && !page()!.branding!.off ? page()!.branding! : null);
  const hiddenHere = () => !!page()?.branding?.off;
  const [brandScope, setBrandScope] = createSignal<"lesson" | "page">(ownBranding() ? "page" : "lesson");
  const [busy, setBusy] = createSignal(false);

  // ------------------------------------------------------------ writing

  const board = () => controller.board;
  const commitStyle = (s: PageStyle, all = bgScope() === "all") => {
    setDesignPreview(null);
    try {
      board().set_page_style_json(JSON.stringify(s), all);
      controller.penForBoard(s.bg_color);
      if (all && count() > 1) showToast(`Applied to all ${count()} pages. Undo with Ctrl+Z.`);
    } catch (e: any) {
      showToast(`Could not change the page: ${e?.message ?? e}`);
    }
  };
  const setBg = (patch: Partial<PageBackground> | null) => {
    const cur = style().background;
    commitStyle({ ...style(), background: patch === null || !cur ? null : { ...cur, ...patch } });
  };
  /** The branding being edited: the lesson's, or this page's own. */
  const editing = (): Branding => {
    if (brandScope() === "page") return ownBranding() ?? lessonBranding() ?? DEFAULT_BRANDING;
    return lessonBranding() ?? DEFAULT_BRANDING;
  };
  const commitBrand = (b: Branding | null) => {
    setDesignPreview(null);
    try {
      board().set_branding_json(JSON.stringify(b ? { ...b, off: false } : null), brandScope() === "page");
    } catch (e: any) {
      showToast(`Could not change the branding: ${e?.message ?? e}`);
    }
  };
  const setBrand = (patch: Partial<Branding>) => commitBrand({ ...editing(), ...patch });
  /** Live preview of a branding change on this page (committed on release). */
  const previewBrand = (patch: Partial<Branding>) => {
    if (brandScope() === "lesson" && (ownBranding() || hiddenHere())) return; // this page shows something else
    setDesignPreview({ branding: { ...editing(), ...patch } });
  };
  const previewBg = (patch: Partial<PageBackground>) => {
    const cur = style().background;
    if (cur) setDesignPreview({ background: { ...cur, ...patch } });
  };

  // ------------------------------------------------------------ images

  const [logoAspect, setLogoAspect] = createSignal(1);
  const logoUrl = createMemo(() => (editing().logo ? controller.assetUrl(editing().logo!) : ""));
  createMemo(() => {
    const u = logoUrl();
    if (u) void imageSize(u).then((s) => s.w && s.h && setLogoAspect(s.w / s.h));
  });

  const upload = async (kind: "logo" | "background") => {
    const f = await pickImage();
    if (!f) return;
    setBusy(true);
    try {
      const img = await prepareImage(f, kind);
      const name = board().add_asset(img.bytes, img.ext);
      if (!name) throw new Error("the lesson is read-only");
      if (kind === "background") {
        const cur = style().background;
        commitStyle({ ...style(), background: { image: name, fit: cur?.fit ?? "fill", opacity: cur?.opacity ?? 1, dim: cur?.dim ?? false } });
      } else {
        const b = editing();
        const aspect = img.w / Math.max(1, img.h);
        setLogoAspect(aspect);
        // A new logo starts in its corner (top left unless it was moved).
        const corner = b.logo ? cornerOf(b, frame().w, frame().h, logoAspect()) : "top-left";
        const pos = corner ? cornerPos(corner, b.logo_w, frame().w, frame().h, aspect) : {};
        commitBrand({ ...b, ...pos, logo: name });
      }
    } catch (e: any) {
      showToast(`Could not use that image: ${e?.message ?? e}`, 4000);
    } finally {
      setBusy(false);
    }
  };

  // ------------------------------------------------------------ preview

  let previewBox!: HTMLDivElement;
  const [previewW, setPreviewW] = createSignal(320);
  const k = () => previewW() / frame().w;
  let sheet!: HTMLElement;
  onMount(() => {
    // Prepare: the slide moves left so it stays fully in view beside the sheet.
    controller.sheetInset = sheet.offsetWidth + 2 * cssPx("--inset", 12);
    controller.updateInsets(true);
    onCleanup(() => {
      controller.sheetInset = 0;
      controller.updateInsets(true);
    });
    const ro = new ResizeObserver(() => setPreviewW(previewBox.clientWidth || 320));
    ro.observe(previewBox);
    // Escape closes the sheet (before the board sees it).
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      props.onClose();
    };
    window.addEventListener("keydown", key, true);
    onCleanup(() => {
      ro.disconnect();
      window.removeEventListener("keydown", key, true);
      setDesignPreview(null);
    });
  });
  const shownBg = () => { const o = designPreview(); return o && "background" in o ? o.background : style().background; };
  const shownBrand = () => { const o = designPreview(); return o && "branding" in o ? o.branding : brandingFor(page(), lessonBranding()); };

  /** Drag the logo in the preview: live on the board, one undo step on release. */
  const dragLogo = (e: PointerEvent) => {
    const b0 = shownBrand();
    if (!b0?.logo || (brandScope() === "lesson" && (ownBranding() || hiddenHere()))) return;
    e.preventDefault();
    (document.activeElement as HTMLElement | null)?.blur?.();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const [x0, y0] = [e.clientX, e.clientY];
    const [, , lw, lh] = logoRect(b0, frame().w, frame().h, logoAspect());
    let next = b0;
    const move = (ev: PointerEvent) => {
      const dx = (ev.clientX - x0) / k() / frame().w, dy = (ev.clientY - y0) / k() / frame().h;
      next = {
        ...b0,
        logo_x: clamp(b0.logo_x + dx, 0, Math.max(0, 1 - lw / frame().w)),
        logo_y: clamp(b0.logo_y + dy, 0, Math.max(0, 1 - lh / frame().h)),
      };
      setDesignPreview({ branding: next });
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      if (next !== b0) setBrand({ logo_x: next.logo_x, logo_y: next.logo_y });
      else setDesignPreview(null);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };

  // ------------------------------------------------------------ default

  const saveDefault = async () => {
    const s = styleOf(page());
    const b = brandingFor(page(), lessonBranding());
    const assets: Record<string, Uint8Array> = {};
    for (const n of [s.background?.image, b?.logo]) {
      const bytes = n ? board().asset(n) : undefined;
      if (n && bytes) assets[n] = bytes;
    }
    const d = { version: 1 as const, style: s, branding: b ? { ...b, off: false } : null, assets };
    if (await designDefaultStore(d)) {
      setDesignDefault(d);
      showToast("Saved on this device: new lessons start with this look and branding.", 3600);
    } else {
      showToast("Could not save the default (storage is blocked in this browser).", 4000);
    }
  };
  const clearDefault = async () => {
    await designDefaultStore(null);
    setDesignDefault(null);
    showToast("New lessons start plain again.");
  };

  const corner = () => cornerOf(editing(), frame().w, frame().h, logoAspect());
  const lessonFollowed = () => brandScope() === "lesson" && !!ownBranding();

  return (
    <aside ref={sheet} class="design-sheet" classList={{ phone: isPhone() }} role="dialog" aria-label="Page design">
      <header class="ds-head">
        <div class="badge"><Icon name="palette" size={20} active /></div>
        <div class="ds-title">
          <h2>Page design</h2>
          <p>Page {pageIndex() + 1} of {count()}</p>
        </div>
        <button class="btn icon" title="Undo  ·  Ctrl+Z" aria-label="Undo" disabled={!canUndo()} onClick={() => controller.board.undo()}>
          <Icon name="undo" size={20} />
        </button>
        <button class="btn icon" title="Close  ·  Esc" aria-label="Close page design" onClick={props.onClose}>
          <Icon name="close" size={20} />
        </button>
      </header>

      <div class="ds-body">
        <div class="ds-preview" ref={previewBox} style={{ height: `${frame().h * k()}px` }}>
          <div
            class={`ds-page ${shownBg() ? "" : style().pattern}`}
            style={{
              width: `${frame().w}px`,
              height: `${frame().h}px`,
              transform: `scale(${k()})`,
              "background-color": cssColor(style().bg_color),
              "--pat": BOARD_COLORS.find((b) => b.rgba === style().bg_color)?.pattern ?? "rgba(30, 60, 120, 0.10)",
            }}
          >
            <PageBackdrop
              w={frame().w}
              h={frame().h}
              bg={style().bg_color}
              pattern={style().pattern}
              background={shownBg()}
              branding={shownBrand()}
              assetUrl={(n) => controller.assetUrl(n)}
              onLogoPointerDown={dragLogo}
            />
          </div>
          <Show when={shownBrand()?.logo}>
            <span class="ds-hint">Drag the logo to move it</span>
          </Show>
        </div>

        {/* ------------------------------------------------ looks */}
        <section class="ds-sec" aria-labelledby="ds-looks">
          <div class="ds-sec-head">
            <h3 id="ds-looks">Looks</h3>
            <Scope value={bgScope()} onChange={setBgScope} count={count()} />
          </div>
          <div class="ds-presets">
            <For each={PRESETS}>
              {(p) => (
                <button class="ds-preset" title={p.hint} onClick={() => commitStyle(presetStyle(p, board()))}>
                  <PresetSwatch preset={p} />
                  <span>{p.label}</span>
                </button>
              )}
            </For>
          </div>
        </section>

        {/* ------------------------------------------------ background */}
        <section class="ds-sec" aria-labelledby="ds-bg">
          <div class="ds-sec-head">
            <h3 id="ds-bg">Background</h3>
            <span class="ds-note">{bgScope() === "all" ? `All ${count()} pages` : "This page"}</span>
          </div>
          <div class="ds-row">
            <span class="ds-label">Colour</span>
            <div class="ds-chips" role="group" aria-label="Page colour">
              <For each={BOARD_COLORS}>
                {(c) => (
                  <button
                    class="ds-chip"
                    classList={{ on: style().bg_color === c.rgba }}
                    title={c.label}
                    aria-label={`${c.label} page`}
                    aria-pressed={style().bg_color === c.rgba}
                    onClick={() => commitStyle({ ...style(), bg_color: c.rgba })}
                  >
                    <span class="bg-dot" style={{ background: cssColor(c.rgba) }} />
                  </button>
                )}
              </For>
            </div>
          </div>
          <div class="ds-row">
            <span class="ds-label">Pattern</span>
            <div class="ds-chips" role="group" aria-label="Pattern">
              <For each={PATTERNS}>
                {(pat) => (
                  <button
                    class="ds-chip"
                    classList={{ on: style().pattern === pat }}
                    title={pat[0].toUpperCase() + pat.slice(1)}
                    aria-label={`${pat[0].toUpperCase() + pat.slice(1)} pattern`}
                    aria-pressed={style().pattern === pat}
                    onClick={() => commitStyle({ ...style(), pattern: pat })}
                  >
                    <span class={`pat-ico ${pat}`} />
                  </button>
                )}
              </For>
            </div>
          </div>
          <div class="ds-row">
            <span class="ds-label">Image</span>
            <div class="ds-actions">
              <button class="btn soft" disabled={busy()} onClick={() => upload("background")}>
                <Icon name="image" size={20} /><span>{style().background ? "Replace" : "Upload image"}</span>
              </button>
              <Show when={style().background}>
                <button class="btn" aria-label="Remove background image" onClick={() => setBg(null)}>
                  <Icon name="trash" size={20} /><span>Remove</span>
                </button>
              </Show>
            </div>
          </div>
          <Show when={style().background}>
            {(bg) => (
              <>
                <div class="seg ds-seg" role="radiogroup" aria-label="Image fit">
                  <For each={FITS}>
                    {(f) => (
                      <button role="radio" aria-checked={bg().fit === f.id} classList={{ on: bg().fit === f.id }} title={f.hint} onClick={() => setBg({ fit: f.id })}>
                        {f.label}
                      </button>
                    )}
                  </For>
                </div>
                <Slider label="Image opacity" value={bg().opacity} min={0.1} max={1} onInput={(v) => previewBg({ opacity: v })} onChange={(v) => setBg({ opacity: v })} />
                <Toggle
                  label="Dim for readability"
                  hint="A wash of the page colour over the image, so ink stands out"
                  on={bg().dim}
                  onChange={(v) => setBg({ dim: v })}
                />
              </>
            )}
          </Show>
          <Show when={bgScope() === "page" && count() > 1}>
            <button class="btn ds-wide" onClick={() => commitStyle(style(), true)}>
              <Icon name="layers" size={20} /><span>Use this look for all {count()} pages</span>
            </button>
          </Show>
        </section>

        {/* ------------------------------------------------ branding */}
        <section class="ds-sec" aria-labelledby="ds-brand">
          <div class="ds-sec-head">
            <h3 id="ds-brand">Branding</h3>
            <div class="seg ds-scope" role="radiogroup" aria-label="Where the branding shows">
              <button role="radio" aria-checked={brandScope() === "lesson"} classList={{ on: brandScope() === "lesson" }} onClick={() => setBrandScope("lesson")}>All pages</button>
              <button role="radio" aria-checked={brandScope() === "page"} classList={{ on: brandScope() === "page" }} onClick={() => setBrandScope("page")}>This page</button>
            </div>
          </div>
          <Show when={lessonFollowed()}>
            <div class="ds-callout">
              <span>This page has its own branding.</span>
              <button class="btn" onClick={() => board().set_branding_json("null", true)}>Use the lesson's here</button>
            </div>
          </Show>
          <Toggle
            label="Hide branding on this page"
            hint="For a title slide or a full-page picture"
            on={hiddenHere()}
            onChange={(v) => board().set_branding_json(v ? JSON.stringify({ off: true }) : "null", true)}
          />
          <Show when={!hiddenHere() || brandScope() === "lesson"}>
            <div class="ds-row">
              <span class="ds-label">Logo</span>
              <div class="ds-actions">
                <button class="btn soft" disabled={busy()} onClick={() => upload("logo")}>
                  <Icon name="image" size={20} /><span>{editing().logo ? "Replace" : "Upload logo"}</span>
                </button>
                <Show when={editing().logo}>
                  <button class="btn" aria-label="Remove logo" onClick={() => setBrand({ logo: null })}>
                    <Icon name="trash" size={20} /><span>Remove</span>
                  </button>
                </Show>
              </div>
            </div>
            <Show when={editing().logo}>
              <div class="ds-row">
                <span class="ds-label">Place</span>
                <div class="ds-chips" role="radiogroup" aria-label="Logo corner">
                  <For each={CORNERS}>
                    {(c) => (
                      <button
                        class="ds-chip"
                        role="radio"
                        aria-checked={corner() === c.id}
                        classList={{ on: corner() === c.id }}
                        title={c.label}
                        aria-label={c.label}
                        onClick={() => setBrand(cornerPos(c.id as Corner, editing().logo_w, frame().w, frame().h, logoAspect()))}
                      >
                        <span class={`corner-ico ${c.id}`} />
                      </button>
                    )}
                  </For>
                </div>
              </div>
              <Slider
                label="Logo size"
                value={editing().logo_w}
                min={MIN_LOGO_W}
                max={0.4}
                onInput={(v) => previewBrand(resizeLogo(editing(), v, frame().w, frame().h, logoAspect()))}
                onChange={(v) => commitBrand(resizeLogo(editing(), Math.min(v, MAX_LOGO_W), frame().w, frame().h, logoAspect()))}
              />
              <Slider label="Logo opacity" value={editing().logo_opacity} min={0.1} max={1} onInput={(v) => previewBrand({ logo_opacity: v })} onChange={(v) => setBrand({ logo_opacity: v })} />
            </Show>
            <TextField label="School" value={editing().school} placeholder="e.g. Dhaka Residential Model College" onChange={(v) => setBrand({ school: v })} />
            <TextField label="Teacher" value={editing().teacher} placeholder="e.g. Ms. Rahman" onChange={(v) => setBrand({ teacher: v })} />
            <TextField label="Class / subject" value={editing().class} placeholder="e.g. Class 8 · Science" onChange={(v) => setBrand({ class: v })} />
            <Toggle label="Today's date" hint="The day the lesson is shown or exported" on={editing().date} onChange={(v) => setBrand({ date: v })} />
            <div class="ds-row">
              <span class="ds-label">Text</span>
              <div class="ds-split">
                <div class="seg" role="radiogroup" aria-label="Text position">
                  <button role="radio" aria-checked={editing().text_pos === "top"} classList={{ on: editing().text_pos === "top" }} onClick={() => setBrand({ text_pos: "top" })}>Header</button>
                  <button role="radio" aria-checked={editing().text_pos === "bottom"} classList={{ on: editing().text_pos === "bottom" }} onClick={() => setBrand({ text_pos: "bottom" })}>Footer</button>
                </div>
                <div class="seg" role="radiogroup" aria-label="Text alignment">
                  <For each={["left", "center", "right"] as const}>
                    {(a) => (
                      <button role="radio" aria-checked={editing().text_align === a} classList={{ on: editing().text_align === a }} title={a[0].toUpperCase() + a.slice(1)} aria-label={`Align ${a}`} onClick={() => setBrand({ text_align: a })}>
                        <span class={`align-ico ${a}`} />
                      </button>
                    )}
                  </For>
                </div>
              </div>
            </div>
            <Show when={(brandScope() === "lesson" ? lessonBranding() : ownBranding())}>
              <button class="btn ds-wide" onClick={() => commitBrand(null)}>
                <Icon name="trash" size={20} /><span>{brandScope() === "lesson" ? "Remove branding from the lesson" : "Remove this page's branding"}</span>
              </button>
            </Show>
          </Show>
        </section>

        {/* ------------------------------------------------ default */}
        <section class="ds-sec" aria-labelledby="ds-default">
          <div class="ds-sec-head"><h3 id="ds-default">Every day</h3></div>
          <p class="ds-text">Save this page's look and the branding as your starting point: new lessons on this device begin with them.</p>
          <div class="ds-actions">
            <button class="btn solid" onClick={saveDefault}><Icon name="save" size={20} /><span>Save as my default for new lessons</span></button>
            <Show when={designDefault()}>
              <button class="btn" onClick={clearDefault}>Clear default</button>
            </Show>
          </div>
        </section>
      </div>
    </aside>
  );
}

function Scope(props: { value: "page" | "all"; onChange: (v: "page" | "all") => void; count: number }) {
  return (
    <div class="seg ds-scope" role="radiogroup" aria-label="Apply looks and background to">
      <button role="radio" aria-checked={props.value === "page"} classList={{ on: props.value === "page" }} onClick={() => props.onChange("page")}>This page</button>
      <button role="radio" aria-checked={props.value === "all"} classList={{ on: props.value === "all" }} onClick={() => props.onChange("all")}>All pages</button>
    </div>
  );
}

/** A small picture of a built-in look. */
function PresetSwatch(props: { preset: Preset }) {
  const p = props.preset;
  const img = p.image ? `url("data:image/svg+xml;charset=utf-8,${encodeURIComponent(examPaperSvg())}")` : undefined;
  return (
    <span
      class={`ds-swatch pd-pattern-mini ${p.pattern}`}
      style={{
        "background-color": cssColor(p.bg),
        "--pat": BOARD_COLORS.find((b) => b.rgba === p.bg)?.pattern ?? "rgba(30, 60, 120, 0.10)",
        "background-image": img,
      }}
    />
  );
}

function Slider(props: { label: string; value: number; min: number; max: number; onInput: (v: number) => void; onChange: (v: number) => void }) {
  const id = `ds-${props.label.replace(/\W+/g, "-").toLowerCase()}`;
  const pct = () => ((props.value - props.min) / (props.max - props.min)) * 100;
  return (
    <div class="ds-row">
      <label class="ds-label" for={id}>{props.label}</label>
      <input
        id={id}
        class="range"
        type="range"
        min={props.min}
        max={props.max}
        step="0.01"
        value={props.value}
        style={{ "--p": `${pct()}%` }}
        onInput={(e) => { e.currentTarget.style.setProperty("--p", `${((Number(e.currentTarget.value) - props.min) / (props.max - props.min)) * 100}%`); props.onInput(Number(e.currentTarget.value)); }}
        onChange={(e) => props.onChange(Number(e.currentTarget.value))}
      />
      <span class="ds-val">{Math.round(props.value * 100)}%</span>
    </div>
  );
}

function Toggle(props: { label: string; hint?: string; on: boolean; onChange: (v: boolean) => void }) {
  return (
    <button class="ds-toggle" role="switch" aria-checked={props.on} onClick={() => props.onChange(!props.on)}>
      <span class="ds-toggle-text">
        <span>{props.label}</span>
        <Show when={props.hint}><small>{props.hint}</small></Show>
      </span>
      <span class="switch" classList={{ on: props.on }} />
    </button>
  );
}

/** A text field written on change (Enter or leaving it): one undo step per edit. */
function TextField(props: { label: string; value: string; placeholder: string; onChange: (v: string) => void }) {
  const id = `ds-${props.label.replace(/\W+/g, "-").toLowerCase()}`;
  return (
    <div class="ds-field">
      <label for={id}>{props.label}</label>
      <input
        id={id}
        class="input"
        value={props.value}
        maxLength={MAX_BRAND_TEXT}
        placeholder={props.placeholder}
        onChange={(e) => {
          const v = e.currentTarget.value.trim();
          if (v !== props.value) props.onChange(v);
        }}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
    </div>
  );
}
