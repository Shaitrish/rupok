// Built-in Smart Object presets: the "Presets" gallery in the Smart Object
// dialog (components/PresetGallery.tsx) and the live demos on the landing
// page (site.ts).
//
// Each preset is a single-file Smart Object from examples/objects/. They are
// validated (with these titles, nominal sizes and step counts) by
// crates/rupok-format/tests/examples.rs; keep the two lists in sync. The
// gallery places a preset scaled to fill the slide frame.
// Objects that use rupoklib:// (three.js, KaTeX) need the vendored libraries:
// run `bun run vendor` before `vite` or `vite build`.
// Gallery thumbnails live in public/presets/<id>-{light,dark}.webp; regenerate
// them with `node scripts/preset-thumbs.mjs` after changing a preset.
//
// No imports and no top-level Vite APIs: scripts/preset-thumbs.mjs loads this
// file with Node.

export type PresetSubject = "Physics" | "Chemistry" | "Maths" | "Biology" | "ICT" | "History" | "Quiz";

export interface Preset {
  /** Stable id (the file name without `.html`). */
  id: string;
  /** Title, also used as the Smart Object's manifest title. */
  title: string;
  subject: PresetSubject;
  /** One short line for the gallery card. */
  summary: string;
  /** One or two sentences: the card's tooltip and the landing page. */
  description: string;
  /** Build steps the presenter's Next button walks through. */
  steps: number;
  /** Nominal size in world units (CSS px at 100 %). */
  w: number;
  h: number;
}

/** The single-file object HTML (wrap with ObjectBundle::from_single_html).
 * Loaded on demand, so the presets cost nothing until one is used. */
export async function presetHtml(id: string): Promise<string> {
  const files = import.meta.glob<string>("../../../examples/objects/*.html", { query: "?raw", import: "default" });
  const load = files[`../../../examples/objects/${id}.html`];
  if (!load) throw new Error(`Unknown preset: ${id}`);
  return load();
}

export const PRESETS: readonly Preset[] = [
  {
    id: "physics-projectile",
    title: "Projectile motion",
    subject: "Physics",
    summary: "Aim and fire on four worlds.",
    description:
      "Drag the launch arrow to aim, then fire on Earth, the Moon, Mars or Jupiter. Fading ghosts at equal times, live velocity vectors and the range equation.",
    steps: 4,
    w: 1920,
    h: 1080,
  },
  {
    id: "chemistry-molecules-3d",
    title: "Molecules in 3D",
    subject: "Chemistry",
    summary: "Six molecules in true 3D.",
    description:
      "Water, CO₂, methane, ammonia, ethanol and benzene in real VSEPR geometry: atoms fly into place, bonds form, lone pairs and bond angles appear. Rotate and zoom by touch.",
    steps: 4,
    w: 1920,
    h: 1080,
  },
  {
    id: "physics-circuit",
    title: "Ohm's law: a simple circuit",
    subject: "Physics",
    summary: "Watch V = I × R update live.",
    description:
      "Build a circuit part by part. Electrons drift faster as the current grows, the bulb glows with its power, and V = I × R updates live. English and বাংলা.",
    steps: 6,
    w: 1920,
    h: 1080,
  },
  {
    id: "chemistry-reaction",
    title: "Burning methane",
    subject: "Chemistry",
    summary: "Bonds break, mass is conserved.",
    description:
      "CH₄ + 2O₂ → CO₂ + 2H₂O at particle level: molecules collide, bonds break and reform, and the energy diagram and atom tallies prove that mass is conserved.",
    steps: 4,
    w: 1920,
    h: 1080,
  },
  {
    id: "math-graph",
    title: "Function grapher",
    subject: "Maths",
    summary: "Drag a curve, see its equation.",
    description:
      "y = a sin(bx + c) + d and y = ax² + bx + c. Drag the peak, period, midline or vertex and watch the equation, amplitude, period and roots change.",
    steps: 5,
    w: 1920,
    h: 1080,
  },
  {
    id: "binary-counter",
    title: "Binary counter",
    subject: "ICT",
    summary: "Tap bits to build 0 to 255.",
    description: "Eight tappable bits with their place values: build any number from 0 to 255, then watch it count.",
    steps: 10,
    w: 1280,
    h: 720,
  },
  {
    id: "computing-timeline",
    title: "A short history of computing",
    subject: "History",
    summary: "From the abacus to AI.",
    description: "From the abacus to artificial intelligence: each step reveals the next era of computing.",
    steps: 8,
    w: 1920,
    h: 1080,
  },
  {
    id: "quiz-networking",
    title: "Quiz: networking basics",
    subject: "Quiz",
    summary: "Four questions with feedback.",
    description: "Four questions on networks with instant feedback, an explanation for each answer and a final score.",
    steps: 4,
    w: 1280,
    h: 720,
  },
];
