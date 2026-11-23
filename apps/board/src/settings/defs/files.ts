// Settings → Files & export: PDF export defaults (the same `rupok.export`
// key the export dialog remembers), quiz answers and a new empty lesson.

import { APP_SLUG } from "../../brand";
import { controller } from "../../controller";
import { autosaveClear } from "../../storage";
import { downloadQuiz } from "../../components/Chrome";
import { openExport } from "../../components/ExportDialog";
import { closeSettings } from "../open";
import { jsonStore } from "../bind";
import { registerSettings } from "../registry";

const pdf = jsonStore(`${APP_SLUG}.export`, { layout: "slides", printFriendly: false, includeInk: true });

registerSettings([
  {
    id: "files.pdfLayout", category: "files", group: "PDF export", order: 10, type: "segmented", label: "Default layout",
    description: "What Export PDF starts with. You can still change it each time.",
    keywords: ["pdf", "export", "handout", "print", "slides per page", "পিডিএফ", "প্রিন্ট"],
    options: [{ value: "slides", label: "Slides" }, { value: "handout3", label: "Handout · 3" }, { value: "handout2", label: "Handout · 2" }],
    get: () => pdf.read().layout,
    set: (v) => pdf.write({ layout: v }),
    default: "slides",
  },
  {
    id: "files.pdfInk", category: "files", group: "PDF export", order: 20, type: "toggle", label: "Include ink",
    description: "Pen and highlighter in the PDF. Shapes are always kept.",
    keywords: ["pdf", "export", "annotations", "drawing"],
    get: () => pdf.read().includeInk,
    set: (v) => pdf.write({ includeInk: v }),
    default: true,
  },
  {
    id: "files.pdfPrintFriendly", category: "files", group: "PDF export", order: 30, type: "toggle", label: "Print-friendly",
    description: "Chalkboard and dark pages become white paper; light ink turns dark.",
    keywords: ["pdf", "print", "ink saving", "white background", "প্রিন্ট"],
    get: () => pdf.read().printFriendly,
    set: (v) => pdf.write({ printFriendly: v }),
    default: false,
  },
  {
    id: "files.exportPdf", category: "files", group: "PDF export", order: 40, type: "action", label: "Export this lesson as PDF",
    description: "Opens Export PDF (Ctrl Shift E).", keywords: ["pdf", "download", "share"],
    button: "Export PDF…", icon: "pdf",
    run: () => { closeSettings(); openExport(); },
  },
  {
    id: "files.quizAnswers", category: "files", group: "This lesson", order: 100, type: "action", label: "Quiz answers",
    description: "Download what students answered in this lesson's quizzes, as a JSON file.",
    keywords: ["quiz", "results", "csv", "answers", "download", "কুইজ"],
    button: "Download", icon: "download",
    run: downloadQuiz,
  },
  {
    id: "files.newLesson", category: "files", group: "This lesson", order: 110, type: "action", label: "New empty lesson",
    description: "Clears the board and its autosave. Lessons saved in your library stay.",
    keywords: ["clear", "start over", "blank", "reset board", "নতুন"],
    button: "New lesson", icon: "trash", danger: true, confirm: "Click again to clear",
    run: async () => {
      await autosaveClear();
      controller.board.new_lesson("Untitled lesson");
      closeSettings();
    },
  },
]);
