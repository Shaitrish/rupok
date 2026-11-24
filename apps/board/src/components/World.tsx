// The world layer: page frame (slide), and DOM-rendered items (Smart Objects,
// images, text). Positioned in world units; the parent applies the camera.
// The selection frame and handles live in screen space (Selection.tsx) so
// they stay crisp and constant-size at any zoom.

import { For, Match, Show, Switch, createEffect, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { renderTex } from "../katex";
import { MediaItem } from "./MediaItem";
import { BOARD_COLORS, type DomItem, editingText, items, pageIndex, pages, setEditingText, tool } from "../state";
import { brandingFor, designPreview, lessonBranding } from "../design";
import { PageBackdrop } from "./PageBackdrop";

function ObjectItem(props: { item: DomItem }) {
  let iframe!: HTMLIFrameElement;
  onMount(() => {
    controller.objects.register(props.item.id, iframe, props.item);
    iframe.srcdoc = controller.objects.srcdoc(props.item.bundle!);
  });
  onCleanup(() => controller.objects.unregister(props.item.id));
  return (
    <>
      <iframe
        ref={iframe}
        title={props.item.title}
        sandbox="allow-scripts"
        allow="fullscreen"
        referrerpolicy="no-referrer"
        loading="eager"
      />
      <Show when={(props.item.steps ?? 0) > 0}>
        <div class="step-badge">
          {props.item.step ?? 0} / {props.item.steps}
        </div>
      </Show>
    </>
  );
}

/** A text item whose text is LaTeX: typeset with KaTeX, edited as source. */
function MathItem(props: { item: DomItem }) {
  let el!: HTMLDivElement;
  const editing = () => editingText() === props.item.id;
  createEffect(() => {
    const src = props.item.text ?? "";
    if (editing()) {
      el.textContent = src;
      el.focus();
      const r = document.createRange();
      r.selectNodeContents(el);
      const s = getSelection();
      s?.removeAllRanges();
      s?.addRange(r);
    } else {
      renderTex(el, src);
    }
  });
  const commit = () => {
    if (!editing()) return;
    setEditingText(null);
    controller.board.set_text(props.item.id, el.innerText.replace(/\n$/, ""), el.offsetHeight);
  };
  return (
    <div
      ref={el}
      class="item text math"
      data-item={props.item.id}
      contentEditable={editing()}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Escape" || e.key === "Enter") el.blur();
        e.stopPropagation();
      }}
      style={{
        left: `${props.item.x}px`,
        top: `${props.item.y}px`,
        "min-width": `${props.item.w}px`,
        "font-size": `${props.item.size}px`,
        color: props.item.color ? props.item.color.slice(0, 7) : undefined,
        "pointer-events": editing() ? "auto" : "none",
      }}
    />
  );
}

function TextItem(props: { item: DomItem }) {
  let el!: HTMLDivElement;
  const editing = () => editingText() === props.item.id;
  createEffect(() => {
    if (editing()) {
      el.focus();
      const r = document.createRange();
      r.selectNodeContents(el);
      const s = getSelection();
      s?.removeAllRanges();
      s?.addRange(r);
    }
  });
  const commit = () => {
    if (!editing()) return;
    setEditingText(null);
    const text = el.innerText.replace(/\n$/, "");
    controller.board.set_text(props.item.id, text, el.offsetHeight);
  };
  return (
    <div
      ref={el}
      class="item text"
      data-item={props.item.id}
      contentEditable={editing()}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Escape") el.blur();
        e.stopPropagation();
      }}
      style={{
        left: `${props.item.x}px`,
        top: `${props.item.y}px`,
        "min-width": `${props.item.w}px`,
        "font-size": `${props.item.size}px`,
        color: props.item.color ? props.item.color.slice(0, 7) : undefined,
        "pointer-events": editing() ? "auto" : "none",
      }}
    >
      {props.item.text}
    </div>
  );
}

export default function World(props: { ref: (el: HTMLDivElement) => void }) {
  const page = () => pages()[pageIndex()];
  return (
    <div class="world" ref={props.ref}>
      <Show when={page()}>
        {(p) => {
          // A drag in Page design previews here before it is committed.
          const background = () => { const o = designPreview(); return o && "background" in o ? o.background : p().background; };
          return (
            <div
              class={`page-frame ${background() ? "" : p().pattern}`}
              style={{
                width: `${p().frame?.w ?? 1920}px`,
                height: `${p().frame?.h ?? 1080}px`,
                "background-color": "#" + (p().bg_color >>> 8).toString(16).padStart(6, "0"),
                "--pat": BOARD_COLORS.find((b) => b.rgba === p().bg_color)?.pattern ?? "rgba(30, 60, 120, 0.10)",
              }}
            >
              <PageBackdrop
                layer="background"
                w={p().frame?.w ?? 1920}
                h={p().frame?.h ?? 1080}
                bg={p().bg_color}
                pattern={p().pattern}
                background={background()}
                branding={null}
                place={{ page: pageIndex() + 1, pages: pages().length }}
                assetUrl={(n) => controller.assetUrl(n)}
              />
            </div>
          );
        }}
      </Show>
      <For each={items}>
        {(item) => (
          <Switch>
            <Match when={item.kind === "object"}>
              <div
                class="item object"
                data-item={item.id}
                style={{
                  left: `${item.x}px`,
                  top: `${item.y}px`,
                  width: `${item.w}px`,
                  height: `${item.h}px`,
                  // While drawing, objects ignore the pointer so ink goes on top.
                  "pointer-events": tool() === "interact" ? "auto" : "none",
                }}
              >
                <ObjectItem item={item} />
              </div>
            </Match>
            <Match when={item.kind === "image" && item.media}>
              <MediaItem item={item} />
            </Match>
            <Match when={item.kind === "image"}>
              <div
                class="item image"
                data-item={item.id}
                style={{ left: `${item.x}px`, top: `${item.y}px`, width: `${item.w}px`, height: `${item.h}px` }}
              >
                <img src={controller.assetUrl(item.asset!)} alt="" draggable={false} />
              </div>
            </Match>
            <Match when={item.kind === "text" && item.latex}>
              <MathItem item={item} />
            </Match>
            <Match when={item.kind === "text"}>
              <TextItem item={item} />
            </Match>
          </Switch>
        )}
      </For>
      {/* Branding: above the items (a logo shows on imported slides), under the ink. */}
      <Show when={page()}>
        {(p) => {
          const branding = () => { const o = designPreview(); return o && "branding" in o ? o.branding : brandingFor(p(), lessonBranding()); };
          return (
            <Show when={branding()}>
              <div class="page-brand" style={{ width: `${p().frame?.w ?? 1920}px`, height: `${p().frame?.h ?? 1080}px` }}>
                <PageBackdrop
                  layer="branding"
                  w={p().frame?.w ?? 1920}
                  h={p().frame?.h ?? 1080}
                  bg={p().bg_color}
                  pattern={p().pattern}
                  background={null}
                  branding={branding()}
                  place={{ page: pageIndex() + 1, pages: pages().length }}
                  assetUrl={(n) => controller.assetUrl(n)}
                />
              </div>
            </Show>
          );
        }}
      </Show>
    </div>
  );
}
