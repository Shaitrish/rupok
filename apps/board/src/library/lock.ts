// One tab edits a lesson at a time. Tabs talk over a BroadcastChannel:
// before opening a lesson a tab asks "who has it?"; the holder answers. If
// the teacher opens it here anyway, the other tab is told to let go and
// stops saving (it shows a polite notice instead of overwriting).

const CHANNEL = "rupok-library-lock";
const ANSWER_MS = 250;

const tab = Math.random().toString(36).slice(2, 10);
let held: string | null = null;
let ch: BroadcastChannel | null = null;
let onLost: ((key: string) => void) | null = null;
const waiting = new Map<string, () => void>();

function channel(): BroadcastChannel | null {
  if (ch) return ch;
  try {
    ch = new BroadcastChannel(CHANNEL);
  } catch {
    return null;
  }
  ch.onmessage = (e) => {
    const m = e.data ?? {};
    if (m.tab === tab) return;
    if (m.type === "who" && m.key === held) ch!.postMessage({ type: "mine", key: m.key, req: m.req, tab });
    else if (m.type === "mine") waiting.get(m.req)?.();
    else if (m.type === "take" && m.key === held) {
      held = null;
      onLost?.(m.key);
    }
  };
  return ch;
}

/** Called when another tab takes the lesson this tab holds. */
export function onLockLost(cb: (key: string) => void) {
  onLost = cb;
  channel();
}

/** True when another tab has `key` open. */
export function heldElsewhere(key: string): Promise<boolean> {
  const c = channel();
  if (!c) return Promise.resolve(false);
  const req = Math.random().toString(36).slice(2);
  return new Promise((resolve) => {
    const t = setTimeout(() => {
      waiting.delete(req);
      resolve(false);
    }, ANSWER_MS);
    waiting.set(req, () => {
      clearTimeout(t);
      waiting.delete(req);
      resolve(true);
    });
    c.postMessage({ type: "who", key, req, tab });
  });
}

/** Hold `key` in this tab (and ask any other holder to let go). */
export function acquire(key: string, takeOver = false) {
  held = key;
  if (takeOver) channel()?.postMessage({ type: "take", key, tab });
  else channel();
}

export function release() {
  held = null;
}

export const heldKey = () => held;
