// The page design layers of one page (design.ts). World units; the parent
// scales. Two layers, drawn apart on the board:
// - "background": the background image, the page-colour wash of "Dim for
//   readability" and the pattern (above the image), inside `.page-frame`,
//   under the items;
// - "branding": the logo and the text line, above the items (so a logo shows
//   on imported PDF slides and pictures) and under the ink canvas.
//
// Neither is an item: never hit-tested, selected or erased. Both are plain
// DOM that changes only when the design does, so the compositor rasterises
// them once (per zoom level); pen frames never touch them.
// The Page design sheet reuses it, scaled, as its live preview, where the
// logo can be dragged.

import { Show, createMemo, createResource } from "solid-js";
import {
  type BrandPlace, type Branding, type PageBackground, DIM_ALPHA, bgCss, brandInk, brandLine, brandTextBox, cssColor, hasBranding, imageSize,
} from "../design";

export interface BackdropProps {
  w: number;
  h: number;
  /** Page colour 0xRRGGBBAA. */
  bg: number;
  pattern: string;
  background: PageBackground | null | undefined;
  branding: Branding | null | undefined;
  assetUrl: (name: string) => string;
  /** The page (1-based) and page count, for `{page}` and `{pages}` in the text. */
  place: BrandPlace;
  /** Which layers to draw (default both: the Page design preview). */
  layer?: "background" | "branding";
  /** Page design preview: pointer down on the logo starts a drag. */
  onLogoPointerDown?: (e: PointerEvent) => void;
}

export function PageBackdrop(props: BackdropProps) {
  const bgUrl = createMemo(() => (props.background ? props.assetUrl(props.background.image) : ""));
  const [bgSize] = createResource(bgUrl, (u) => (u ? imageSize(u) : Promise.resolve({ w: 0, h: 0 })));
  const brand = () => (hasBranding(props.branding) ? props.branding : null);
  const logoUrl = createMemo(() => (brand()?.logo ? props.assetUrl(brand()!.logo!) : ""));
  const line = () => (brand() ? brandLine(brand()!, props.place) : "");
  const box = () => brandTextBox(brand()!, props.w, props.h);
  const bgOn = () => props.layer !== "branding";
  const brandOn = () => props.layer !== "background";
  return (
    <div class="page-design" classList={{ [`pd-${props.layer}`]: !!props.layer }} aria-hidden="true">
      <Show when={bgOn() && props.background && bgUrl()}>
        <div
          class="pd-bg"
          style={{
            "background-image": `url("${bgUrl()}")`,
            opacity: String(props.background!.opacity),
            ...bgCss(props.background!.fit, bgSize()?.w ?? 0, bgSize()?.h ?? 0, props.w, props.h),
          }}
        />
        <Show when={props.background!.dim}>
          <div class="pd-dim" style={{ background: cssColor(props.bg), opacity: String(DIM_ALPHA) }} />
        </Show>
        {/* With an image the pattern is drawn above it (the frame's own is off). */}
        <Show when={props.pattern !== "plain"}>
          <div class={`pd-pattern ${props.pattern}`} />
        </Show>
      </Show>
      <Show when={brandOn() && logoUrl()}>
        <img
          class="pd-logo"
          classList={{ editable: !!props.onLogoPointerDown }}
          src={logoUrl()}
          alt=""
          draggable={false}
          onPointerDown={(e) => props.onLogoPointerDown?.(e)}
          style={{
            left: `${brand()!.logo_x * props.w}px`,
            top: `${brand()!.logo_y * props.h}px`,
            width: `${brand()!.logo_w * props.w}px`,
            opacity: String(brand()!.logo_opacity),
          }}
        />
      </Show>
      <Show when={brandOn() && line()}>
        <div
          class="pd-text"
          style={{
            left: `${box().rect[0]}px`,
            top: `${box().rect[1]}px`,
            width: `${box().rect[2]}px`,
            height: `${box().rect[3]}px`,
            "line-height": `${box().rect[3]}px`,
            "font-size": `${box().size}px`,
            "text-align": brand()!.text_align,
            color: brandInk(props.bg),
          }}
        >
          {line()}
        </div>
      </Show>
    </div>
  );
}
