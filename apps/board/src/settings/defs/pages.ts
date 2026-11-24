// Settings → Appearance → Pages: the pages drawer, Page design, and the
// teacher's default look for new lessons (design.ts).

import { designDefault, setDesignDefault } from "../../design";
import { designDefaultStore } from "../../storage";
import { openPageDesign } from "../../components/PageDesign";
import { showToast } from "../../state";
import { pref } from "../bind";
import { closeSettings } from "../open";
import { registerSettings } from "../registry";

const PAGES = ["pages", "slides", "thumbnails", "পৃষ্ঠা", "পাতা", "স্লাইড"];

registerSettings([
  {
    id: "appearance.pagesPinned", category: "appearance", group: "Pages", order: 10, type: "toggle", label: "Keep the pages drawer open",
    description: "After you tap a page in Show pages (G), the drawer stays open instead of closing.",
    keywords: [...PAGES, "drawer", "pin", "overview", "jump", "go to page", "পিন", "ড্রয়ার"],
    ...pref("pagesPinned"),
  },
  {
    id: "appearance.pageDesign", category: "appearance", group: "Pages", order: 20, type: "action", label: "Page design",
    description: "Background image, looks such as chalkboard or exam paper, and a school logo and name on every page.",
    keywords: [
      ...PAGES, "background", "logo", "branding", "school", "watermark", "template", "chalkboard", "exam paper", "header", "footer",
      "পটভূমি", "ব্যাকগ্রাউন্ড", "লোগো", "স্কুল", "বিদ্যালয়", "নকশা", "ডিজাইন", "শিক্ষক", "শ্রেণি", "তারিখ",
    ],
    button: "Open", icon: "palette",
    run: () => {
      closeSettings();
      openPageDesign();
    },
  },
  {
    id: "appearance.pageDesignDefault", category: "appearance", group: "Pages", order: 30, type: "action", label: "Default design for new lessons",
    description: "New lessons on this device start with your saved look and branding. Save one from Page design.",
    keywords: [...PAGES, "default", "new lesson", "branding", "logo", "every day", "ডিফল্ট", "নতুন পাঠ", "লোগো"],
    when: () => !!designDefault(),
    button: "Clear", icon: "trash", danger: true, confirm: "Click again to clear",
    run: async () => {
      await designDefaultStore(null);
      setDesignDefault(null);
      showToast("New lessons start plain again.");
    },
  },
]);
