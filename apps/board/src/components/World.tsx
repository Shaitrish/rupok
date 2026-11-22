// The world layer: page frame (slide), and DOM-rendered items (Smart Objects,
// images, text). Positioned in world units; the parent applies the camera.

import { For, Match, Show, Switch, createEffect, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { BOARD_COLORS, type DomItem, editingText, items, pageIndex, pages, selection, setEditingText, tool } from "../state";

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
      classList={{ selected: props.item.selected }}
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
        {(p) => (
          <div
            class={`page-frame ${p().pattern}`}
            style={{
              width: `${p().frame?.w ?? 1920}px`,
              height: `${p().frame?.h ?? 1080}px`,
              "background-color": "#" + (p().bg_color >>> 8).toString(16).padStart(6, "0"),
              "--pat": BOARD_COLORS.find((b) => b.rgba === p().bg_color)?.pattern ?? "rgba(30, 60, 120, 0.10)",
            }}
          />
        )}
      </Show>
      <For each={items}>
        {(item) => (
          <Switch>
            <Match when={item.kind === "object"}>
              <div
                class="item object"
                classList={{ selected: item.selected }}
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
            <Match when={item.kind === "image"}>
              <div
                class="item image"
                classList={{ selected: item.selected }}
                style={{ left: `${item.x}px`, top: `${item.y}px`, width: `${item.w}px`, height: `${item.h}px` }}
              >
                <img src={controller.assetUrl(item.asset!)} alt="" draggable={false} />
              </div>
            </Match>
            <Match when={item.kind === "text"}>
              <TextItem item={item} />
            </Match>
          </Switch>
        )}
      </For>
      <Show when={selection()}>
        {(b) => (
          <div
            class="selection-box"
            style={{
              left: `${b()[0] - 6}px`,
              top: `${b()[1] - 6}px`,
              width: `${b()[2] - b()[0] + 12}px`,
              height: `${b()[3] - b()[1] + 12}px`,
            }}
          />
        )}
      </Show>
    </div>
  );
}
