# Rich media for class

Status: 🚧 first version shipped on `feat/media`. Format key **proposed,
pending RFC**.

Teachers asked: "Can they play a YouTube video in class, just by the link?"
This plan covers YouTube by link, video and audio files, 3D models and a few
trusted education sites, and lists what could come next.

## What works

| Kind | How it gets in | Where it lives | Offline |
|---|---|---|---|
| YouTube | paste a link on the board; Insert ▾ → Video / YouTube…; Smart Object library → Media; `add_youtube` / `add_media` | the link in the `media` map, plus a poster image | poster with "Online video, needs internet" |
| Video, audio file (mp4, webm, mp3, m4a, ogg, wav) | drop or pick a file; Insert ▾ → Audio…; `add_media` with `file` | `assets/<blake3>.<ext>` in the `.rpk` | plays |
| 3D model (glTF 2.0 `.glb`, or `.gltf` with everything embedded) | drop or pick a file; Insert ▾ → 3D model (GLB)…; `add_3d_model` | a Smart Object bundle with the viewer and the model inside | plays (three.js from `rupoklib://`) |
| Trusted web embed (PhET, GeoGebra, Desmos, published Google Slides/Docs/Sheets/Forms, Wikipedia, Khan Academy) | paste a link; Insert ▾ → Web embed… | the link plus a poster | poster |

Each media item moves and resizes like an image. A control pill sits in its
bottom-left corner: play/pause, from the start, mute, the time, open the link
in the browser, and while preparing, replace and settings (title, start and
end, mute, loop, autoplay). In Teach the pill is bigger (56–72 px targets);
Space, K, "." (a clicker's blank key) and the media key play and pause the
focused video. The pill is above the ink layer, but the video itself is
under it like a Smart Object: the pen always draws on top and nothing steals
pen input unless the teacher picks *Use objects* (I), which gives the player
its own controls. Students can play media in the Student view; nothing they
do is saved.

YouTube and web embeds load only when played (click to load): a page full of
videos contacts nobody and costs nothing until Play. Autoplay starts a video
when its page is shown in Teach or the Student view, never while preparing.

PDF export shows the poster with a caption card: the kind, the title, and for
links the URL and a QR code (`apps/board/src/media/qr.ts`, checked against a
reference encoder). The readable summary (`content.md`, `content.json`)
lists every media item with its link or file.

## Format

A media item is an `image` item with a `media` map; `asset` is its poster
(spec/rupok-format.md, "Media items"). This is additive: a reader that does
not know `media` shows the poster, so lessons with videos open in every
earlier Rupok as pictures, in the right place and size. A new item kind would
have been cleaner on paper, but older readers would have hidden it (or, before
v0.2, failed on the page).

3D models need nothing new: the viewer is an ordinary Smart Object bundle
(`generator: "rupok-3d-viewer"`), so save, copy and paste, the live AI tools
and PDF export (the object snapshot) work as for any object.

## Security decisions

1. **Embeds are not Smart Objects.** Smart Objects are untrusted code (an AI
   wrote them, or a stranger shared a lesson) and run in an opaque-origin
   sandbox with no network. YouTube needs the network and its own origin, so
   it is a built-in embed type with a strict allowlist, not a Smart Object
   with `permissions.network`. The Smart Object sandbox and CSP are
   unchanged.
2. **One allowlist, in Rust.** `crates/rupok-format/src/media.rs` parses a
   link, accepts only the listed sites and paths, and rebuilds the iframe URL
   from the parsed pieces (an 11-character YouTube id, a PhET sim slug, …).
   The board calls it through wasm (`Board.media_embed_url`) every time it
   loads a frame, so a hand-edited `.rpk` with
   `src: "https://evil.example"` or `javascript:` never becomes a frame. Look-
   alike hosts (`youtube.com.evil.example`), user info, odd ports and other
   schemes are refused; tests cover them.
3. **The frames.** YouTube plays from `https://www.youtube-nocookie.com`
   (privacy-enhanced) with `sandbox="allow-scripts allow-same-origin
   allow-presentation"`; web embeds also get `allow-forms` (Google Forms).
   `allow-same-origin` here keeps *the site's* origin for its own storage: the
   frame is always a cross-origin `https` page, never `srcdoc` and never the
   board's origin, so it cannot reach the board. No popups, top navigation,
   downloads or modals. `referrerpolicy="strict-origin-when-cross-origin"`
   (YouTube refuses to play without a referrer).
4. **Talking to the player.** Play and pause use YouTube's iframe
   `postMessage` protocol (`enablejsapi=1`); no YouTube script is loaded into
   the board. Replies are accepted only from `event.source` of that frame
   with origin `https://www.youtube-nocookie.com`.
5. **Desktop CSP.** `frame-src` lists only the trusted origins; `img-src` and
   `connect-src` add `i.ytimg.com` (thumbnails), YouTube oEmbed (the title)
   and `phet.colorado.edu` (sim pictures) so posters can be made.
6. **Links off the list** are never embedded. Pasting one opens the Web
   embed dialog, which says so and offers *Open in browser* (only `http(s)`
   links, normalised).
7. **Files** are sniffed by content (`sniff_media`, `sniff_model`), never
   trusted by name. Clips are capped at 200 MB (a warning above 50 MB: the
   lesson file grows by that much); models at 18 MB (they are inlined as
   base64 in the viewer, which must stay under the 25 MiB bundle limit).
   A `.gltf` that points at other files is refused with "export as one .glb".
8. **The 3D viewer** decodes embedded textures with `<img>` (blob URLs are
   allowed by `img-src`) instead of `fetch()`, because the object CSP has
   `connect-src 'none'` and must keep it.

## Performance

Nothing here touches the pen path. Media items are DOM nodes under the ink
canvas; YouTube and web frames exist only while playing; video files load
metadata only. `bench/latency/pen-latency.mjs --check` passes on the branch.

## Known limits

- **Desktop YouTube:** the desktop app's origin is `tauri://localhost` (or
  `http://tauri.localhost` on Windows); YouTube may refuse some videos in an
  embed without a web origin. Open in browser is always there.
