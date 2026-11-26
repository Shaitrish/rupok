# Testing ink speed on a classroom board

One visit, about 20 minutes. You need the board, its Windows PC and a pen.
What the numbers mean is in [performance.md](performance.md).

## 1. Install

1. Download the Windows build of the pull request (the `rupok-windows`
   artifact of its Desktop run; the link is in the pull request), unzip it,
   and run the `…-setup.exe`. It installs over the Rupok that is there;
   lessons and settings stay.
2. Open Rupok on the board, full screen, with a lesson that has some
   writing on it (or write a few lines first).

## 2. Look at the overlay (2 minutes)

Settings → About → Troubleshooting → **Performance and pen latency
overlay**: on. Write a sentence, as in class, and read the corner:

| Line | Good | Not good |
|---|---|---|
| Frames while writing | `55 of 60 fps` or more, almost all of them `small` | under 45, or mostly `full` |
| Pen → frame | 20 ms or less, p95 under 35 ms | over 30 ms |
| Input | `pen`, 100 Hz or more | `mouse`, or under 60 Hz |
| Canvas | `2880×1620 · Auto (75 %) · wet layer 768×576` on a 4K board | `wet layer off` |
| Renderer | `WebGPU` | red "Software rendering" |

Take a photo of the overlay after writing.

## 3. Run the speed test (5 minutes)

Settings → About → Troubleshooting → **Run board speed test**.

1. The board writes by itself, seven modes of 10 seconds. Do not touch it.
2. "Your turn": write on the board as in class for 10 seconds, four
   times (the last one is the native wet-ink layer). It starts when you
   touch. "Skip the writing part" leaves these out.
3. The board reloads by itself, writes the seven modes again with the
   other renderer, reloads once more and shows the report.
4. The report is saved to `Documents\Rupok\speed-test-<date>.txt`. **Copy**
   and **Save as file** are there too. Your lesson is put back as it was.

## 4. Try each switch by feel (10 minutes)

Settings → About → **Performance lab**. Every switch applies at once, no
restart. Write the same sentence each time, and one long underline. Put
each back before the next.

| # | Switch | Try | What to notice |
|---|---|---|---|
| a | (nothing) | The defaults: Ink sharpness Auto, Wet-ink layer Medium | This is the candidate. Is the ink glued to the pen? |
| b | Wet-ink layer | **Off** | The old way. It should feel clearly worse; if it does not, tell us. |
| c | Wet-ink layer | **Small**, then **Large** | Long strokes and fast scribbles: any stutter? |
| d | Ink sharpness | **100 %**, then **50 %** | 100 %: slower when you pan, zoom or erase? 50 %: is the ink too soft from the back of the room? |
| e | Windows ink trail | **Off** | Does the tip fall behind the pen without it? |
| f | Native wet ink · prototype | **On**, pen or ballpoint style, a solid colour | The overlay says `Native wet ink: drawing` while you write. More glued than (a)? Anything wrong: ink in the wrong place, a jump or flicker when you lift the pen, ink left on screen? If it switches itself off, Settings shows why. |
| g | Flat panels | **On** | Any difference when writing near the dock? |
| h | Renderer (under Troubleshooting) | **WebGL2** (the board reloads) | Only by feel; the speed test has measured both. Put it back on Auto. |

Then write the same sentence in Note3 on the same board, and compare with (a).

## 5. Send back

- The `speed-test-….txt` file (or the pasted report).
- The photo of the overlay from step 2.
- For each of a–h: better, same or worse than the Rupok you had, and
  anything that looked wrong.
- (a) against Note3: same, close, or still behind?
- The board's model if you can see it, and Windows' display scaling
  (Settings → System → Display → Scale).

## If something goes wrong

- The board stays blank after changing the renderer: Settings → About →
  Renderer → WebGL2.
- Ink stays on screen with the native layer on: switch Native wet ink off.
- To undo everything: Settings → About → reset the Performance lab
  switches to Auto, Medium, on, off, off.
