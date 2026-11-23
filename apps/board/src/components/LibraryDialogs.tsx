// Library dialogs: new lesson (templates), folder and course, rename, move,
// confirm, the first-run Teacher/Student choice, and "open in another tab".

import { For, type JSX, Show, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { Icon } from "./Icon";
import { LIcon } from "./LibraryIcon";
import type { LibIconName } from "../library/icons.generated";
import { COPIES_DIR, basename, isInside, split, stem } from "../library/paths";
import { TEMPLATES, type TemplateId } from "../library/templates";
import type { FileLibrary } from "../library/store";

export function LibDialog(props: { icon: LibIconName; title: string; sub?: JSX.Element; width?: number; onClose: () => void; children: JSX.Element }) {
  let el!: HTMLDivElement;
  onMount(() => {
    // Escape closes the dialog even when focus has wandered outside it.
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented && el.isConnected && !el.contains(e.target as Node)) props.onClose();
    };
    window.addEventListener("keydown", esc);
    onCleanup(() => window.removeEventListener("keydown", esc));
    const first = el.querySelector<HTMLElement>("[data-autofocus]") ?? el.querySelector<HTMLElement>("button.solid");
    queueMicrotask(() => {
      first?.focus();
      if (first instanceof HTMLInputElement) first.select();
    });
  });
  return (
    <div
      class="scrim"
      onPointerDown={(e) => e.target === e.currentTarget && props.onClose()}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          props.onClose();
        }
      }}
    >
      <div class="dialog lib-dialog" ref={el} role="dialog" aria-modal="true" aria-label={props.title} style={{ width: props.width ? `min(${props.width}px, 100%)` : undefined }}>
        <div class="dialog-head">
          <div class="badge"><LIcon name={props.icon} size={24} active /></div>
          <div style={{ flex: 1, "min-width": 0 }}>
            <h2>{props.title}</h2>
            <Show when={props.sub}><p class="sub">{props.sub}</p></Show>
          </div>
          <button class="btn icon" onClick={props.onClose} aria-label="Close"><Icon name="close" /></button>
        </div>
        {props.children}
      </div>
    </div>
  );
}

/** "In: ICT › Grade 9 ICT" for a folder path. */
export function WhereLine(props: { dir: string; root: string }) {
  const parts = () => split(props.dir);
  return (
    <div class="lib-where-line">
      <LIcon name="folder" size={16} />
      <span>{props.root}</span>
      <For each={parts()}>{(p) => <><LIcon name="chevron" size={16} /><span>{p}</span></>}</For>
    </div>
  );
}

