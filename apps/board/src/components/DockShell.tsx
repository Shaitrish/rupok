// The Smart Dock's shell, shared by the board (Dock.tsx) and the desktop
// overlay (overlay/OverlayToolbar.tsx): the grip that drags it anywhere with
// magnet snapping to the edges and corners (vertical on the left and right),
// the minimise chevron and double-tap, the 48px round bubble, and popovers
// that attach to the dock and open on the side with room.
//
// Position is CSS only: `left = margin + (100% − margins) × x` with
// `translate(−x × 100%)` keeps the dock anchored (centred, flush to a corner…)
// while its width changes, and on screen at any window size.

import { type JSX, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { type DockEdge, type DockFrame, type DockPlace, snapPlace } from "../dock";
import type { DockOrientation } from "../modes";
import { Icon } from "./Icon";
import { isDesktop } from "../platform";
import "./Dock.css";

const TAP_MOVE = 5;
const DOUBLE_TAP_MS = 320;

export interface DockShellProps {
  place: DockPlace;
  vertical: boolean;
  orientation: DockOrientation;
  frame: (edge: DockEdge, vertical: boolean) => DockFrame;
  /** A drag moved the dock (`done` once released). */
  onPlace: (p: DockPlace, done: boolean) => void;
  minimized: boolean;
  onMinimized: (min: boolean) => void;
  /** Desktop overlay toolbar window: the window is the dock; the OS moves it. */
  windowed?: boolean;
  class?: string;
  classList?: Record<string, boolean>;
  label: string;
  /** Icon size of the grip and chevron. */
  iconSize: number;
  bubble: JSX.Element;
  bubbleLabel: string;
  /** Any touch, hover or key on the dock (wakes a faded dock). */
  onActivity?: () => void;
  onDragStart?: () => void;
  ref?: (el: HTMLDivElement) => void;
  gripClass?: string;
  children: JSX.Element;
}

/** Six dots: a handle, not an icon, so it is drawn here. */
function GripDots(props: { size: number; vertical: boolean }) {
  const dots = () => (props.vertical ? [[6, 9], [12, 9], [18, 9], [6, 15], [12, 15], [18, 15]] : [[9, 6], [9, 12], [9, 18], [15, 6], [15, 12], [15, 18]]);
  return (
    <svg class="icon" viewBox="0 0 24 24" width={props.size} height={props.size} aria-hidden="true">
      {dots().map(([x, y]) => <circle cx={x} cy={y} r="1.6" fill="currentColor" />)}
    </svg>
  );
}

const chevronFor = (e: DockEdge) => (e === "top" ? "up" : e === "left" ? "prev" : e === "right" ? "next" : "down");

export function DockShell(props: DockShellProps) {
  let el!: HTMLDivElement;
  let bubbleEl: HTMLButtonElement | undefined;
  const [dragging, setDragging] = createSignal(false);

  const posStyle = (w?: number): JSX.CSSProperties | undefined => {
    if (props.windowed) return undefined;
    const p = props.place;
    const f = props.frame(p.edge, props.vertical);
    const style: JSX.CSSProperties = {
      left: `calc(${f.left}px + (100% - ${f.left + f.right}px) * ${p.x})`,
      top: `calc(${f.top}px + (100% - ${f.top + f.bottom}px) * ${p.y})`,
      transform: `translate(${-p.x * 100}%, ${-p.y * 100}%)`,
    };
    if (w === undefined) {
      style["max-width"] = `calc(100% - ${f.left + f.right}px)`;
      style["max-height"] = `calc(100% - ${f.top + f.bottom}px)`;
    }
    return style;
  };

  /** Drag `target` by a pointer; returns true if it moved (else it was a tap). */
  const track = (e: PointerEvent, target: HTMLElement, grip: () => { x: number; y: number }, onTap: () => void) => {
    if (e.button !== 0) return;
    const pid = e.pointerId;
    const [x0, y0] = [e.clientX, e.clientY];
    let moved = false;
    let last: DockPlace | null = null;
    try { target.setPointerCapture(pid); } catch { /* gone */ }
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== pid) return;
      if (!moved && Math.hypot(ev.clientX - x0, ev.clientY - y0) < TAP_MOVE) return;
      if (!moved) {
        moved = true;
        setDragging(true);
        props.onDragStart?.();
      }
      ev.preventDefault();
      const box = props.minimized ? bubbleEl! : el;
      last = snapPlace(ev.clientX, ev.clientY, grip(), { w: box.offsetWidth, h: box.offsetHeight }, props.frame, props.vertical, props.minimized ? "auto" : props.orientation);
      // A bubble keeps the dock's orientation rules: it has none of its own.
      props.onPlace(last, false);
    };
    const end = (ev: PointerEvent) => {
      if (ev.pointerId !== pid) return;
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", end);
      target.removeEventListener("pointercancel", end);
      setDragging(false);
      if (moved && last) props.onPlace(last, true);
      else if (ev.type === "pointerup") onTap();
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", end);
    target.addEventListener("pointercancel", end);
    e.preventDefault();
  };

  let lastTap = 0;
  const onGripDown = (e: PointerEvent) => {
    props.onActivity?.();
    if (props.windowed) return; // data-tauri-drag-region: the OS moves the window
    const g = e.currentTarget as HTMLElement;
    track(e, g, () => ({ x: g.offsetLeft + g.offsetWidth / 2, y: g.offsetTop + g.offsetHeight / 2 }), () => {
      const now = performance.now();
      if (now - lastTap < DOUBLE_TAP_MS) {
        lastTap = 0;
        props.onMinimized(true);
      } else lastTap = now;
    });
  };

  const onBubbleDown = (e: PointerEvent) => {
    props.onActivity?.();
    const b = e.currentTarget as HTMLButtonElement;
    if (props.windowed) {
      // Tap expands; a drag hands the window to the OS.
      if (e.button !== 0) return;
      const [x0, y0] = [e.clientX, e.clientY];
      const move = (ev: PointerEvent) => {
        if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < TAP_MOVE) return;
        cleanup();
        if (isDesktop) void import("@tauri-apps/api/window").then((w) => w.getCurrentWindow().startDragging()).catch(() => {});
      };
      const up = () => {
        cleanup();
        props.onMinimized(false);
      };
      const cleanup = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      return;
    }
    track(e, b, () => ({ x: b.offsetWidth / 2, y: b.offsetHeight / 2 }), () => props.onMinimized(false));
  };

  return (
    <Show
      when={!props.minimized}
      fallback={
        <button
          ref={(b) => (bubbleEl = b)}
          class="dock-bubble"
          classList={{ ...props.classList, dragging: dragging(), windowed: !!props.windowed }}
          style={posStyle(48)}
          aria-label={props.bubbleLabel}
          title={`${props.bubbleLabel}  ·  tap to open, drag to move`}
          onPointerDown={onBubbleDown}
          onPointerEnter={() => props.onActivity?.()}
          onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), props.onMinimized(false))}
        >
          {props.bubble}
        </button>
      }
    >
      <div
        ref={(d) => {
          el = d;
          props.ref?.(d);
        }}
        class={`dock-shell ${props.class ?? ""}`}
        classList={{ ...props.classList, vertical: props.vertical, dragging: dragging(), windowed: !!props.windowed }}
        style={posStyle()}
        data-edge={props.place.edge}
        role="toolbar"
        aria-orientation={props.vertical ? "vertical" : "horizontal"}
        aria-label={props.label}
        onPointerDown={() => props.onActivity?.()}
        onPointerEnter={() => props.onActivity?.()}
      >
        <div
          class={`tool dock-grip ${props.gripClass ?? ""}`}
          data-tauri-drag-region={props.windowed ? "" : undefined}
          title="Drag to move  ·  double-tap to minimise"
          aria-hidden="true"
          onPointerDown={onGripDown}
        >
          <GripDots size={props.iconSize} vertical={props.vertical} />
        </div>
        {props.children}
        <button
          class="tool dock-min"
          title="Minimise the dock  ·  or double-tap the grip"
          aria-label="Minimise the dock"
          onClick={() => props.onMinimized(true)}
        >
          <Icon name={chevronFor(props.windowed ? "bottom" : props.place.edge)} size={props.iconSize} />
        </button>
      </div>
    </Show>
  );
}