- Embeds need the internet; their posters are taken when the link is added
  (the real YouTube thumbnail, the PhET picture, else a plain card). An item
  an AI added through MCP gets a plain SVG card; online, the board shows the
  YouTube thumbnail instead.
- Some sites change their framing rules; GeoGebra and Khan Academy pages are
  heavy in a small frame. The poster and Open in browser are the fallback.
- No trimming or compression of video files: trim before adding.
- The 3D viewer supports glTF 2.0 without Draco, Meshopt or KTX2 compression.
- Saved 3D views are clicker steps declared at run time (the viewer calls
  `rupok.steps.setCount`); the step badge under the object does not show
  them yet.

## More possibilities (not built)

Ideas that fit the same pieces (built-in Smart Objects, media items, the
allowlist). Each wants its own small plan before it is built.

- **Classroom timer and stopwatch**: a big countdown with a sound, as a
  built-in object; the clicker starts and pauses it.
- **Random name picker**: paste the class list once, spin to pick who
  answers; remembers who was picked (object state).
- **Dice and spinner**: 1–6 dice, a custom spinner with the teacher's own
  labels, for games and probability lessons.
- **Molecule viewer for PDB and MOL files**: ball-and-stick and cartoon
  views from a file, offline, built on the 3D viewer.
- **Maps**: an offline world map with countries and capitals to tap; an
  online OpenStreetMap embed through the allowlist.
- **The teacher's voice**: record a short audio explanation onto a page
  (MediaRecorder → an `audio` media item), for students who missed class.
- **Replay with the voice**: the class recording with its audio, see
  [replay-audio](replay-audio.md).
- **Image galleries and slideshows**: several pictures in one item, stepped
  with the clicker.
- **Video bookmarks**: chapter points inside a video as build steps ("pause
  at 1:20 and ask a question").
- **Subtitles**: show YouTube captions on by default (`cc_load_policy=1`),
  and `.vtt` files for local videos.
- **More trusted sites**, by request: Scratch projects, CK-12, Wolfram
  demonstrations, NASA Eyes, Vimeo, Bangladesh's own Shikkhok Batayon and
  Muktopaath. Each needs its link forms and a check that it allows framing.
- **Sound effects board**: a few classroom sounds (correct, wrong, applause)
  on a strip of buttons.
- **Poll from a video**: pause at a point and open a live quiz.