export function NewLessonDialog(props: { dir: string; rootLabel: string; onClose: () => void; onCreate: (name: string, t: TemplateId) => void }) {
  const [name, setName] = createSignal("Untitled lesson");
  const [tpl, setTpl] = createSignal<TemplateId>("blank");
  const create = () => props.onCreate(name().trim() || "Untitled lesson", tpl());
  return (
    <LibDialog icon="lesson" title="New lesson" sub="Pick a page style. You can change pages later." width={640} onClose={props.onClose}>
      <WhereLine dir={props.dir} root={props.rootLabel} />
      <div class="field">
        <label for="lib-new-name">Name</label>
        <input id="lib-new-name" class="input" data-autofocus value={name()} onInput={(e) => setName(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && create()} />
      </div>
      <div class="lib-templates" role="radiogroup" aria-label="Template">
        <For each={TEMPLATES}>
          {(t) => (
            <button class="lib-tpl" classList={{ on: tpl() === t.id }} role="radio" aria-checked={tpl() === t.id} data-template={t.id} onClick={() => setTpl(t.id)} onDblClick={create}>
              <div class={`pv ${t.id}`}>{t.id === "slides" ? <><i /><i /><i /></> : null}</div>
              <span class="tl">{t.label}</span>
              <span class="th">{t.hint}</span>
            </button>
          )}
        </For>
      </div>
      <div class="actions">
        <div class="grow" />
        <button class="btn" onClick={props.onClose}>Cancel</button>
        <button class="btn solid" onClick={create}><Icon name="plus" /><span class="label">Create lesson</span></button>
      </div>
    </LibDialog>
  );
}

export function NameDialog(props: {
  icon: LibIconName; title: string; label: string; initial: string; action: string; sub?: string; onClose: () => void; onSubmit: (name: string) => void;
}) {
  const [name, setName] = createSignal(props.initial);
  const submit = () => name().trim() && props.onSubmit(name().trim());
  return (
    <LibDialog icon={props.icon} title={props.title} sub={props.sub} width={480} onClose={props.onClose}>
      <div class="field">
        <label for="lib-name">{props.label}</label>
        <input id="lib-name" class="input" data-autofocus value={name()} onInput={(e) => setName(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
      </div>
      <div class="actions">
        <div class="grow" />
        <button class="btn" onClick={props.onClose}>Cancel</button>
        <button class="btn solid" disabled={!name().trim()} onClick={submit}>{props.action}</button>
      </div>
    </LibDialog>
  );
}

export function NewCourseDialog(props: { subjects: string[]; initialSubject: string; onClose: () => void; onCreate: (subject: string, course: string, chapters: string[]) => void }) {
  const [subject, setSubject] = createSignal(props.initialSubject);
  const [course, setCourse] = createSignal("");
  const [chapters, setChapters] = createSignal("");
  const ok = () => subject().trim() && course().trim();
  const create = () => ok() && props.onCreate(subject().trim(), course().trim(), chapters().split("\n"));
  return (
    <LibDialog
      icon="course"
      title="New course"
      sub="A course is a folder of chapters inside a subject, for example ICT › Grade 9 ICT › 01 Number systems."
      width={560}
      onClose={props.onClose}
    >
      <div class="row">
        <div class="field">
          <label for="lib-subject">Subject</label>
          <input id="lib-subject" class="input" list="lib-subjects" data-autofocus placeholder="e.g. ICT" value={subject()} onInput={(e) => setSubject(e.currentTarget.value)} />
          <datalist id="lib-subjects"><For each={props.subjects}>{(s) => <option value={s} />}</For></datalist>
        </div>
        <div class="field">
          <label for="lib-course">Course or class</label>
          <input id="lib-course" class="input" placeholder="e.g. Grade 9 ICT" value={course()} onInput={(e) => setCourse(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && create()} />
        </div>
      </div>
      <div class="field">
        <label for="lib-chapters">Chapters (optional, one per line)</label>
        <textarea id="lib-chapters" style={{ height: "128px", "font-family": "var(--font)", "font-size": "14px" }} placeholder={"Number systems\nNetworks\nProgramming"} value={chapters()} onInput={(e) => setChapters(e.currentTarget.value)} />
        <span class="hint">Chapters are numbered so they stay in teaching order: 01, 02, 03…</span>
      </div>
      <div class="actions">
        <div class="grow" />
        <button class="btn" onClick={props.onClose}>Cancel</button>
        <button class="btn solid" disabled={!ok()} onClick={create}><LIcon name="course" /><span class="label">Create course</span></button>
      </div>
    </LibDialog>
  );
}

/** Pick a destination folder. `moving` are the paths being moved (they and their insides are not valid targets). */
export function MoveDialog(props: { store: FileLibrary; moving: string[]; onClose: () => void; onMove: (dir: string) => void }) {
  const [target, setTarget] = createSignal<string | null>(null);
  const [folders] = createResource(async () => (await props.store.walk("")).filter((e) => e.kind === "folder").map((e) => e.path).sort((a, b) => a.localeCompare(b, undefined, { numeric: true })));
  const invalid = (dir: string) => props.moving.some((m) => isInside(dir, m));
  const label = () => (props.moving.length === 1 ? `“${stem(basename(props.moving[0]))}”` : `${props.moving.length} items`);
  return (
    <LibDialog icon="move" title={`Move ${label()}`} sub="Choose a folder." width={520} onClose={props.onClose}>
      <div class="lib-tree" role="listbox" aria-label="Folders">
        <button classList={{ on: target() === "" }} onClick={() => setTarget("")} role="option" aria-selected={target() === ""}>
          <LIcon name="mine" size={20} /><span>My lessons</span>
        </button>
        <For each={folders() ?? []}>
          {(p) => (
            <button
              classList={{ on: target() === p }}
              disabled={invalid(p)}
              style={{ "padding-left": `${12 + split(p).length * 20}px` }}
              onClick={() => setTarget(p)}
              onDblClick={() => !invalid(p) && props.onMove(p)}
              role="option"
              aria-selected={target() === p}
              data-folder={p}
            >
              <LIcon name={p === COPIES_DIR ? "copies" : "folder"} size={20} /><span>{basename(p)}</span>
            </button>
          )}
        </For>
      </div>
      <div class="actions">
        <div class="grow" />
        <button class="btn" onClick={props.onClose}>Cancel</button>
        <button class="btn solid" disabled={target() === null} onClick={() => target() !== null && props.onMove(target()!)}>Move here</button>
      </div>
    </LibDialog>
  );
}

export function ConfirmDialog(props: { icon: LibIconName; title: string; body: JSX.Element; action: string; danger?: boolean; onClose: () => void; onConfirm: () => void }) {
  return (
    <LibDialog icon={props.icon} title={props.title} width={480} onClose={props.onClose}>
      <p style={{ margin: "0 0 20px", color: "var(--text-2)", font: "450 14px/20px var(--font)" }}>{props.body}</p>
      <div class="actions">
        <div class="grow" />
        <button class="btn" onClick={props.onClose}>Cancel</button>
        <button class="btn solid" classList={{ "danger-solid": props.danger }} style={props.danger ? { background: "var(--danger)", color: "#fff" } : undefined} onClick={props.onConfirm}>
          {props.action}
        </button>
      </div>
    </LibDialog>
  );
}

/** First run: a friendly two-button choice. Bangla under each label. */
export function RoleDialog(props: { onChoose: (r: "teacher" | "student") => void }) {
  return (
    <div class="scrim">
      <div class="dialog lib-dialog" role="dialog" aria-modal="true" aria-label="Welcome" style={{ width: "min(600px, 100%)" }}>
        <h2 style={{ margin: "0 0 4px" }}>Welcome to your Library</h2>
        <p class="sub" style={{ margin: "0 0 16px" }}>All your lessons in one place, saved on this device. Who will use it? You can change this later in Settings.</p>
        <div class="lib-roles">
          <button class="lib-role" data-role="teacher" onClick={() => props.onChoose("teacher")}>
            <div class="badge"><LIcon name="teacher" size={28} active /></div>
            <span class="rt">I'm a teacher</span>
            <span class="bn" lang="bn">আমি শিক্ষক</span>
            <span class="rc">Make lessons and organise them by subject, course and chapter.</span>
          </button>
          <button class="lib-role" data-role="student" onClick={() => props.onChoose("student")}>
            <div class="badge"><LIcon name="student" size={28} active /></div>
            <span class="rt">I'm a student</span>
            <span class="bn" lang="bn">আমি শিক্ষার্থী</span>
            <span class="rc">Open lessons from your teacher and keep your own copies.</span>
          </button>
        </div>
      </div>
    </div>
  );
}

export function LockDialog(props: { name: string; answer: (take: boolean) => void }) {
  return (
    <LibDialog icon="lock" title="Open in another tab" width={500} onClose={() => props.answer(false)}>
      <p style={{ margin: "0 0 20px", color: "var(--text-2)", font: "450 14px/20px var(--font)" }}>
        “{props.name}” is already open in another tab. To keep your work safe, a lesson is edited in one tab at a time.
        If you open it here, the other tab stops saving.
      </p>
      <div class="actions">
        <div class="grow" />
        <button class="btn" onClick={() => props.answer(false)}>Cancel</button>
        <button class="btn solid" onClick={() => props.answer(true)}>Open here instead</button>
      </div>
    </LibDialog>
  );
}
