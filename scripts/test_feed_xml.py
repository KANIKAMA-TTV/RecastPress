"""Parse the Rust fixture with an independent XML implementation, locally only."""
import json
import os
from pathlib import Path
import subprocess
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ATOM = "{http://www.w3.org/2005/Atom}"
DC = "{http://purl.org/dc/elements/1.1/}"


class FeedXmlTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        completed = subprocess.run(
            ["cargo", "run", "-p", "recastpress-server", "--example", "feed_probe", "--offline", "--locked", "--quiet"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        cls.fixture = json.loads(completed.stdout)

    def test_budgeted_output_matches_parsed_xml_and_retains_charge_to_final_shared_owner(self):
        observations = self.fixture["budgeted_output_observations"]
        self.assertEqual([item["document"] for item in observations],
                         ["rss", "atom", "empty_rss", "empty_atom"])
        for item in observations:
            with self.subTest(document=item["document"]):
                for key in ["output_bytes", "owner_bytes", "charged_bytes", "claimed_before_drop",
                            "rendering_before_drop", "claimed_after_first_drop", "claimed_after_final_drop"]:
                    self.assertIs(type(item[key]), int)
                self.assertGreater(item["owner_bytes"], 0)
                self.assertEqual(item["output_bytes"], len(self.fixture[item["document"]].encode("utf-8")))
                charged = item["output_bytes"] + item["owner_bytes"]
                self.assertEqual(item["charged_bytes"], charged)
                self.assertEqual(item["claimed_before_drop"], charged)
                self.assertEqual(item["rendering_before_drop"], charged)
                self.assertEqual(item["claimed_after_first_drop"], charged)
                self.assertEqual(item["claimed_after_final_drop"], 0)

    def test_rss_required_fields_namespace_dates_and_original_html_text(self):
        root = ET.fromstring(self.fixture["rss"])
        self.assertEqual((root.tag, root.attrib), ("rss", {"version": "2.0"}))
        channel = root.find("channel")
        self.assertIsNotNone(channel)
        self.assertEqual(channel.findtext("title"), 'Site & "friends" <研究>')
        self.assertEqual(channel.findtext("link"), "https://example.test/")
        self.assertEqual(channel.findtext("description"), "Description\rline\nnext\t<&>")
        self.assertEqual(channel.find(f"{ATOM}link").attrib, {
            "rel": "self", "href": "https://example.test/feed/rss",
            "type": "application/rss+xml; charset=utf-8"})
        self.assertEqual(channel.findtext("lastBuildDate"), "Mon, 26 Oct 2026 03:04:05 GMT")
        items = channel.findall("item")
        self.assertEqual(len(items), 2)
        self.assertEqual(items[0].findtext("description"), "<p>A & B &lt;em&gt;日本語</p>\r\n")
        self.assertEqual(items[0].findtext("link"), "https://example.test/entry?x=1&y=2")
        self.assertEqual(items[0].findtext(f"{DC}creator"), "Author ' & name")
        self.assertEqual(items[0].find("guid").attrib["isPermaLink"], "false")
        self.assertEqual(items[0].findtext("pubDate"), "Thu, 01 Jan 1970 00:00:00 GMT")
        self.assertEqual(items[1].findtext("pubDate"), "Tue, 29 Feb 2000 12:34:56 GMT")
        self.assertEqual(self.fixture["rss_content_type"], "application/rss+xml; charset=utf-8")

    def test_atom_required_metadata_author_links_and_html_summary(self):
        root = ET.fromstring(self.fixture["atom"])
        self.assertEqual(root.tag, ATOM + "feed")
        for name in ["id", "title", "updated"]:
            self.assertEqual(len(root.findall(ATOM + name)), 1)
        self.assertEqual(root.findtext(ATOM + "id"), "https://example.test/feed-id")
        self.assertEqual(root.findtext(ATOM + "updated"), "2026-10-26T03:04:05Z")
        self.assertEqual(root.findtext(ATOM + "author/" + ATOM + "name"), "Feed author & friends")
        self_link = next(link for link in root.findall(ATOM + "link") if link.attrib.get("rel") == "self")
        self.assertEqual(self_link.attrib, {
            "rel": "self", "href": "https://example.test/feed/atom",
            "type": "application/atom+xml; charset=utf-8"})
        entries = root.findall(ATOM + "entry")
        self.assertEqual(len(entries), 2)
        self.assertEqual(entries[0].findtext(ATOM + "title"), 'Entry <title> & "quote"')
        self.assertEqual(entries[0].findtext(ATOM + "published"), "1970-01-01T00:00:00Z")
        alternate = next(link for link in entries[0].findall(ATOM + "link") if link.attrib.get("rel") == "alternate")
        self.assertEqual(alternate.attrib["href"], "https://example.test/entry?x=1&y=2")
        self.assertIn('href="https://example.test/entry?x=1&amp;y=2"', self.fixture["atom"])
        summary = entries[0].find(ATOM + "summary")
        self.assertEqual(summary.attrib["type"], "html")
        self.assertEqual(summary.text, "<p>A & B &lt;em&gt;日本語</p>\r\n")
        self.assertEqual(list(summary), [])
        self.assertEqual(self.fixture["atom_content_type"], "application/atom+xml; charset=utf-8")

    def test_empty_feeds_keep_required_metadata_and_author(self):
        rss = ET.fromstring(self.fixture["empty_rss"]).find("channel")
        self.assertEqual(rss.findall("item"), [])
        self.assertIsNotNone(rss.find("title"))
        self.assertEqual(rss.find(ATOM + "link").attrib, {
            "rel": "self", "href": "https://example.test/feed/rss",
            "type": "application/rss+xml; charset=utf-8"})
        atom = ET.fromstring(self.fixture["empty_atom"])
        self.assertEqual(atom.findall(ATOM + "entry"), [])
        self.assertIsNotNone(atom.find(ATOM + "author/" + ATOM + "name"))
        self_link = next(link for link in atom.findall(ATOM + "link") if link.attrib.get("rel") == "self")
        self.assertEqual(self_link.attrib, {
            "rel": "self", "href": "https://example.test/feed/atom",
            "type": "application/atom+xml; charset=utf-8"})

    def test_markup_and_carriage_returns_are_encoded_without_dtd_or_cdata(self):
        for key in ["rss", "atom", "empty_rss", "empty_atom"]:
            xml = self.fixture[key]
            self.assertNotIn("<!DOCTYPE", xml)
            self.assertNotIn("<![CDATA[", xml)
            self.assertNotIn("\r", xml)
            self.assertRegex(xml, r"&#(?:13|x0*[dD]);")
            self.assertIn("&amp;", xml)


if __name__ == "__main__":
    unittest.main()
