# SPDX-License-Identifier: Apache-2.0
"""Tests for read_rpk.py.  Run: python3 -m unittest discover skills/read-rpk/tests"""

import io
import json
import os
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr, redirect_stdout

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import read_rpk  # noqa: E402

SAMPLE = os.path.join(HERE, "sample-lesson.rpk")

try:
    import loro  # noqa: F401
    HAVE_LORO = True
except ImportError:
    HAVE_LORO = False


def without(names, src=SAMPLE):
    """A copy of the sample without some entries (an older writer)."""
    fd, path = tempfile.mkstemp(suffix=".rpk")
    os.close(fd)
    with zipfile.ZipFile(src) as zin, zipfile.ZipFile(path, "w") as zout:
        for info in zin.infolist():
            if info.filename not in names:
                zout.writestr(info, zin.read(info.filename))
    return path


class TextEntries(unittest.TestCase):
    def test_prints_content_md(self):
        md = read_rpk.read_rpk(SAMPLE, source="text")
        with zipfile.ZipFile(SAMPLE) as z:
            self.assertEqual(md, z.read("content.md").decode())
        self.assertIn("# Binary numbers", md)
        self.assertIn("- Chapter: Number systems", md)
        self.assertIn("$$1011_2 = 8 + 2 + 1 = 11$$", md)
        self.assertIn("recognised maths (machine reading, may be wrong): $x^2$", md)
        self.assertIn("AND gate, OR gate, 2 × arrow", md)
        self.assertIn("`objects/binary-counter/index.html`", md)

    def test_json(self):
        c = json.loads(read_rpk.read_rpk(SAMPLE, source="text", as_json=True))
        self.assertEqual(c["format"], "rupok-lesson-content")
        self.assertEqual(c["page_count"], 3)
        self.assertEqual(c["pages"][2]["objects"][0]["steps"], 8)

    def test_readme_points_to_the_docs(self):
        with zipfile.ZipFile(SAMPLE) as z:
            readme = z.read("README.md").decode()
        self.assertTrue(readme.startswith("This is a Rupok lesson (Rich Presentation Kit"))
        self.assertIn("https://rupok.app/skills/read-rpk.md", readme)

    def test_old_package_without_outline(self):
        path = without({"README.md", "content.md", "content.json"})
        try:
            md = read_rpk.read_rpk(path, source="text")
            self.assertIn("# Binary numbers", md)
            self.assertIn("Binary counter", md)
            self.assertIn("pip install loro", md)
        finally:
            os.unlink(path)

    def test_rejects_non_lessons(self):
        fd, path = tempfile.mkstemp(suffix=".rpk")
        os.write(fd, b"not a zip")
        os.close(fd)
        try:
            with self.assertRaises(read_rpk.NotALesson):
                read_rpk.read_rpk(path)
            buf = io.StringIO()
            with redirect_stdout(buf), redirect_stderr(buf):
                self.assertEqual(read_rpk.main([path]), 1)
        finally:
            os.unlink(path)
        path = without({"mimetype"})
        try:
            with self.assertRaises(read_rpk.NotALesson):
                read_rpk.read_rpk(path)
        finally:
            os.unlink(path)

    def test_list(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.assertEqual(read_rpk.main([SAMPLE, "--list"]), 0)
        self.assertIn("doc.loro", buf.getvalue())


@unittest.skipUnless(HAVE_LORO, "loro not installed (pip install loro)")
class LoroDocument(unittest.TestCase):
    def test_doc_loro_matches_written_outline(self):
        # Reading doc.loro gives the same Markdown Rupok wrote.
        self.assertEqual(read_rpk.read_rpk(SAMPLE, source="loro"),
                         read_rpk.read_rpk(SAMPLE, source="text"))

    def test_old_package_reads_pages_from_doc(self):
        path = without({"README.md", "content.md", "content.json"})
        try:
            md = read_rpk.read_rpk(path)
            self.assertIn("## Page 2: Logic gates", md)
            self.assertIn("Handwriting: 3 ink strokes", md)
            self.assertNotIn("recognised maths", md)  # only Rupok's outline has it
        finally:
            os.unlink(path)


if __name__ == "__main__":
    unittest.main()
