#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Turn a Rupok lesson (.rpk, Rich Presentation Kit) into Markdown.

    python3 read_rpk.py lesson.rpk            # Markdown outline
    python3 read_rpk.py lesson.rpk --json     # structured outline
    python3 read_rpk.py lesson.rpk --list     # entries in the package
    python3 read_rpk.py lesson.rpk --source text|loro

A .rpk is a ZIP archive. Rupok writes `content.md` / `content.json` (readable
outlines) on every save; the authoritative lesson is `doc.loro`, a Loro CRDT
snapshot. With the `loro` package installed (`pip install loro`) this script
reads `doc.loro` itself, so it also works on old packages that have no
outline; otherwise it prints the outline Rupok wrote.

Standard library only, plus optional `loro`. Format: https://rupok.app/rpk/
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import sys
import zipfile

MIMETYPE = "application/vnd.rupok.lesson+zip"
# Never read more than this from one entry (packages come from strangers).
MAX_TEXT_ENTRY = 4 << 20
MAX_DOC = 512 << 20
MAX_TEXT = 1000

SHAPE_NAMES = {
    "rect": "rectangle", "ellipse": "ellipse", "circle": "circle",
    "square": "square", "line": "line", "arrow": "arrow",
    "triangle": "triangle", "diamond": "diamond (flowchart decision)",
    "pentagon": "pentagon", "hexagon": "hexagon", "star": "star",
    "parallelogram": "parallelogram (flowchart input/output)",
    "terminator": "flowchart terminator", "axes": "coordinate axes",
    "numberline": "number line", "angle": "angle arc",
    "right-angle": "right-angle mark", "table": "table",
    "and-gate": "AND gate", "nand-gate": "NAND gate", "or-gate": "OR gate",
    "nor-gate": "NOR gate", "xor-gate": "XOR gate", "xnor-gate": "XNOR gate",
    "not-gate": "NOT gate", "resistor": "resistor", "cell": "cell (battery)",
    "bulb": "bulb", "switch": "switch", "equilibrium": "equilibrium arrows",
    "graph-line": "straight-line graph", "graph-quad": "quadratic graph",
    "graph-sine": "sine graph", "graph-exp": "exponential graph",
    "venn": "Venn diagram", "benzene": "benzene ring",
}


class NotALesson(Exception):
    pass


def _read(z: zipfile.ZipFile, name: str, limit: int) -> bytes | None:
    try:
        info = z.getinfo(name)
    except KeyError:
        return None
    if info.file_size > limit:
        raise NotALesson(f"{name} is too large ({info.file_size} bytes)")
    with z.open(info) as f:
        data = f.read(limit + 1)
    if len(data) > limit:
        raise NotALesson(f"{name} is too large")
    return data


def _json(z: zipfile.ZipFile, name: str):
    data = _read(z, name, MAX_TEXT_ENTRY)
    if data is None:
        return None
    try:
        return json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None


def _cap(s: str, n: int = MAX_TEXT) -> str:
    return s if len(s) <= n else s[:n] + "…"


def _line(s: str) -> str:
    s = " ".join(str(s).split())
    return "\\" + s if s.startswith("#") else s


def _iso(ms) -> str:
    try:
        ms = float(ms)
        if ms <= 0:
            return ""
        t = _dt.datetime.fromtimestamp(ms / 1000, _dt.timezone.utc)
        return t.strftime("%Y-%m-%dT%H:%MZ")
    except (TypeError, ValueError, OverflowError, OSError):
        return ""


def _num(v, default=0.0) -> float:
    return float(v) if isinstance(v, (int, float)) and not isinstance(v, bool) else default


def open_lesson(path: str) -> zipfile.ZipFile:
    try:
        z = zipfile.ZipFile(path)
    except (zipfile.BadZipFile, OSError) as e:
        raise NotALesson(f"{path}: not a .rpk (ZIP) file: {e}") from e
    if _read(z, "mimetype", 64) != MIMETYPE.encode():
        raise NotALesson(f"{path}: not a Rupok lesson (missing or wrong mimetype)")
    return z


# ------------------------------------------------------------ from doc.loro --

