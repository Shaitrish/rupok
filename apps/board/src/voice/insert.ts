// Putting dictated text into a field: <input>, <textarea> or a
// contentEditable element (the board's text boxes). The text goes in at the
// caret (replacing a selection) the way typing would, so undo, Solid's
// onInput handlers and the text box's commit-on-blur all behave as usual.

export type EditableEl = HTMLInputElement | HTMLTextAreaElement | HTMLElement;

const TEXT_INPUTS = new Set(["text", "search", "url", "email", "tel", ""]);

/** True for elements dictation can type into. */
export function isEditable(el: Element | null | undefined): el is EditableEl {
  if (!el || !(el instanceof HTMLElement)) return false;
  if (el instanceof HTMLTextAreaElement) return !el.readOnly && !el.disabled;
  if (el instanceof HTMLInputElement) return TEXT_INPUTS.has(el.type) && !el.readOnly && !el.disabled;
  return el.isContentEditable;
}

/** The editable element that has the focus (or contains it), if any. */
export function focusedEditable(): EditableEl | null {
  let el: Element | null = document.activeElement;
  // Inside shadow roots.
  while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
  if (!isEditable(el)) return null;
  // The root of a contentEditable region, not a child node.
  if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) {
    let root: HTMLElement = el;
    while (root.parentElement?.isContentEditable) root = root.parentElement;
    return root;
  }
  return el;
}

const isField = (el: EditableEl): el is HTMLInputElement | HTMLTextAreaElement =>
  el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;

/** Text left and right of the caret, for spacing and capitals. */
export function caretContext(el: EditableEl): { before: string; after: string } {
  if (isField(el)) {
    const s = el.selectionStart ?? el.value.length;
    const e = el.selectionEnd ?? s;
    return { before: el.value.slice(0, s), after: el.value.slice(e) };
  }
  const sel = getSelection();
  if (!sel || !sel.rangeCount || !el.contains(sel.anchorNode)) return { before: el.innerText, after: "" };
  const r = sel.getRangeAt(0);
  const pre = document.createRange();
  pre.selectNodeContents(el);
  pre.setEnd(r.startContainer, r.startOffset);
  const post = document.createRange();
  post.selectNodeContents(el);
  post.setStart(r.endContainer, r.endOffset);
  const before = pre.toString();
  const after = post.toString();
  // The board's new text box starts as "Text", all selected: replacing it.
  return { before, after };
}

/** Focus the element and make sure the caret is inside it (at the end when
 * it was elsewhere). */
export function focusForDictation(el: EditableEl) {
  if (document.activeElement !== el && !(el.contains(document.activeElement))) el.focus({ preventScroll: true });
  if (isField(el)) return;
  const sel = getSelection();
  if (sel && (!sel.rangeCount || !el.contains(sel.anchorNode))) {
    const r = document.createRange();
    r.selectNodeContents(el);
    r.collapse(false);
    sel.removeAllRanges();
    sel.addRange(r);
  }
}

/** Insert text at the caret as if typed. Works without focus for inputs
 * (appends at the caret position they remember). */
export function insertText(el: EditableEl, text: string) {
  if (!text) return;
  const focused = document.activeElement === el || el.contains(document.activeElement);
  if (focused) {
    if (!isField(el)) focusForDictation(el);
    // execCommand keeps the browser's undo stack and fires `input`.
    const before = isField(el) ? el.value : el.innerText;
    let ok = false;
    try {
      ok = document.execCommand("insertText", false, text);
    } catch { /* not supported */ }
    if (ok && (isField(el) ? el.value : el.innerText) !== before) return;
  }
  if (isField(el)) {
    const s = el.selectionStart ?? el.value.length;
    const e = el.selectionEnd ?? s;
    el.setRangeText(text, s, e, "end");
    el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
    return;
  }
  const sel = getSelection();
  if (!sel || !sel.rangeCount || !el.contains(sel.anchorNode)) return;
  const r = sel.getRangeAt(0);
  r.deleteContents();
  const node = document.createTextNode(text);
  r.insertNode(node);
  r.setStartAfter(node);
  r.collapse(true);
  sel.removeAllRanges();
  sel.addRange(r);
  el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
}

/** After dictation, let fields that save on `change` (the lesson title) save. */
export function commitField(el: EditableEl, valueAtStart: string) {
  if (isField(el) && el.value !== valueAtStart) el.dispatchEvent(new Event("change", { bubbles: true }));
}

export const fieldValue = (el: EditableEl) => (isField(el) ? el.value : el.innerText);
