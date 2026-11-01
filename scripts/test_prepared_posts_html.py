"""Independent HTML parsing of explicitly prepared text; no DB, WP or network."""
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
TITLE = "Title 日本 & < > \" '"
BODY = "\n<script>alert('literal')</script>\r\n\t&copy; 日本語 🦀"


class ParsedArticle(HTMLParser):
    def __init__(self, html):
        super().__init__(convert_charrefs=True)
        self.tags = []
        self.title = []
        self.heading = []
        self.body = []
        self.current = None
        self.body_attributes = None
        self.feed(html)
        self.close()

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        if tag == "title":
            self.current = self.title
        elif tag == "h1":
            self.current = self.heading
        elif tag == "div":
            self.current = self.body
            self.body_attributes = dict(attrs)

    def handle_endtag(self, tag):
        if tag in {"title", "h1", "div"}:
            self.current = None

    def handle_data(self, data):
        if self.current is not None:
            self.current.append(data)


class PreparedPostsHtmlTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        completed = subprocess.run(
            ["cargo", "run", "-p", "recastpress-server", "--example", "prepared_posts_probe",
             "--offline", "--locked", "--quiet"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        cls.fixture = json.loads(completed.stdout)

    def test_title_and_body_are_literal_text_without_html_or_entity_reinterpretation(self):
        parsed = ParsedArticle(self.fixture["html"])
        self.assertEqual("".join(parsed.title), TITLE)
        self.assertEqual("".join(parsed.heading), TITLE)
        self.assertEqual("".join(parsed.body), BODY)
        self.assertNotIn("script", parsed.tags)
        self.assertEqual(parsed.body_attributes, {"class": "post-body", "style": "white-space:pre-wrap"})
        self.assertIn("\r\n", "".join(parsed.body))
        self.assertTrue("".join(parsed.body).startswith("\n"))
        self.assertIn("&copy;", "".join(parsed.body))

    def test_empty_title_body_and_explicit_targets_survive_the_existing_bundle_shape(self):
        empty = ParsedArticle(self.fixture["empty_html"])
        self.assertEqual("".join(empty.title), "")
        self.assertEqual("".join(empty.heading), "")
        self.assertEqual("".join(empty.body), "")
        wire = self.fixture["bundle"]
        self.assertTrue(wire.endswith("\n"))
        bundle = json.loads(wire)
        self.assertEqual(set(bundle), {"format_version", "pages"})
        self.assertEqual(bundle["format_version"], 1)
        self.assertEqual([page["target"] for page in bundle["pages"]], ["/chosen?b=2&a=1", "/empty"])
        self.assertEqual([page["body"] for page in bundle["pages"]],
                         [self.fixture["html"], self.fixture["empty_html"]])
        self.assertTrue(all(page["status"] == 200 and page["content_type"] == "text/html; charset=utf-8"
                            for page in bundle["pages"]))
        self.assertEqual(set(self.fixture["input"]["posts"][0]), {"target", "title", "body_text"})


if __name__ == "__main__":
    unittest.main()
