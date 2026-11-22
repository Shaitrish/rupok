# Rupok Design System

Rupok is used on 86-inch classroom panels, on teachers' laptops, and on students' phones. It is also used under projectors and with chalk-dusted fingers. The design has one job: **stay out of the way of the lesson**. The page and the ink are the content, and the interface around them is a quiet, precise instrument.

This document is the source of truth. The tokens live in [`apps/board/src/styles.css`](apps/board/src/styles.css), and nothing in the UI may use a raw value that isn't a token.

---

## 1. Principles

1. **Content first.** Chrome floats in translucent surfaces at the screen edges. The page is never boxed in.
2. **Calm, not loud.** Colour comes from neutrals. The accent marks only the current state (the selected tool, the primary action, the current page).
3. **Every pixel on the grid.** Spacing is a 4px grid, every control in a row shares one cell size, and radii are concentric. When something looks "almost aligned", it is a bug.
4. **Built for touch at arm's length.** Hit targets are at least 40px, and 56px in *Large board* density. No feature depends on hover.
5. **The teacher is in control.** Theme, accent, icon style, toolbar side, density, pen feel and board colour are all user settings.
6. **Works offline.** Fonts, icons and libraries are bundled. A classroom without Wi-Fi still gets the full design.

---

## 2. Brand

**Rupok** (রূপক, "metaphor") turns ideas into form. Tagline: **Mind Over Matter**.

| Asset | File | Use |
|---|---|---|
| Mark | [`brand/rupok-mark.svg`](brand/rupok-mark.svg) | UI, avatar, loading screen (28px and up) |
| Favicon cut | [`brand/rupok-favicon.svg`](brand/rupok-favicon.svg) | 16–32px: heavier stroke and spark |
| App icon | [`brand/rupok-app-icon.svg`](brand/rupok-app-icon.svg) → `icon-192/512.png`, `apple-touch-icon.png` | Full-bleed; the platform applies its own mask |
| Wordmark | [`rupok-wordmark.svg`](brand/rupok-wordmark.svg) · [`-dark`](brand/rupok-wordmark-dark.svg) | Mark + name, for headers |
| Lockup | [`rupok-lockup.svg`](brand/rupok-lockup.svg) · [`-dark`](brand/rupok-lockup-dark.svg) | Mark + name + tagline, for README, site and slides |
| Social preview | [`brand/rupok-social.png`](brand/rupok-social.png) (1280×640) | GitHub social image, `og:image` |

**The mark.** A lowercase **r** drawn in one round-capped pen stroke stands for *matter*, the ink. An amber idea spark floats *over* it and stands for *mind*. Together they read as "Mind Over Matter". Both sit on the board squircle: an iris gradient from `#7B76F0` to `#4440C0` with a 20% top sheen.

Mark geometry (64-unit grid):
- squircle: inset 4, radius 17;
- stroke: 6 units, white, round caps;
- spark: a four-point concave star of radius 6.5 in `#FFD36E`;
- the composition is optically centred.

Rules for the mark:
- Use the mark at 28px or larger, and the favicon cut below that.
- Keep 4 units of clear space around it, which is already inside the viewBox.
- Never recolour the stroke or the spark, and never place the mark on a busy image.

**The wordmark.** `rupok` is always lowercase, set in Figtree Bold at −0.02em tracking. The lettering is converted to vector paths, so it renders identically without the font.
- Text colour: `#1C2024` on light backgrounds, `#EDEEF0` on dark.
- Placement: x-height centred on the mark, with 14 units between mark and wordmark.

**The tagline.** `MIND OVER MATTER` is set in Figtree SemiBold, uppercase, at +0.16em tracking. In the lockup it is sized so it matches the wordmark width exactly.

Brand assets are regenerated from glyph outlines with fontTools; outside the app the font is not needed.

---

## 3. Colour

### Neutrals (slate, 12 steps)

Steps follow the Radix scale semantics:

| Step | Role |
|---|---|
| 1–2 | App backgrounds |
| 3–5 | Component backgrounds: rest, hover, pressed |
| 6–8 | Borders and hairlines |
| 9–10 | Solid UI and placeholders |
| 11 | Secondary text |
| 12 | Primary text |

| Token | Light | Dark | Use |
|---|---|---|---|
| `--canvas` | `#eceef2` | `#0d0e10` | Board background around the page |
| `--surface` | `rgba(255,255,255,.9)` + blur 20px | `rgba(28,29,32,.9)` + blur | Floating chrome |
| `--surface-solid` | `#ffffff` | `#1b1c1f` | Dialogs |
| `--text` / `--text-2` / `--text-3` | gray 12 / 11 / 9 | gray 12 / 11 / 9 | Primary / secondary / tertiary text |
| `--hairline` | gray-a5 | gray-a5 | 1px separators and surface edges |
| `--hover` / `--pressed` | gray-a3 / a4 | gray-a3 / a4 | Ghost control states |

