// The Settings sidebar. Ordered by what teachers look for; a feature should
// put its settings in one of these rather than add a category.

import { registerCategory } from "../registry";

registerCategory({ id: "general", order: 10, icon: "settings", label: "General", description: "Who you are and where your lessons are kept.", keywords: ["সাধারণ", "account", "profile"] });
registerCategory({ id: "appearance", order: 20, icon: "palette", label: "Appearance", description: "Theme, colour and the size of buttons.", keywords: ["look", "চেহারা", "রং", "display"] });
registerCategory({ id: "pen", order: 30, icon: "pen", label: "Pen & ink", description: "How writing feels, and ink that tidies itself.", keywords: ["কলম", "লেখা", "stylus", "ink"] });
registerCategory({ id: "touch", order: 40, icon: "hand", label: "Touch & gestures", description: "Fingers, the palm eraser and swipes on the board.", keywords: ["স্পর্শ", "অঙ্গভঙ্গি", "finger", "smartboard"] });
registerCategory({ id: "modes", order: 50, icon: "monitor", label: "Teaching modes", description: "What Prepare, Teach and Student view show, and the Smart Dock.", keywords: ["মোড", "dock", "toolbar", "ডক"] });
registerCategory({ id: "ai", order: 60, icon: "smartink", label: "AI & agents", description: "Connect your own AI, and turn handwriting into text.", keywords: ["এআই", "Claude", "assistant", "agent"] });
registerCategory({ id: "files", order: 70, icon: "export", label: "Files & export", description: "PDF export defaults, quiz answers and new lessons.", keywords: ["ফাইল", "pdf", "print", "download"] });
registerCategory({ id: "keyboard", order: 80, icon: "keyboard", label: "Keyboard shortcuts", description: "Keys for tools and slides. Clickers send the arrow keys.", keywords: ["কিবোর্ড", "shortcut", "hotkey", "clicker", "keys"] });
registerCategory({ id: "about", order: 90, icon: "info", label: "About", description: "Version, licences and troubleshooting.", keywords: ["সম্পর্কে", "version", "license", "help"] });