def outline_from_loro(z: zipfile.ZipFile) -> dict | None:
    """Build content.json-shaped data from doc.loro; None without `loro`."""
    try:
        import loro  # type: ignore
    except ImportError:
        return None
    data = _read(z, "doc.loro", MAX_DOC)
    if not data:
        raise NotALesson("missing doc.loro")
    doc = loro.LoroDoc()
    doc.import_(data)
    root = doc.get_deep_value()
    meta = root.get("meta") or {}
    manifest = _json(z, "manifest.json") or {}
    bundles = {}
    for name in z.namelist():
        parts = name.split("/")
        if len(parts) == 3 and parts[0] == "objects" and parts[2] == "manifest.json":
            m = _json(z, name)
            if isinstance(m, dict):
                bundles[parts[1]] = m
    # Recognised handwriting only exists in the outline Rupok wrote.
    written = _json(z, "content.json") or {}
    written_pages = written.get("pages") if isinstance(written, dict) else None
    pages = []
    for i, p in enumerate(root.get("pages") or []):
        if not isinstance(p, dict):
            continue
        fw, fh = p.get("frame_w"), p.get("frame_h")
        slide = isinstance(fw, (int, float)) and isinstance(fh, (int, float))
        page = {
            "number": i + 1, "title": _cap(str(p.get("title") or ""), 200),
            "layout": "slide" if slide else "board",
            "background": str(p.get("bg_pattern") or "plain"),
            "text": [], "ink": {"strokes": 0}, "shapes": [], "images": [], "objects": [],
        }
        if slide:
            page["width"], page["height"] = fw, fh
        raw = [(k, it) for k, it in (p.get("items") or {}).items() if isinstance(it, dict)]
        # Shapes by kind in order of first appearance (stacking order: z, id).
        shapes: dict[str, int] = {}
        for _, it in sorted(raw, key=lambda kv: (_num(kv[1].get("z")), kv[0])):
            if it.get("kind") == "stroke" and it.get("shape"):
                shape = str(it["shape"])[:32]
                shapes[shape] = shapes.get(shape, 0) + 1
        # Everything else in reading order: top to bottom, left to right.
        items = sorted((it for _, it in raw), key=lambda it: (_num(it.get("y")), _num(it.get("x"))))
        for it in items:
            kind = it.get("kind")
            if kind == "text" and str(it.get("text") or "").strip():
                t = {"text": _cap(str(it["text"]))}
                if it.get("latex") is True:
                    t["latex"] = True
                page["text"].append(t)
            elif kind == "stroke":
                if it.get("shape"):
                    pass
                elif it.get("tool") == "highlighter":
                    page["ink"]["highlighter"] = page["ink"].get("highlighter", 0) + 1
                else:
                    page["ink"]["strokes"] += 1
            elif kind == "image":
                page["images"].append({
                    "file": f"assets/{it.get('asset', '')}",
                    "width": round(_num(it.get("w"))), "height": round(_num(it.get("h"))),
                })
            elif kind == "object":
                bid = str(it.get("bundle") or "")
                b = bundles.get(bid, {})
                entry = str(b.get("entry") or "index.html")
                o = {
                    "title": _cap(str(it.get("title") or b.get("title") or ""), 200),
                    "bundle": bid, "steps": int(_num(b.get("steps"))),
                }
                if b.get("description"):
                    o["description"] = _cap(str(b["description"]), 300)
                if f"objects/{bid}/{entry}" in z.namelist():
                    o["html"] = f"objects/{bid}/{entry}"
                page["objects"].append(o)
        page["shapes"] = [
            {"kind": k, "name": SHAPE_NAMES.get(k, k), "count": n} for k, n in shapes.items()
        ]
        if isinstance(written_pages, list) and i < len(written_pages):
            rec = (written_pages[i].get("ink") or {}).get("recognized")
            if rec:
                page["ink"]["recognized"] = rec
        pages.append(page)
    return {
        "format": "rupok-lesson-content", "version": 1,
        "about": "Outline read from doc.loro by read_rpk.py.",
        "spec": "https://rupok.app/rpk/",
        "title": str(meta.get("title") or manifest.get("title") or ""),
        "created": _iso(meta.get("created") or manifest.get("created")),
        "modified": _iso(manifest.get("modified")),
        "generator": str(manifest.get("generator") or ""),
        **({"course": manifest["course"]} if isinstance(manifest.get("course"), dict) else {}),
        "page_count": len(pages), "pages": pages, "truncated": False,
    }


# ---------------------------------------------------------------- markdown --

