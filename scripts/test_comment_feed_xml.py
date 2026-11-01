"""Independent XML parsing of fixed, caller-prepared comment data; no DB/PHP."""
import json
import os
from pathlib import Path
import subprocess
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ATOM = "{http://www.w3.org/2005/Atom}"
CONTENT = "{http://purl.org/rss/1.0/modules/content/}"
DC = "{http://purl.org/dc/elements/1.1/}"
THREAD = "{http://purl.org/syndication/thread/1.0}"
XML = "{http://www.w3.org/XML/1998/namespace}"
FULL_HTML = '<p>Full 日本語 & <a href="relative">link</a>]]></p>\r\n'


class CommentFeedXmlTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        completed = subprocess.run(
            ["cargo", "run", "-p", "recastpress-server", "--example", "comment_feed_probe", "--offline", "--locked", "--quiet"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        cls.fixture = json.loads(completed.stdout)

    def test_rss_separate_summary_full_content_and_public_metadata_roundtrip(self):
        self.assertEqual(self.fixture["rss_content_type"], "application/rss+xml; charset=utf-8")
        root = ET.fromstring(self.fixture["rss"])
        self.assertEqual((root.tag, root.attrib), ("rss", {"version": "2.0"}))
        channel = root.find("channel")
        self.assertEqual(channel.findtext("title"), "Comments <公開> & readers")
        self.assertEqual(channel.findtext("description"), "Public comments\r\n<&>")
        self.assertEqual(channel.find(ATOM + "link").attrib, {"rel": "self", "href": "https://example.test/comments/rss", "type": self.fixture["rss_content_type"]})
        items = channel.findall("item")
        self.assertEqual(len(items), 2)
        self.assertEqual(items[0].findtext("description"), "<p>Summary & prepared</p>\r\n")
        self.assertEqual(items[0].findtext(CONTENT + "encoded"), FULL_HTML)
        self.assertEqual(list(items[0].find(CONTENT + "encoded")), [])
        self.assertEqual(items[0].findtext(DC + "creator"), "A & B")
        self.assertEqual(items[0].find("guid").attrib, {"isPermaLink": "false"})
        self.assertEqual(items[0].findtext("guid"), "https://example.test/post-id#comment-1")
        self.assertEqual(items[0].findtext("pubDate"), "Tue, 29 Feb 2000 12:34:56 GMT")
        self.assertEqual(items[1].findtext("pubDate"), "Thu, 01 Jan 1970 00:00:00 GMT")
        self.assertEqual(items[1].findtext(CONTENT + "encoded"), "")

    def test_atom_full_content_base_and_explicit_post_or_absent_parent_references(self):
        self.assertEqual(self.fixture["atom_content_type"], "application/atom+xml; charset=utf-8")
        root = ET.fromstring(self.fixture["atom"])
        self.assertEqual(root.tag, ATOM + "feed")
        self.assertEqual(root.findtext(ATOM + "id"), "https://example.test/comments-id")
        self_link = next(link for link in root.findall(ATOM + "link") if link.attrib["rel"] == "self")
        self.assertEqual(self_link.attrib, {"rel": "self", "href": "https://example.test/comments/atom", "type": self.fixture["atom_content_type"]})
        entries = root.findall(ATOM + "entry")
        self.assertEqual(len(entries), 2)
        content = entries[0].find(ATOM + "content")
        self.assertEqual(content.attrib, {"type": "html", XML + "base": "https://example.test/post?x=1&y=2#comment-1"})
        self.assertEqual(content.text, FULL_HTML)
        self.assertEqual(list(content), [])
        self.assertIsNone(entries[0].find(ATOM + "summary"))
        self.assertEqual(entries[0].find(THREAD + "in-reply-to").attrib, {
            "ref": "https://example.test/post-id", "href": "https://example.test/post#comments", "type": "text/html"})
        self.assertEqual(entries[1].find(THREAD + "in-reply-to").attrib, {
            "ref": "https://example.test/post-id#comment-999", "href": "https://example.test/post?x=3&y=4#comment-999", "type": "text/html"})
        self.assertNotIn("https://example.test/post-id#comment-999", [entry.findtext(ATOM + "id") for entry in entries])
        self.assertEqual(entries[0].findtext(ATOM + "published"), "2000-02-29T12:34:56Z")
        self.assertEqual(entries[0].findtext(ATOM + "updated"), "2000-02-29T12:34:56Z")

    def test_empty_documents_keep_metadata_and_xml_never_uses_cdata_or_dtd(self):
        rss = ET.fromstring(self.fixture["empty_rss"]).find("channel")
        atom = ET.fromstring(self.fixture["empty_atom"])
        self.assertEqual(rss.findall("item"), [])
        self.assertEqual(atom.findall(ATOM + "entry"), [])
        self.assertEqual(rss.findtext("title"), "Comments <公開> & readers")
        self.assertEqual(atom.findtext(ATOM + "author/" + ATOM + "name"), "Feed author")
        for name in ("rss", "atom", "empty_rss", "empty_atom"):
            with self.subTest(document=name):
                xml = self.fixture[name]
                ET.fromstring(xml)
                self.assertNotIn("<!DOCTYPE", xml)
                self.assertNotIn("<![CDATA[", xml)
                self.assertNotIn("\r", xml)
                self.assertIn("&#13;", xml)

    def test_budgeted_bytes_and_shared_owner_lifetime_are_checked_independently(self):
        observations = self.fixture["budgeted_output_observations"]
        self.assertEqual([item["document"] for item in observations], ["rss", "atom", "empty_rss", "empty_atom"])
        for item in observations:
            with self.subTest(document=item["document"]):
                for key, value in item.items():
                    if key != "document":
                        self.assertIs(type(value), int)
                self.assertEqual(item["output_bytes"], len(self.fixture[item["document"]].encode("utf-8")))
                self.assertGreater(item["owner_bytes"], 0)
                charge = item["output_bytes"] + item["owner_bytes"]
                for key in ("charged_bytes", "rendering_before_drop", "claimed_after_first_drop"):
                    self.assertEqual(item[key], charge)
                self.assertEqual(item["claimed_after_final_drop"], 0)


if __name__ == "__main__":
    unittest.main()
