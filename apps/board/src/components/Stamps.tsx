// Stamps UI: the palette in the dock's Stamps popover, the long-press quick
// menu (a ring of favourite stamps around the pen), the stamp on the board,
// and the celebration burst. The data and drawing live in stamps.ts.

import { For, Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { controller } from "../controller";
import { freshStamps, onColor, stampSvg, stampSize, stampDef, STAMPS, EMOJI, celebrate, celebration, currentStamp, nextBadge, quickMenu, setCurrentStamp, setQuickMenu, toggleFavorite } from "../stamps";
import { type DomItem, editingText, isView, mode, prefs, replay, showToast, tool } from "../state";
import { longPress } from "./DockShell";
import "./Stamps.css";
import { Icon } from "./Icon";

const hexOf = (c: number) => `#${(c >>> 8).toString(16).padStart(6, "0")}`;
const reducedMotion = () => typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/** A stamp's picture at icon size (palette, quick menu). */
export function StampIcon(props: { id: string; emoji?: string; size?: number }) {
  const markup = () => {
    const def = stampDef(props.id);
    if (!def) return stampSvg("emoji", props.emoji ?? props.id, 0x1c2024ff, 100, 100);
    const text = props.id === "badge" ? String(nextBadge()) : def.text;
    const { w, h } = stampSize(def, text);
    return stampSvg(def.id, text, def.color, w, h);
  };
  const def = () => stampDef(props.id);
  return (
    <Show
      when={def()?.render === "label"}
      fallback={<span class="stamp-icon" style={{ "--s": `${props.size ?? 28}px` }} innerHTML={markup()} aria-hidden="true" />}
    >
      {/* A label reads better as a real pill than as a scaled-down picture. */}
      <span class="stamp-pill" style={{ background: hexOf(def()!.color), color: onColor(def()!.color) }} aria-hidden="true">
        {def()!.text}
      </span>
    </Show>
  );
}

/** Choose the stamp the Stamps tool places, and switch to it. */
export function useStamp(id: string, emoji?: string) {
  setCurrentStamp({ id, emoji });
  controller.setTool("stamp");
}

/** The Stamps popover: classroom stamps, emoji, Celebrate. */
export function StampsPanel(props: { onClose: () => void }) {
  const [emojiOpen, setEmojiOpen] = createSignal(currentStamp().id === "emoji");
  let field!: HTMLInputElement;
  const favs = () => prefs().stampFavorites;
  const pickEmoji = (e: string) => {
    useStamp("emoji", e);
  };
  return (
    <div class="stamps">
      <div class="dock-pop-title">Stamps</div>
      <div class="stamp-grid" role="radiogroup" aria-label="Stamps">
        <For each={[...STAMPS.filter((s) => s.render !== "label"), ...STAMPS.filter((s) => s.render === "label")]}>
          {(s) => {
            const on = () => tool() === "stamp" && currentStamp().id === s.id;
            const lp = longPress(() => {
              const fav = toggleFavorite(s.id);
              showToast(fav ? `${s.label} added to the quick menu (hold the pen still)` : `${s.label} removed from the quick menu`);
            });
            return (
              <button
                class="tool stamp-cell"
                classList={{ on: on(), fav: favs().includes(s.id), wide: s.render === "label" }}
                role="radio"
                aria-checked={on()}
                aria-label={s.label}
                title={`${s.label}  ·  hold to add to the quick menu`}
                data-stamp={s.id}
                onPointerDown={lp.onPointerDown}
                onClick={() => !lp.consumed() && useStamp(s.id)}
              >
                <StampIcon id={s.id} />
              </button>
            );
          }}
        </For>
        <button
          class="tool stamp-cell"
          classList={{ on: emojiOpen() }}
          aria-label="Emoji"
          aria-expanded={emojiOpen()}
          title="Any emoji"
          data-stamp="emoji"
          onClick={() => setEmojiOpen(!emojiOpen())}
        >
          <span class="stamp-icon emoji-glyph">😀</span>
        </button>
      </div>
      <Show when={emojiOpen()}>
        <div class="dock-hr" />
        <div class="emoji-grid" role="radiogroup" aria-label="Emoji">
          <For each={EMOJI}>
            {(e) => (
              <button
                class="tool emoji-cell"
                classList={{ on: tool() === "stamp" && currentStamp().id === "emoji" && currentStamp().emoji === e }}
                role="radio"
                aria-checked={currentStamp().emoji === e}
                aria-label={`Emoji ${e}`}
                onClick={() => pickEmoji(e)}
              >
                {e}
              </button>
            )}
          </For>
        </div>
        <input
          ref={field}
          class="input emoji-field"
          placeholder="Type or paste any emoji"
          aria-label="Any emoji"
          maxLength={16}
          onKeyDown={(e) => e.stopPropagation()}
          onInput={() => {
            const v = [...new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(field.value.trim())].map((s) => s.segment).pop();
            if (v) pickEmoji(v);
          }}
        />
      </Show>
      <div class="dock-hr" />
      <button
        class="dock-row"
        onClick={() => {
          props.onClose();
          celebrate();
        }}
      >
        <Icon name="celebrate" size={20} />
        <span class="grow">Celebrate</span>
        <span class="dock-state">Great answer!</span>
      </button>
      <p class="stamp-hint">Tap the board to stamp. Hold the pen still for the quick menu.</p>
    </div>
  );
}

/** A stamp on the board: drawn from its kind, colour and box, rotated. New
 * stamps pop in (not on reduced motion). */
export function StampItem(props: { item: DomItem }) {
  const it = () => props.item;
  const fresh = freshStamps.has(props.item.id) || !!replay();
  freshStamps.delete(props.item.id);
  const color = () => parseInt((it().color ?? "#1c2024ff").slice(1, 9).padEnd(8, "f"), 16) >>> 0;
  return (
    <div
      class="item stamp"
      classList={{ pop: fresh && !reducedMotion() }}
      data-item={it().id}
      data-stamp={it().stamp}
      style={{
        left: `${it().x}px`,
        top: `${it().y}px`,
        width: `${it().w}px`,
        height: `${it().h}px`,
        transform: it().rot ? `rotate(${it().rot}rad)` : undefined,
      }}
      innerHTML={stampSvg(it().stamp ?? "emoji", it().text ?? "", color(), it().w, it().h)}
    />
  );
}

/** Long-press quick menu: favourite stamps in a ring around the pen, with
 * Celebrate in the middle. */
export function QuickMenu() {
  const items = () => prefs().stampFavorites.slice(0, 8);
  const close = () => setQuickMenu(null);
  createEffect(on(quickMenu, (m) => {
    if (!m) return;
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", key);
    onCleanup(() => window.removeEventListener("keydown", key));
  }));
  createEffect(on([tool, mode, editingText], close, { defer: true }));
  return (
    <Show when={!isView() && quickMenu()}>
      {(m) => {
        // Keep the ring on screen.
        const R = items().some((id) => stampDef(id)?.render === "label") ? 108 : 84;
        const root = controller.root.getBoundingClientRect();
        const x = () => Math.min(Math.max(root.left + m().x, R + 32), window.innerWidth - R - 32);
        const y = () => Math.min(Math.max(root.top + m().y, R + 32), window.innerHeight - R - 32);
        return (
          <div class="quick-menu" onPointerDown={(e) => e.target === e.currentTarget && close()}>
            <div class="quick-ring" style={{ left: `${x()}px`, top: `${y()}px` }} role="menu" aria-label="Stamps quick menu">
              <For each={items()}>
                {(id, i) => {
                  const a = () => -Math.PI / 2 + (i() * 2 * Math.PI) / Math.max(items().length, 1);
                  return (
                    <button
                      class="quick-item"
                      classList={{ wide: stampDef(id)?.render === "label" }}
                      role="menuitem"
                      aria-label={stampDef(id)?.label ?? id}
                      title={stampDef(id)?.label ?? id}
                      data-stamp={id}
                      style={{ "--dx": `${Math.cos(a()) * R}px`, "--dy": `${Math.sin(a()) * R}px`, "--i": i() }}
                      onClick={() => {
                        const at = m();
                        close();
                        controller.placeStamp(at.x, at.y, id);
                      }}
                    >
                      <StampIcon id={id} size={30} />
                    </button>
                  );
                }}
              </For>
              <button
                class="quick-item quick-center"
                role="menuitem"
                aria-label="Celebrate"
                title="Celebrate: a burst of stars"
                onClick={() => {
                  close();
                  celebrate();
                }}
              >
                <Icon name="celebrate" size={24} active />
              </button>
            </div>
          </div>
        );
      }}
    </Show>
  );
}

// ------------------------------------------------------------ celebration

const BURST_MS = 1700;
const COLORS = ["#f5b301", "#20c997", "#3e63dd", "#e5484d", "#f76b15", "#8e4ec6", "#ffffff"];

interface Bit { x: number; y: number; vx: number; vy: number; r: number; spin: number; a: number; c: string; star: boolean }

/** A short burst of stars and confetti across the board, for a great
 * answer. Reduced motion: a still "Great answer!" badge instead. */
export function Celebrate() {
  const [shown, setShown] = createSignal(false);
  const [still, setStill] = createSignal(false);
  let cv: HTMLCanvasElement | undefined;
  let raf = 0;
  let timer = 0;
  createEffect(on(celebration, (n) => {
    if (!n) return;
    cancelAnimationFrame(raf);
    clearTimeout(timer);
    if (reducedMotion()) {
      setStill(true);
      timer = window.setTimeout(() => setStill(false), 1400);
      return;
    }
    setShown(true);
    queueMicrotask(() => cv && run(cv));
  }, { defer: true }));
  onCleanup(() => (cancelAnimationFrame(raf), clearTimeout(timer)));

  const run = (canvas: HTMLCanvasElement) => {
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    const W = window.innerWidth, H = window.innerHeight;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    const g = canvas.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    // Two cannons at the bottom corners and a shower of stars from the middle.
    const bits: Bit[] = [];
    let seed = 7;
    const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
    const n = Math.min(160, Math.round((W * H) / 9000) + 60);
    for (let i = 0; i < n; i++) {
      const side = i % 3;
      const x = side === 0 ? W * 0.08 : side === 1 ? W * 0.92 : W * (0.3 + 0.4 * rnd());
      const y = side === 2 ? H * 0.42 : H * 0.98;
      const ang = side === 0 ? -Math.PI / 2 + 0.35 + rnd() * 0.5 : side === 1 ? -Math.PI / 2 - 0.35 - rnd() * 0.5 : -Math.PI / 2 + (rnd() - 0.5) * 2.4;
      const sp = (side === 2 ? 5 : 11 + rnd() * 6) * Math.min(1.4, H / 800);
      bits.push({
        x, y, vx: Math.cos(ang) * sp, vy: Math.sin(ang) * sp, r: 5 + rnd() * 7, spin: (rnd() - 0.5) * 0.4, a: rnd() * 6,
        c: COLORS[Math.floor(rnd() * COLORS.length)], star: side === 2 || rnd() < 0.3,
      });
    }
    const t0 = performance.now();
    let last = t0;
    const star = (r: number) => {
      g.beginPath();
      for (let k = 0; k < 10; k++) {
        const a = -Math.PI / 2 + (k * Math.PI) / 5;
        const rr = k % 2 ? r * 0.45 : r;
        if (k) g.lineTo(Math.cos(a) * rr, Math.sin(a) * rr);
        else g.moveTo(Math.cos(a) * rr, Math.sin(a) * rr);
      }
      g.closePath();
      g.fill();
    };
    const frame = (now: number) => {
      const dt = Math.min(2.5, (now - last) / 16.7);
      last = now;
      const t = (now - t0) / BURST_MS;
      g.clearRect(0, 0, W, H);
      g.globalAlpha = t > 0.7 ? Math.max(0, 1 - (t - 0.7) / 0.3) : 1;
      for (const b of bits) {
        b.vy += 0.32 * dt;
        b.vx *= 0.99;
        b.x += b.vx * dt;
        b.y += b.vy * dt;
        b.a += b.spin * dt;
        g.save();
        g.translate(b.x, b.y);
        g.rotate(b.a);
        g.fillStyle = b.c;
        if (b.star) star(b.r);
        else g.fillRect(-b.r * 0.5, -b.r * 0.25, b.r, b.r * 0.5);
        g.restore();
      }
      if (t < 1) raf = requestAnimationFrame(frame);
      else setShown(false);
    };
    raf = requestAnimationFrame(frame);
  };

  return (
    <>
      <Show when={shown()}>
        <canvas ref={cv} class="celebrate" aria-hidden="true" />
      </Show>
      <Show when={still()}>
        <div class="celebrate-still" role="status">
          <span innerHTML={stampSvg("star", "⭐", 0xf5b301ff, 40, 40)} aria-hidden="true" />
          Great answer!
        </div>
      </Show>
      <div class="sr-only" aria-live="polite">{shown() || still() ? "Great answer!" : ""}</div>
    </>
  );
}

