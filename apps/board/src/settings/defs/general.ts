// Settings → General: who you are and where your lessons live (Library).

import { libPrefs, setLibPrefs } from "../../library/prefs";
import { bump } from "../../library/session";
import { LibraryLocation } from "../../components/settings/LibraryLocation";
import { registerSettings } from "../registry";

registerSettings([
  {
    id: "general.role", category: "general", order: 10, type: "segmented", label: "I use Rupok as a",
    description: "Students start on lessons shared with them and keep their own copies.",
    keywords: ["role", "teacher", "student", "শিক্ষক", "শিক্ষার্থী", "ছাত্র"],
    options: [{ value: "teacher", label: "Teacher" }, { value: "student", label: "Student" }],
    get: () => libPrefs().role ?? "teacher",
    set: (v) => { setLibPrefs({ role: v as "teacher" | "student", setupDone: true }); bump(); },
  },
  {
    id: "general.author", category: "general", order: 20, type: "text", label: "Your name",
    description: "Saved inside lessons you make or copy, so copies can credit you.",
    keywords: ["author", "name", "credit", "নাম"],
    placeholder: "Shown on lessons you make", maxLength: 120,
    get: () => libPrefs().author,
    set: (v) => setLibPrefs({ author: v.trim().slice(0, 120) }),
  },
  {
    id: "general.library", category: "general", group: "Library", order: 100, type: "section", label: "Where your lessons live",
    description: "This browser, or a folder on this computer that Drive, OneDrive or the desktop app can sync.",
    keywords: ["library location", "folder", "storage", "save location", "Google Drive", "OneDrive", "sync", "browser storage", "keep safe", "লাইব্রেরি", "ফোল্ডার"],
    component: LibraryLocation,
  },
]);