// --------------------------------------------------------------- popover

export interface DockPopoverProps {
  /** The dock button it belongs to. */
  anchor: HTMLElement;
  /** The dock (decides the side). */
  dock: () => HTMLElement | undefined;
  vertical: boolean;
  hand: "right" | "left";
  label: string;
  onClose: () => void;
  /** Desktop overlay window: laid out inline (the window grows to fit). */
  inline?: boolean;
  class?: string;
  children: JSX.Element;
}

/** A panel attached to the dock, on the side facing the screen centre. Along
 * the dock it opens away from the writing hand (right-handed: to the left),
 * so the teacher's arm never covers it. */
export function DockPopover(props: DockPopoverProps) {
  let el!: HTMLDivElement;
  const [pos, setPos] = createSignal<{ left: number; top: number } | null>(null);
  const GAP = 8, M = 8;
  const place = () => {
    const d = props.dock();
    if (props.inline || !d || !el) return;
    const dr = d.getBoundingClientRect(), ar = props.anchor.getBoundingClientRect();
    const pw = el.offsetWidth, ph = el.offsetHeight, W = window.innerWidth, H = window.innerHeight;
    const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
    let left: number, top: number;
    if (!props.vertical) {
      const above = dr.top + dr.height / 2 > H / 2;
      top = above ? dr.top - GAP - ph : dr.bottom + GAP;
      left = props.hand === "right" ? ar.right - pw : ar.left;
      left = clamp(left, M, W - pw - M);
      top = clamp(top, M, H - ph - M);
    } else {
      const roomL = dr.left - GAP - pw >= M, roomR = dr.right + GAP + pw <= W - M;
      const onLeft = props.hand === "right" ? roomL || !roomR : !roomR && roomL;
      left = onLeft ? dr.left - GAP - pw : dr.right + GAP;
      top = clamp(ar.top + ar.height / 2 - ph / 2, M, H - ph - M);
      left = clamp(left, M, W - pw - M);
    }
    setPos({ left: Math.round(left), top: Math.round(top) });
  };
  onMount(() => {
    place();
    requestAnimationFrame(place);
    const ro = new ResizeObserver(place);
    ro.observe(el);
    const dock = props.dock();
    if (dock) ro.observe(dock);
    const outside = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!el.contains(t) && !props.anchor.contains(t)) props.onClose();
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("keydown", key);
    window.addEventListener("resize", place);
    onCleanup(() => {
      ro.disconnect();
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("keydown", key);
      window.removeEventListener("resize", place);
    });
  });
  createEffect(() => {
    void props.vertical;
    void props.hand;
    queueMicrotask(place);
  });
  return (
    <div
      ref={el}
      class={`dock-pop ${props.class ?? ""}`}
      classList={{ inline: !!props.inline, placed: !!pos() || !!props.inline }}
      style={pos() && !props.inline ? { left: `${pos()!.left}px`, top: `${pos()!.top}px` } : undefined}
      role="dialog"
      aria-label={props.label}
    >
      {props.children}
    </div>
  );
}

