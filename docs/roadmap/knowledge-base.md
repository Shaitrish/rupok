# Chapter knowledge base

`Status: 📋 planned · Phase 2 · Difficulty: advanced`

Let a teacher attach the syllabus, textbook pages and notes to a chapter, so their AI agent builds lessons from the material the class actually uses, locally and with citations.

## Goal

Agents produce generic content unless they are grounded. Give every chapter a
local, private set of sources and an MCP interface to search and read them, so
"make tomorrow's lesson on subnetting" follows the board's syllabus, the
class textbook's wording and the teacher's own notes.

## User story

- *As a teacher*, I drag the textbook chapter PDF, the syllabus page and my
  notes into "Chapter 2: Networks"; Rupok shows them as sources.
- *As a teacher*, I ask my agent for a lesson on IP addressing; it searches the
  chapter sources, quotes the textbook definition, and the slides say which
  page each fact came from.
- *As a teacher*, I know the files never leave my computer unless my agent
  sends excerpts to its own model.
- *As a teacher*, I remove an outdated source and the agent stops using it.

## Scope

- Sources per chapter: PDF, DOCX, Markdown/plain text, images (OCR later),
  web pages saved as HTML.
- Text extraction with page/section anchors; chunking; a local full-text index.
- MCP tools to list, search and read sources with citations.
- UI to add/remove sources and see extraction status.
- Citation metadata on generated content.

## Non-goals

- A hosted vector database or any cloud upload by rupok.
- Embedding models shipped inside Rupok (agents can bring their own via the
  read tools; optional local embeddings are an open question).
- Copyright clearance: the teacher owns the decision about their materials.

## Design

### Storage (extends `spec/library.md`, requires an RFC)

Inside a chapter folder of the [course library](./course-library.md):

```text
<Chapter>/kb/
  sources/<blake3>.<ext>          original files, content-addressed (like assets/)
  sources.json                    list: id, title, file, kind, added_ms, enabled
  text/<blake3>.jsonl             extracted chunks (one JSON object per line)
  index/                          full-text index (cache, rebuildable)
```

Chunk record:

```json
{ "id": "a1b2…:p12:c3", "source": "a1b2…", "page": 12, "heading": "4.2 IP addresses",
  "text": "An IP address is …", "chars": [1830, 2710] }
```

`kb/` layout, `sources.json` and the chunk format go into `spec/library.md`;
this **requires an RFC**. `index/` is explicitly a cache and not specified.

### Crate

```text
crates/rupok-kb/                       (new, AGPL-3.0-or-later, native only)
  src/lib.rs        KnowledgeBase::open(chapter_dir), add_source, remove_source
  src/extract.rs    pdf (via rupok-pdf text API), docx, md/txt, html → chunks
  src/chunk.rs      heading-aware chunking, ~800 chars with 100 overlap
  src/index.rs      BM25 index (tantivy, or a small in-house inverted index)
  src/cite.rs       citation strings: "<title>, p. 12"
```

```rust
pub struct Hit { pub chunk_id: String, pub source: String, pub title: String, pub page: Option<u32>, pub score: f32, pub snippet: String }
impl KnowledgeBase {
    pub fn add_source(&mut self, bytes: &[u8], file_name: &str, now_ms: f64) -> Result<SourceId>;
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>>;
    pub fn read(&self, chunk_id: &str, context_chunks: usize) -> Result<String>;
}
```

- PDF text extraction shares the PDFium binding with
  [pdf-import-native](./pdf-import-native.md) (`rupok-pdf` gains
  `page_text(k) -> String`).
- Multilingual: tokenise with Unicode word boundaries; Bangla must work
  (no English-only stemming by default). See [i18n](./i18n.md).
- `now_ms` is passed in; no clock reads.

### MCP tools (`crates/rupok-cli`)

| Tool | Returns |
|---|---|
| `kb_list_sources { chapter }` | id, title, kind, pages, enabled |
| `kb_search { chapter, query, limit? }` | hits with snippet + citation |
| `kb_read { chapter, chunk_id, context? }` | full chunk text (+ neighbours) with citation |
| `kb_add_source { chapter, path }` | new source id (path checked by `workspace.rs`) |

`read_object_spec` / server instructions (`mcp.rs::instructions`) gain a short
paragraph: "When a chapter has sources, search them first and cite them."

### Citations in lessons

- Text items: the agent appends a small "Source: …" text item (no format
  change).
- Smart Objects: propose an optional manifest field
  `sources: [{ "title": "...", "page": 12 }]` in
  `spec/smart-object.md#manifest` (**requires an RFC**). The UI shows a
  "Sources" chip on the object.

### UI

- Chapter view: "Sources" tab with drop zone, list, status (extracting,
  indexed, failed), enable/disable toggle.
- Source preview opens the original at the cited page.

## Implementation tasks

1. **RFC**: `kb/` layout, `sources.json`, chunk JSONL in `spec/library.md`.
   *(intermediate)*
2. **`rupok-kb` skeleton** with Markdown/plain-text extraction and chunking;
   tests with fixtures. *(intermediate)*
3. **BM25 search** with Unicode tokenisation; Bangla and English fixtures.
   *(advanced)*
4. **PDF text** via `rupok-pdf::page_text`, page anchors in chunks. *(intermediate)*
5. **DOCX extraction** (headings preserved). *(intermediate)*
6. **HTML extraction** (readability-style main-content extraction). *(intermediate)*
7. **MCP tools** `kb_list_sources`, `kb_search`, `kb_read`, `kb_add_source`
   with workspace checks. *(intermediate)*
8. **Sources UI** in the chapter view. *(intermediate)*
9. **RFC + implementation**: Smart Object `sources` manifest field and UI
   chip. *(intermediate)*
10. **Skill update**: teach agents (in `skills/`) to search and cite.
    *(good-first-issue)*

## Acceptance criteria and tests

- `cargo test -p rupok-kb`: adding a 3-page PDF fixture yields chunks with the
  right page numbers; searching a phrase from page 2 returns a page-2 chunk
  first.
- Bangla query on a Bangla fixture returns the expected chunk (no
  ASCII-only tokenisation).
- Removing a source removes its chunks from search results.
- Deleting `kb/index/` and reopening rebuilds the index with identical results.
- MCP `kb_read` output includes a citation string; paths outside the workspace
  are refused.
- No network access during any KB operation (test with networking disabled in
  CI sandbox).

## Performance budget

| Metric | Budget |
|---|---|
| Index a 300-page textbook PDF | ≤ 60 s on a 4-core laptop, background |
| `kb_search` latency, 5,000 chunks | ≤ 50 ms |
| Disk overhead | ≤ 2× the extracted text size (index + chunks) |

## Open questions

- tantivy (fast, larger binary) vs a ~500-line in-house BM25 (small, enough for
  a chapter)? Measure binary size impact.
- Optional local embeddings (e.g. a small multilingual model via ONNX) for
  semantic search: worth the download size?
- Should sources be shareable between chapters (course-level `kb/`)?
- OCR for scanned textbooks: reuse [handwriting-recognition](./handwriting-recognition.md)'s
  recognition stack or Tesseract?

## References

- `spec/smart-object.md` — `#manifest`
- `crates/rupok-cli/src/workspace.rs` — path sandbox; `mcp.rs` — tool router
- Related: [course-library](./course-library.md), [pdf-import-native](./pdf-import-native.md),
  [acp-agent-panel](./acp-agent-panel.md), [i18n](./i18n.md)
