// Top bar: back to the Library, and "Save a copy" for received lessons
// (the student viewer `?view`, or a lesson in "Shared with me").

import { Show } from "solid-js";
import { LIcon } from "./LibraryIcon";
import { lessonOpened, studentEntry } from "../state";
import { SHARED_DIR, isInside } from "../library/paths";
import { current, openLesson, openLibrary, received } from "../library/session";
import { saveCopy } from "../library/actions";
import { shared } from "../cloud/viewer";

export function LibraryTopButton() {
  // A Rupok Cloud link has its own "Make a copy" (SharedLessonBar).
  const showCopy = () => !shared() && (studentEntry ? lessonOpened() : !!current() && isInside(current()!.path, SHARED_DIR));
  const savedTo = () => received()?.savedTo;
  const copy = async () => {
    const path = await saveCopy();
    // In the board, open the copy (like "Make a copy"); the viewer offers a link.
    if (path && !studentEntry) await openLesson(path);
  };
  return (
    <div class="cluster surface lib-top">
      <button class="btn" onClick={() => void openLibrary()} title="Library: all your lessons" aria-label="Library">
        <LIcon name="library" /><span class="label">Library</span>
      </button>
      <Show when={showCopy()}>
        <div class="vsep" />
        <Show
          when={!savedTo()}
          fallback={
            <button class="btn copy" onClick={() => (location.href = `${location.pathname}?open=${encodeURIComponent(savedTo()!)}`)} title="Open your copy in your library">
              <LIcon name="check" /><span class="label">Open my copy</span>
            </button>
          }
        >
          <button class="btn copy" onClick={() => void copy()} title="Save a copy to my library">
            <LIcon name="copy" /><span class="label">Save a copy</span>
          </button>
        </Show>
      </Show>
    </div>
  );
}