// ---------------------------------------------------------------- pieces

/** Long-press (pen or finger) on a dock button: opens its options. */
export function longPress(onLong: () => void, ms = 450) {
  let timer = 0;
  let fired = false;
  return {
    onPointerDown: (e: PointerEvent) => {
      fired = false;
      if (e.button !== 0) return;
      const [x0, y0] = [e.clientX, e.clientY];
      const t = e.currentTarget as HTMLElement;
      const move = (ev: PointerEvent) => Math.hypot(ev.clientX - x0, ev.clientY - y0) > 8 && cancel();
      const cancel = () => {
        clearTimeout(timer);
        t.removeEventListener("pointermove", move);
        t.removeEventListener("pointerup", cancel);
        t.removeEventListener("pointercancel", cancel);
        t.removeEventListener("pointerleave", cancel);
      };
      timer = window.setTimeout(() => {
        fired = true;
        cancel();
        onLong();
      }, ms);
      t.addEventListener("pointermove", move);
      t.addEventListener("pointerup", cancel);
      t.addEventListener("pointercancel", cancel);
      t.addEventListener("pointerleave", cancel);
    },
    /** True once if the click that follows a long-press must be ignored. */
    consumed: () => {
      const f = fired;
      fired = false;
      return f;
    },
  };
}
