// Tooltips of the Classic islands, in a teacher's words (English and
// Bangla), with the shortcut as a separate key chip ("Hold Space").
// Keyed by the button's aria-label; anything not here shows its label.

export type Lang = "en" | "bn";

export interface Tip {
  en: string;
  bn: string;
  /** Shortcut keys, each shown as a key chip. */
  keys?: string[];
  /** The key is held, not pressed ("Hold Space"). */
  hold?: boolean;
}

export const TIPS: Record<string, Tip> = {
  "Main menu": { en: "Menu: files, export, settings, help", bn: "মেনু: ফাইল, এক্সপোর্ট, সেটিংস, সাহায্য" },
  Mode: { en: "Present or prepare", bn: "উপস্থাপন বা প্রস্তুতি", keys: ["F5"] },
  Tools: { en: "More tools: text, highlighter, pictures, video", bn: "আরও টুল: লেখা, হাইলাইটার, ছবি, ভিডিও" },
  Share: { en: "Share the lesson", bn: "পাঠ শেয়ার করুন" },
  Zoom: { en: "Zoom in or out", bn: "বড় বা ছোট করে দেখুন", keys: ["+", "−"] },
  "Ask AI": { en: "Ask AI", bn: "AI-কে জিজ্ঞেস করুন" },
  "Select & move": { en: "Select", bn: "বাছাই করুন", keys: ["V"] },
  Pen: { en: "Pen", bn: "কলম", keys: ["P"] },
  Highlighter: { en: "Highlighter", bn: "হাইলাইটার", keys: ["H"] },
  Eraser: { en: "Eraser", bn: "ইরেজার", keys: ["E"] },
  Shapes: { en: "Shapes", bn: "আকৃতি", keys: ["S"] },
  Hand: { en: "Hand: move the board", bn: "হাত: বোর্ড সরান", keys: ["Space"], hold: true },
  "Recognise ink": { en: "Recognise ink: neat shapes, text", bn: "লেখা চেনা: সুন্দর আকৃতি, টেক্সট", keys: ["M"] },
  Stamps: { en: "Stamps", bn: "স্ট্যাম্প", keys: ["K"] },
  Undo: { en: "Undo", bn: "আগের অবস্থায় ফিরুন", keys: ["Ctrl", "Z"] },
  Redo: { en: "Redo", bn: "আবার করুন", keys: ["Ctrl", "Shift", "Z"] },
  "New page": { en: "Add page", bn: "পৃষ্ঠা যোগ করুন" },
  "Previous page": { en: "Previous page", bn: "আগের পৃষ্ঠা", keys: ["←"] },
  "Next page": { en: "Next page", bn: "পরের পৃষ্ঠা", keys: ["→"] },
  "All pages": { en: "All pages", bn: "সব পৃষ্ঠা", keys: ["G"] },
  "Swap the side islands": { en: "Move to the other side", bn: "অন্য পাশে সরান" },
  Close: { en: "Close", bn: "বন্ধ করুন", keys: ["Esc"] },
};

/** The board's language for these tips: Bangla when the page or the browser says so. */
export function tipLang(): Lang {
  if (typeof document !== "undefined" && document.documentElement.lang === "bn") return "bn";
  return typeof navigator !== "undefined" && navigator.language?.toLowerCase().startsWith("bn") ? "bn" : "en";
}

/** The tip for a button's aria-label (the page counter's label starts "Page n of m"). */
export function tipFor(label: string): Tip | null {
  if (TIPS[label]) return TIPS[label];
  if (/^Page \d+ of \d+/.test(label)) return TIPS["All pages"];
  return null;
}