def to_markdown(c: dict) -> str:
    out = [f"# {_line(c.get('title') or 'Untitled lesson')}", ""]
    course = c.get("course") or {}
    for key, label in (("subject", "Subject"), ("course", "Course"), ("chapter", "Chapter")):
        if course.get(key):
            out.append(f"- {label}: {_line(course[key])}")
    out.append(f"- Pages: {c.get('page_count', len(c.get('pages', [])))}")
    for key, label in (("created", "Created"), ("modified", "Modified"), ("generator", "Made with")):
        if c.get(key):
            out.append(f"- {label}: {_line(c[key])}")
    for p in c.get("pages", []):
        head = f"## Page {p['number']}"
        if p.get("title"):
            head += f": {_line(p['title'])}"
        out += ["", head, ""]
        ink = p.get("ink") or {}
        if not any([p.get("text"), ink.get("strokes"), ink.get("highlighter"),
                    p.get("shapes"), p.get("images"), p.get("objects")]):
            out.append("_(empty page)_")
            continue
        for t in p.get("text", []):
            if t.get("latex"):
                out += [f"$${t['text'].strip()}$$", ""]
            else:
                out += [f"> {ln.rstrip()}" for ln in t["text"].splitlines()] + [""]
        if ink.get("strokes"):
            n = ink["strokes"]
            out.append(f"- Handwriting: {n} ink stroke{'s' if n != 1 else ''}")
            for r in ink.get("recognized") or []:
                out.append(f"  - recognised maths (machine reading, may be wrong): ${r['latex']}$")
        if ink.get("highlighter"):
            n = ink["highlighter"]
            out.append(f"- Highlighter: {n} stroke{'s' if n != 1 else ''}")
        if p.get("shapes"):
            names = [s["name"] if s["count"] == 1 else f"{s['count']} × {s['name']}"
                     for s in p["shapes"]]
            out.append(f"- Shapes and diagrams: {', '.join(names)}")
        for img in p.get("images", []):
            out.append(f"- Image: `{img['file']}` ({img['width']} × {img['height']})")
        for o in p.get("objects", []):
            n = o.get("steps") or 0
            inner = f"{n} step{'s' if n != 1 else ''}" if n else "interactive"
            if o.get("html"):
                inner += f", `{o['html']}`"
            line = f"- Smart Object: **{_line(o.get('title', ''))}** ({inner})"
            if o.get("description"):
                line += f": {_line(o['description'])}"
            out.append(line)
    return "\n".join(out) + "\n"


def minimal_outline(z: zipfile.ZipFile) -> str:
    """Old package, no outline and no `loro`: what the manifests say."""
    m = _json(z, "manifest.json") or {}
    out = [f"# {_line(m.get('title') or 'Untitled lesson')}", "",
           f"- Pages: {m.get('pages', '?')}"]
    if _iso(m.get("created")):
        out.append(f"- Created: {_iso(m.get('created'))}")
    for name in sorted(z.namelist()):
        parts = name.split("/")
        if len(parts) == 3 and parts[0] == "objects" and parts[2] == "manifest.json":
            b = _json(z, name) or {}
            out.append(f"- Smart Object: **{_line(b.get('title', parts[1]))}** "
                       f"(`objects/{parts[1]}/{b.get('entry', 'index.html')}`)")
    out += ["", "_This package has no readable outline (written before Rupok added one). "
            "Install `loro` (`pip install loro`) to read the pages from doc.loro._"]
    return "\n".join(out) + "\n"


def read_rpk(path: str, source: str = "auto", as_json: bool = False) -> str:
    z = open_lesson(path)
    with z:
        outline = None
        if source in ("auto", "loro"):
            outline = outline_from_loro(z)
            if outline is None and source == "loro":
                raise NotALesson("the `loro` package is not installed (pip install loro)")
        if outline is not None:
            return json.dumps(outline, indent=2, ensure_ascii=False) + "\n" if as_json \
                else to_markdown(outline)
        if as_json:
            data = _read(z, "content.json", MAX_TEXT_ENTRY)
            if data is not None:
                return data.decode("utf-8", "replace")
            raise NotALesson("no content.json in this package; install loro to read doc.loro")
        data = _read(z, "content.md", MAX_TEXT_ENTRY)
        if data is not None:
            return data.decode("utf-8", "replace")
        return minimal_outline(z)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Print a Rupok lesson (.rpk) as Markdown.")
    ap.add_argument("file")
    ap.add_argument("--json", action="store_true", help="structured outline (content.json shape)")
    ap.add_argument("--list", action="store_true", help="list the package entries")
    ap.add_argument("--source", choices=["auto", "text", "loro"], default="auto",
                    help="auto: doc.loro when `loro` is installed, else content.md")
    a = ap.parse_args(argv)
    try:
        if a.list:
            with open_lesson(a.file) as z:
                for info in z.infolist():
                    print(f"{info.file_size:>10}  {info.filename}")
            return 0
        sys.stdout.write(read_rpk(a.file, a.source, a.json))
        return 0
    except NotALesson as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