### Accent: user-selectable, Iris by default

Iris is calm, friendly and readable for every age, and it doesn't clash with ink colours. Seven accents ship: **Iris, Teal, Blue, Violet, Grass, Ruby, Amber**. Each defines the same tokens for light and dark:

| Token | Use |
|---|---|
| `--accent-3` / `--accent-4` | Soft background of the active tool or option, rest and hover |
| `--accent-9` / `--accent-10` | Solid primary button, focus ring, current page, rest and hover |
| `--accent-11` | Accent text and the active icon colour |
| `--accent-contrast` | Text on `--accent-9`: white, or dark on Amber |

The accent is never used for decoration. If it appears on screen, it means *selected*, *primary* or *focused*.

### Ink and board colours

- **Pen:** graphite `#1c2024`, blue `#3e63dd`, red `#e5484d`, green `#30a46c`, orange `#f76b15`, purple `#8e4ec6`, white. All are chosen to read from the back of a classroom.
- **Highlighter:** yellow, mint, sky, pink, peach. They are rendered at 38% opacity, so text underneath stays legible.
- **Board:** White, Paper `#fbf7ed`, Chalkboard `#1f3b30`, Blackboard `#17191c`. Switching to a dark board flips the default graphite pen to white.

### Status

| State | Light | Dark | Use |
|---|---|---|---|
| Danger | `#dc3e42` on `#feebec` | `#ff6369` on `#3b1219` | Destructive actions and validation errors |
| Selection | `#3e63dd` | `#6e8bff` | Lasso and selection outlines on the page; the same in every accent |

---

## 4. Typography

