"""Serve only the fixed prepared comment-feed example through an owned server."""
import json
import xml.etree.ElementTree as ET

from smoke import ROOT, reap_servers, request, start, stop


def run_example():
    bundle = ROOT/"examples/comment-feed-bundle.json"
    recorded = json.loads(bundle.read_text(encoding="utf-8"))
    pages = {page["target"]: page for page in recorded["pages"]}
    observations = []
    server = start("public", "--host", "example.test", "--bundle", str(bundle))
    original = None
    try:
        for target, root_tag, mime in [
            ("/comments/rss", "rss", "application/rss+xml; charset=utf-8"),
            ("/comments/atom", "{http://www.w3.org/2005/Atom}feed", "application/atom+xml; charset=utf-8"),
        ]:
            expected = pages[target]["body"].encode("utf-8")
            status, headers, body = request(server.port, target)
            assert status == 200 and body == expected
            assert headers["content-type"] == mime
            assert int(headers["content-length"]) == len(expected)
            document = ET.fromstring(body)
            assert document.tag == root_tag
            if root_tag == "rss":
                entries = document.findall("./channel/item")
                assert len(entries) == 2
                bodies = []
                for entry in entries:
                    description = entry.find("description")
                    content = entry.find("{http://purl.org/rss/1.0/modules/content/}encoded")
                    assert description is not None and content is not None
                    bodies.append((description.text or "", content.text or ""))
                assert any(description != content for description, content in bodies)
            else:
                entries = document.findall("{http://www.w3.org/2005/Atom}entry")
                assert len(entries) == 2
                for entry in entries:
                    content = entry.find("{http://www.w3.org/2005/Atom}content")
                    reply = entry.find("{http://purl.org/syndication/thread/1.0}in-reply-to")
                    assert content is not None and content.attrib["type"] == "html"
                    assert content.attrib["{http://www.w3.org/XML/1998/namespace}base"]
                    assert reply is not None and reply.attrib["ref"] and reply.attrib["href"]
                    assert reply.attrib["type"] == "text/html"
            tag = headers["etag"]
            status, headers, body = request(server.port, target, "HEAD")
            assert status == 200 and body == b"" and headers["etag"] == tag
            assert int(headers["content-length"]) == len(expected)
            for method in ["GET", "HEAD"]:
                status, headers, body = request(server.port, target, method,
                    extra_headers={"If-None-Match": "W/"+tag})
                assert status == 304 and body == b"" and headers["etag"] == tag
                assert headers.get("content-length") in (None, str(len(expected)))
            status, headers, body = request(server.port, target,
                extra_headers={"Range": "bytes=16-32", "If-Range": tag})
            assert status == 206 and body == expected[16:33]
            assert headers["content-range"] == f"bytes 16-32/{len(expected)}"
            assert request(server.port, target, host="foreign.test")[0] == 421
            assert request(server.port, target+"?unprepared=1")[0] == 404
            assert request(server.port, target, "POST")[0] == 405
            observations.append({"target": target, "bytes": len(expected), "entries": len(entries),
                "etag": tag, "xml_parsed": True, "head_verified": True,
                "conditional_verified": True, "range_verified": True})
    except BaseException as error:
        original = error
        raise
    finally:
        stop(server, original=original)
    (ROOT/"target/comment-feed-example-http-observations.json").write_text(
        json.dumps({"profile": "fixed_prepared_example_only", "process_pid": server.process.pid,
            "exit_code": server.process.returncode, "reaped": server.process.poll() is not None,
            "feeds": observations}, indent=2)+"\n", encoding="utf-8")
    print("Fixed comment-feed example HTTP passed: XML, GET/HEAD, validators, byte Range; owned server reaped")


def main():
    original = None
    try:
        run_example()
    except BaseException as error:
        original = error
        raise
    finally:
        reap_servers(original=original)


if __name__ == "__main__":
    main()
