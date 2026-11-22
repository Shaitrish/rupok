# Live quizzes

`Status: 📋 planned · Phase 3 · Difficulty: intermediate`

Students answer a quiz Smart Object on their own devices during class, and the teacher sees results appear live on the board.

## Goal

Turn any quiz Smart Object into a live class activity. Students in a
[live broadcast](./live-broadcast.md) session run the object in `student`
mode on their phones; every `rupok.quiz.submit()` call reaches the teacher's
app, which shows a live results panel (answer distribution per question,
how many answered) and can reveal the results on the board. A built-in quick
poll (A/B/C/D, true/false) covers the case where there is no quiz object.

## User story

- *As a teacher*, I put my AI-made networking quiz on the slide, press
  **Start live quiz**, and watch the bars fill as 40 students answer on their
  phones. I see that 70 % picked "Switch" for question 2, so I stop and
  re-explain routers.
- *As a teacher*, I ask a quick "Which is bigger, 0.5 or 0.45? A / B" poll
  without preparing anything.
- *As a teacher*, after class I export the results as a CSV for my records.
- *As a teacher*, I can run it anonymously so shy students answer honestly.

## Scope

- Collect `quiz` bridge messages from student devices through the relay.
- Teacher results panel: per-question distribution, answered count, correct
  rate (when objects report `correct`).
- Reveal results on the board (a results card the teacher can annotate).
- Quick poll tool (no Smart Object needed).
- Anonymous or named mode (display name typed by the student at join).
- CSV export of a session's answers.

## Non-goals

- Graded assessment, cheating prevention, timed exams.
- Rosters and LMS gradebook sync (Phase 3 "rosters and data export").
- Students' free-form ink answers (that is submissions over
  [student layers](./student-layers.md)).

## Design

### Message flow

```text
student device                         relay                 teacher app
 Smart Object ─quiz {payload}─▶ host ─▶ frame kind 9 "answer" ─▶ results store ─▶ panel / board card
  (mode "student")              (web viewer)   (encrypted)       (per session)
```

- The object side is unchanged: objects call `rupok.quiz.submit({questionId,
  answer, correct?, score?})` (`spec/smart-object.md`, section "Bridge
  protocol"). The SDK already queues calls before `ready` and drops malformed
  payloads.
- The student host (web viewer, `apps/board` in viewer mode) wraps each
  payload with `{session, object_item_id, student}` and sends it as a new
  broadcast frame `9 answer`. **Requires an RFC** amending `spec/broadcast.md`
  (introduced by [live-broadcast](./live-broadcast.md)).
- Answers flow student → relay → teacher only; students never see each other's
  answers. The relay stores nothing.

### Results store (teacher, new)

New module `crates/rupok-wasm/src/quiz.rs` (plain Rust, unit-testable):

```rust
pub struct QuizSession { id: String, started: f64, anonymous: bool, answers: Vec<Answer> }
pub struct Answer { student: String, object: ItemId, question: String, answer: serde_json::Value,
                    correct: Option<bool>, at: f64 }

impl QuizSession {
    pub fn record(&mut self, a: Answer);                     // latest answer per (student, object, question) wins
    pub fn summary(&self, object: &str) -> Vec<QuestionSummary>; // counts per distinct answer, answered, correct rate
    pub fn to_csv(&self) -> String;
}
```

Board API (new): `quiz_start(anonymous: bool) -> String`,
`quiz_record_json(json: &str)`, `quiz_summary_json(object_id: &str) -> String`,
`quiz_stop()`, `quiz_csv() -> String`.

### Persistence (proposed; requires an RFC)

Results are class records, not lesson content, so they must not go into
`doc.loro` (every student's answer would become part of the lesson history).
Proposed: a new optional package entry `results/<session>.jsonl`, one JSON
object per answer, plus a header line `{session, started, anonymous, lesson}`.
**Requires an RFC** amending `spec/rupok-format.md` sections "Package layout"
and "Versioning and forward compatibility". Additive: v1 readers ignore unknown
entries. Until then, results live in memory and are exported as CSV.

### Showing results to the object (optional, requires an RFC)

Some quiz objects may want to show the class distribution themselves. Proposed
host → object message: `{rupok:1, type:"results", questionId, counts: {answer: n}, answered}`
sent only in `present` mode on the teacher's screen. **Requires an RFC** for
`spec/smart-object.md` "Bridge protocol" and a matching SDK addition
(`rupok.quiz.onResults(cb)`). Phase 1 of this plan does not need it: the
teacher's results panel is native UI.

### Quick poll

A built-in Smart Object shipped with the app (`apps/board/src/builtin/poll.html`,
new) with 2–6 big buttons, using `rupok.quiz.submit`. It goes through the same
path, so there is one code path for results.

### Trust

`correct` is self-reported by the student's copy of the object. That is fine
for formative quizzes; the panel labels it "as reported by the quiz". Hosts
rate-limit answers per student (max 5/s) and cap payload size (4 KB).

## Implementation tasks

1. **RFC**: `answer` broadcast frame; `results/` package entry. (intermediate)
2. `quiz.rs`: `QuizSession::{record, summary, to_csv}` with unit tests (dedupe, latest wins, anonymous mode hides names in CSV). (good-first-issue)
3. Student host: forward `quiz` messages in `student` mode to the live connection with session/object ids. (intermediate)
4. Teacher: receive `answer` frames, feed `quiz_record_json`, live panel component with bars per question. (intermediate)
5. "Reveal on board": render the summary as a static results card (text + image item, or a small Smart Object) the teacher can annotate. (intermediate)
6. Built-in quick poll object + toolbar button. (good-first-issue)
7. CSV export and (after the RFC) `results/` persistence in `Engine::save`. (good-first-issue)
8. Join flow: optional display name; anonymous by default. (good-first-issue)
9. Optional: `results` host → object message + SDK `quiz.onResults` (separate RFC). (intermediate)

## Acceptance criteria and tests

- `cargo test -p rupok-wasm quiz` covers aggregation, deduplication and CSV
  escaping (names with commas, quotes, Bangla text).
- End-to-end test with the relay test harness: 40 simulated students submit
  answers to `examples/objects/quiz-networking.html`; the teacher summary shows
  exactly 40 answers per question.
- Quiz answers never appear in `doc.loro` history (check with
  `Lesson::loro()` after a session).
- In anonymous mode, the CSV and panel contain no names.
- The quick poll works with no network beyond the relay connection.

## Performance budget

- Answer tap on a student phone → bar updates on the teacher screen: p95 ≤ 500 ms.
- Teacher panel update for 200 answers/s burst: no dropped pen frames
  (`rupok-bench` wet frame p99 ≤ 4 ms unaffected; aggregation is off the pen path).

## Open questions

- Should the teacher be able to lock answers ("time's up")? Needs a host →
  student control frame.
- Where should named results go long-term: the lesson package, a separate class
  record file, or only CSV?
- Should objects declare their answer key in the manifest so the teacher's app
  can compute `correct` itself?

## References

- `spec/smart-object.md` — "Bridge protocol" (`quiz` message), "Modes" (`student`), "SDK API"
- `spec/rupok-format.md` — "Package layout", "Versioning and forward compatibility"
- `examples/objects/quiz-networking.html`
- [live-broadcast](./live-broadcast.md), [web-viewer](./web-viewer.md), [student-layers](./student-layers.md)
