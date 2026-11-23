# Rupok vs OpenBoard: two free, open-source whiteboards

> Rupok and OpenBoard are both free, open-source whiteboard software for teachers. Compare desktop vs browser, AI, replay, recording and languages. Facts checked November 2026.

Canonical: https://rupok.app/compare/openboard/ · Language: en · Updated: 2026-11-23

OpenBoard is a mature, free desktop whiteboard maintained in Geneva. Rupok is a newer free whiteboard that runs in the browser, with AI-made lessons and stroke-by-stroke replay. Both are open source, so this is about fit, not price.

## Rupok vs OpenBoard at a glance

*Rupok and OpenBoard compared, as of November 2026*

|  | Rupok | OpenBoard |
|---|---|---|
| Price | ✓ Free, every feature, on every device. No licence keys | ✓ Free, no registration[^1] |
| Open source | ✓ Yes: AGPL-3.0. The `.rpk` format spec is Apache-2.0 | ✓ Yes: GPL-3.0[^3] |
| Runs on | Chrome, Edge or Safari on Windows, macOS, Linux and ChromeOS. Desktop app for Windows, macOS (beta) and Linux; installers not code-signed yet | Windows 10+, macOS 12+ and Debian, plus community Linux packages[^3][^4]. No web, ChromeOS or mobile app |
| Account | ✓ None, for teachers or students | ✓ None[^1] |
| AI | ✓ Your own AI (Claude, Codex or OpenCode): chat with it inside Rupok, let it edit the open board live, or connect it over MCP. It makes live Smart Objects and writes step-by-step solutions in handwriting (Solve this). HTML from any chatbot works too | ✗ None |
| Replay and recording | ✓ Stroke-by-stroke replay of every page, built in. Replay with audio: coming soon | ◐ Podcast mode records the board and your voice as a video[^5] |
| Interactive content | ✓ AI-made simulations, 3D models and quizzes that run live on the slide, plus a gallery of ready-made presets (physics, chemistry, maths, ICT, history, quiz) | ✓ Web widgets and about 25 built-in activities (memory, word games)[^6] |
| PDF and slides | ✓ Import a PDF and write on every page; export slides, or handouts with 2 or 3 slides per page and lines for notes | ✓ Imports and exports PDF[^5]; IWB (Common File Format) import and export[^3] |
| Bangla | ◐ Bangla text and fonts in lessons; Bangla voice typing and settings search. Bangla interface: coming soon | ✗ 35 interface languages, no Bangla translation[^3] |
| Offline | ✓ The desktop app works without internet, including maths recognition (AI features need internet). In the browser, the board runs on your device once it is open | ✓ Installed desktop app[^1] |
| Smartboards and panels | ✓ Any smartboard, interactive panel, projector or laptop with Chrome, Edge or Safari | ✓ Any interactive whiteboard, graphics tablet or mouse[^1] |
| File format | Open `.rpk` (a ZIP package with a published spec) | `.ubz`[^5] |
| Status (Nov 2026) | Actively developed: early alpha, v0.2.0 released 23 Nov 2026 | Actively maintained: version 1.7.7 released 30 March 2026; 1.8 in development[^2] |

## Where OpenBoard is stronger

Being honest about this saves you time.

- **A mature native desktop app** for Windows, macOS and Linux that works fully offline, with roots going back to 2003 (Uniboard, then Open-Sankoré)[^1].
- **Video recording with sound** (Podcast mode)[^5]. Rupok replays strokes; audio is coming.
- **IWB / Common File Format** import and export, for swapping lessons with other whiteboard software[^3].
- **A library of widgets and activities**, and a widget API[^6].
- **Institutional backing:** maintained with the Canton of Geneva’s public education department[^1].

## Where Rupok is stronger

- **Nothing to install.** It runs in Chrome, Edge or Safari, so it also works on Chromebooks, Macs and locked-down school PCs. Prefer an app? The free desktop app runs on Windows, Mac and Linux.
- **AI Smart Objects.** Live simulations, 3D models and quizzes made by Claude, ChatGPT, Codex or OpenCode, with an MCP server for agents.
- **Stroke-by-stroke replay** in a small lesson file, instead of a large video. Students pause, zoom and still use the live objects.
- **GPU ink** that stays smooth with 100,000 strokes.
- **A student viewer** at [rupok.app/open](https://rupok.app/open/): no account, nothing uploaded.
- **Bangla text and fonts** in lessons.

## Which one should you pick?

**Pick OpenBoard** if you want a proven desktop app that works fully offline, records video with sound, or exchanges IWB files.

**Pick Rupok** if you want to teach from any browser, add AI-made interactive lessons, or send students replayable classes.

Both are free, so try both on your board for a week.

## Switching from OpenBoard

1. **Export from OpenBoard.** OpenBoard exports documents as PDF[^5].
2. **Open the board.** Go to [rupok.app/board](https://rupok.app/board/) in Chrome, Edge or Safari, on the panel's PC or your laptop. No install, no account.
3. **Drop the PDF in.** Every page becomes a slide you can write on.
4. **Add the live parts.** Ask your AI for a simulation or quiz and drop it on a slide as a Smart Object ([how](https://rupok.app/ai-whiteboard/)).
5. **Save and share.** Save the lesson as a `.rpk` file and send it to students. They replay it at [rupok.app/open](https://rupok.app/open/).

## Rupok and OpenBoard: questions

### What is a good OpenBoard alternative?

If you want to stay free and open source, Rupok is the closest: same price (free), plus AI Smart Objects and stroke replay, running in the browser instead of as a desktop app.

### Is OpenBoard still maintained?

Yes. Version 1.7.7 came out on 30 March 2026, and 1.8 is in development[^2].

### Can Rupok open .ubz files?

No. Export the document from OpenBoard as a PDF and drop it into Rupok.

### Which licence do they use?

OpenBoard is GPL-3.0[^3]. Rupok is AGPL-3.0, and its `.rpk` file format spec is Apache-2.0, so anyone can build compatible tools.

## Sources

Facts about other products were checked on 23 November 2026. Prices and features change; if you spot something out of date, please [open an issue](https://github.com/Shaitrish/rupok/issues).

[^1]: [OpenBoard](https://openboard.ch/), official website (checked 23 Nov 2026).
[^2]: [OpenBoard releases](https://github.com/OpenBoard-org/OpenBoard/releases), GitHub (1.7.7, 30 Mar 2026).
[^3]: [OpenBoard source code and README](https://github.com/OpenBoard-org/OpenBoard), GitHub (licence, platforms, translations, IWB import/export in the source).
[^4]: [OpenBoard downloads](https://github.com/OpenBoard-org/OpenBoard/wiki/Downloads), GitHub wiki.
[^5]: [OpenBoard 1.6 tutorial](https://openboard.ch/download/Tutoriel_OpenBoard_1.6EN.pdf), official manual, 3 Mar 2022.
[^6]: [OpenBoard documentation](https://openboard.ch/en/documentation), official website.

*OpenBoard is developed by its community and the Canton of Geneva’s education department. Rupok is not affiliated with or endorsed by the OpenBoard project.*