- **UI typeface:** [Figtree](https://github.com/erikdkennedy/figtree), variable, OFL, bundled. It is friendly, highly legible and has open apertures, which help on projectors.
- **Bangla:** Noto Sans Bengali, variable and bundled. It is the fallback in the same stack.
- **Monospace:** the system UI monospace, used for code and HTML only.
- **Numbers:** `font-variant-numeric: tabular-nums` wherever numbers change (zoom, time, steps).

| Style | Size / line | Weight | Use |
|---|---|---|---|
| Dialog title | 18 / 24 | 700 | Dialog headings |
| Body | 14 / 20 | 450 | Inputs, descriptions |
| Control | 13 / 16 | 550 | Buttons, segmented controls |
| Label | 12 / 16 | 600 | Field labels |
| Overline | 12 / 16 | 650, uppercase, +0.02em | Section titles, panel heads |
| Caption | 11 / 12 | 550 | Statistics, counts |

---

## 5. Space, size, radius

- **Grid:** 4px. The tokens are `--s1` 4 · `--s2` 8 · `--s3` 12 · `--s4` 16 · `--s5` 20 · `--s6` 24 · `--s8` 32.
- **Floating chrome inset:** `--inset` 12px from every screen edge. The gap between clusters is 8px.
- **Top bar:** height 48px, padding 4px, controls 40px. The bar radius is 14px, which is the control radius (10px) plus the 4px padding.
- **Toolbar cell (`--tb`):** every tool, swatch and size button is the same square, so the toolbar is one perfectly regular row.

  | Density | Cell | Padding | Icon | Cell radius | Bar radius |
  |---|---|---|---|---|---|
  | Compact | 40 | 4 | 20 | 10 | 14 |
  | Comfortable (default) | 48 | 6 | 24 | 12 | 18 |
  | Large board | 56 | 8 | 28 | 14 | 22 |

- **Concentric radii:** an inner radius always equals the outer radius minus the padding between them. Never put a rounded child flush inside a parent with a different radius.
- **Icon sizes:** only 16, 20, 24 or 28. Solar is drawn on a 24px grid, so these sizes stay crisp.
- **Separators:** 1px `--hairline`, 20px tall in the top bar and 24px in the toolbar, with 6px margins.

---

## 6. Elevation

There are only two levels:

| Level | Token | Use |
|---|---|---|
| 1 | `--shadow-1` | Floating chrome: top bar clusters, toolbar, pages, zoom |
| 2 | `--shadow-2` | Dialogs, over a `--scrim` |

The edge of a surface is drawn by a 1px hairline ring inside its shadow, not by a border. This keeps sizes exact.

The page has its own soft `--page-shadow`, so a slide reads as a physical sheet on the canvas.

---

## 7. Iconography

**Solar** by 480 Design, licensed CC BY 4.0, is the icon set. It is chosen for precise, friendly geometry, and because it is less ubiquitous than Lucide or Material.

- Only the icons the app uses are extracted into `src/icons.generated.ts` by `apps/board/scripts/gen-icons.mjs`. Run that script after adding a name to its `MAP`.
- **Style is a user setting:** Linear (default), Duotone, Broken or Bold. All four share the same names and geometry.
- **The active state** renders the *Bold Duotone* weight in `--accent-11`, so the selected tool reads at a glance even across a classroom.
- Icons are `currentColor` and never carry their own colour, except the mark.

Attribution lives in `NOTICE` and in Settings → About.

---

## 8. Components

| Component | Spec |
|---|---|
| **Ghost button** `.btn` | 40px tall, radius 10, 8px side padding, icon 20 plus label 13/550. Hover `--hover`, pressed `--pressed`, disabled `--text-3`. |
| **Icon button** `.btn.icon` | 40×40. |
| **Primary button** `.btn.solid` | `--accent-9` fill, `--accent-contrast` text, hover `--accent-10`. At most one per surface. |
| **Soft button** `.btn.soft` | `--accent-3` fill, `--accent-11` text. Used for play/pause. |
| **Tool** `.tool` | `--tb` square, secondary text colour. Selected: `--accent-3` background, `--accent-11` icon, bold-duotone weight. |
| **Swatch** | A circle of icon size + 2px inside a tool cell. Selected: 2px surface gap plus a 2px accent ring. |
| **Segmented control** `.seg` | 36px track (gray-a3), 32px options, radius 10/8. The selected option is a raised surface with a hairline and a small shadow. |
| **Input** | 40px, radius 10, gray-a3 fill. Focus switches to the solid surface with a 2px inset accent ring. There is no separate border. |
| **Dialog** | Radius 20, padding 24, shadow 2. The header is a 40px soft-accent icon badge, then the title, the subtitle, and a close button. |
| **Toast** | 40px pill under the top bar, with an info icon. It auto-dismisses after about 2.6s. |
| **Page thumbnail** | 90px tall and tinted with the page's board colour. The page-number chip sits bottom-left. The current page gets a 2px accent ring. |
| **Step badge** | A 28px pill at the bottom-right of a Smart Object showing `step / steps`, in tabular numbers. |

---

## 9. Board-specific rules

- **Drawing always wins.** While a drawing tool is active, Smart Objects ignore the pointer and ink goes on top. *Use objects* (`I`) hands the pointer to them.
- **Fit respects chrome.** The engine knows the screen insets that floating UI covers, so "Fit page" and newly placed items centre in the *visible* area.
- **Toolbar placement:** bottom by default. Left or right suits tall panels where teachers can't reach the bottom, and for left- or right-handed use.
- **Laser pointer:** red `#ff2d55`, 9px, fades out over 1.4s. It is never saved.
- **Projector legibility:** objects and text should use 24px or larger at 1920×1080. The Smart Object guide tells agents this.

---

## 10. Motion

- One easing curve: `--ease: cubic-bezier(0.2, 0, 0, 1)`.
- Hover and state changes take 120ms. Dialogs pop in 180ms, translating 4px and scaling from 0.98. The scrim fades in 140ms.
- Ink never animates, so latency is sacred. Only replay and laser trails move.
- `prefers-reduced-motion` disables every animation and transition.

---

## 11. Accessibility

- Text contrast is at least 4.5:1, and UI components are at least 3:1, in both themes and all accents. Amber uses dark contrast text.
- Every control is reachable by keyboard, with a visible 2px accent focus ring offset 2px.
- Icon-only controls carry `aria-label`. Toggles use `aria-pressed` or `role="radio"`.
- Never rely on colour alone: the current tool also changes icon weight, and the current page also shows a ring.

---

## 12. Customisation (Settings)

| Setting | Options |
|---|---|
| Theme | System · Light · Dark |
| Accent | Iris · Teal · Blue · Violet · Grass · Ruby · Amber |
| Icons | Linear · Duotone · Broken · Bold |
| Toolbar | Bottom · Left · Right |
| Size | Compact · Comfortable · Large board |
| Pen & touch | Finger mode (auto palm rejection / draw / pan & zoom), smoothing, pressure curve |
| Board | Page pattern (plain, grid, dots, lines) and board colour, per page |

Planned customisation (see the [roadmap](ROADMAP.md)) includes custom accent hex values, toolbar tool order and pinning, a high-contrast theme, and agent-driven theming through the MCP tool `set_ui_theme`.

---

## 13. Pixel-perfect review checklist

Run this before merging any UI change. Screenshot at 1×, 1.5× and 2× DPR, in light and dark, and check:

- [ ] Every size, gap and padding is a token, or a multiple of 4.
- [ ] Controls in a row share a height, and their icons are vertically centred (same cell, `display: grid; place-items: center`).
- [ ] Radii are concentric (inner = outer − padding).
- [ ] Icons are 16, 20, 24 or 28px, `currentColor`, with no stray `fill`.
- [ ] Hairlines are exactly 1px, `--hairline`.
- [ ] Hover, pressed, focus, disabled and selected states exist and use tokens.
- [ ] Dark mode has no hard-coded light colours, and dark swatches get the light inner ring.
- [ ] Text never clips at the 1100px breakpoint, where labels collapse to icons.
- [ ] Touch targets are 40px or larger, 56px in Large board density.
- [ ] No layout shift when a toast or the dirty dot appears.
