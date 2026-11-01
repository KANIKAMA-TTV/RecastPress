"""Compare supported Rust primitive subsets against isolated pinned WP functions."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import itertools
import json
import math
import struct
from urllib.parse import quote_from_bytes
import os
from pathlib import Path
import platform
import random
import subprocess

import compatibility_registry as registry_tool
import wordpress_reference as reference
import wordpress_entity_names as entity_names
import wordpress_xml_entity_mapping as xml_entities
import wp_bcrypt_profile as wp_bcrypt
import wp_rehash_profile as wp_rehash
import export_remove_accents as accent_data
import remove_accents_profile as accent_profile
import user_sanitization_profile as user_profile
import slug_byte_profile as slug_profile
import sanitize_title_profile as default_title_profile
import truncate_slug_profile as truncate_profile
import post_meta_cached_profile as post_meta_profile
import texturize_profile
import title_profile
import auth_cookie_profile
import auth_cookie_parse_profile
import sanitize_text_profile


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")


BATCH_CASE_LIMIT = 2048
BATCH_INPUT_LIMIT = 1024 * 1024


def batches(inputs, max_cases=BATCH_CASE_LIMIT, max_bytes=BATCH_INPUT_LIMIT):
    if max_cases <= 0 or max_bytes < 2:
        raise ValueError("comparison batch limits must be positive and fit a JSON array")
    batch, size = [], 2  # The surrounding [] are part of the input budget.
    for case in inputs:
        encoded = len(canonical(case))
        if encoded + 2 > max_bytes:
            raise ValueError("one primitive case exceeds the fixture input budget")
        if batch and (len(batch) >= max_cases or size + 1 + encoded > max_bytes):
            yield batch
            batch, size = [], 2
        size += encoded + bool(batch)
        batch.append(case)
    if batch:
        yield batch


def canonical_list_hash(values):
    digest = hashlib.sha256(b"[")
    for index, value in enumerate(values):
        if index:
            digest.update(b",")
        digest.update(canonical(value))
    digest.update(b"]")
    return digest.hexdigest()


def string(value):
    return {"type": "string", "hex": value.hex()}


def integer(value):
    return {"type": "int", "value": str(value)}


def array(keys):
    return {"type": "array", "entries": [{"key": key, "value": {"type": "null"}} for key in keys]}


def cases():
    result = []

    def add(api, value, filter_mode=None, **extra):
        case = {"id": f"primitive-{len(result):05d}", "api": api, "input": value}
        case.update(extra)
        if filter_mode:
            case["filter"] = filter_mode
        result.append(case)

    # Explicit i64 predicate only: do not invoke PHP's other (int) casts.
    unicode_boundaries = [-(2**63), -1, 0, 9, 10, 13, 0x20, 0xD7FF, 0xD800,
                          0xDFFF, 0xE000, 0xFDD0, 0xFDEF, 0xFFFD, 0xFFFE,
                          0xFFFF, 0x10000, 0x10FFFF, 0x110000, 2**63 - 1]
    unicode_boundaries.extend(plane * 0x10000 + tail
                              for plane in range(17) for tail in (0xFFFE, 0xFFFF))
    unicode_points = set(range(34))
    unicode_points.update(point + delta for point in unicode_boundaries for delta in range(-2, 3)
                          if -(2**63) <= point + delta < 2**63)
    for point in sorted(unicode_points):
        add("valid_unicode", integer(point))
    unicode_generator = random.Random(7132346)
    for _ in range(2000):
        add("valid_unicode", integer(unicode_generator.randrange(-1024, 0x111000)))
    for _ in range(1000):
        add("valid_unicode", integer(unicode_generator.randrange(-(2**63), 2**63)))

    # Fixed default-name HTML context, not KSES bootstrap or a PHP HTML table.
    def entity_case(raw):
        add("wp_kses_normalize_entities", string(raw), context="html")

    for name in entity_names.read_names():
        raw = name.encode("ascii")
        for variant in (raw, raw.upper(), raw.swapcase(), raw + b"0", raw + b"XYZ"):
            entity_case(b"&" + variant + b";")
        for raw_input in (b"&" + raw, b"&&" + raw + b";", b"&amp;" + raw + b";",
                          b"&" + raw + b";tail"):
            entity_case(raw_input)
    entity_points = {0, 1, 8, 9, 10, 11, 13, 31, 32, 65, 127, 0x85, 9999999, 10000000}
    entity_points.update(point + delta for point in unicode_boundaries for delta in (-1, 0, 1)
                         if 0 <= point + delta < 2**63)
    for point in sorted(entity_points):
        decimal = str(point).encode()
        hex_lower = format(point, "x").encode()
        hex_upper = format(point, "X").encode()
        for zeros in (b"", b"0", b"00000000"):
            for body in (b"#" + zeros + decimal, b"#x" + zeros + hex_lower, b"#X" + zeros + hex_upper):
                entity_case(b"&" + body + b";")
    for length in (64, 4096):
        for ending in (b"0", b"9", b"65", b"1114112"):
            entity_case(b"&#" + b"0" * length + ending + b";")
        for ending in (b"0", b"D", b"D800", b"FFFF", b"10FFFF", b"110000"):
            entity_case(b"&#X" + b"0" * length + ending + b";")
    for raw in (b"AT&T", b"&amp;#x2E;", b"&#x2E;", b"&amp;amp;", b"&AMP;", b"&#0;",
                b"&#X0000;", b"&#65", b"&#x0A", b"&copy\n;", b"a\0\xff&copy;",
                b"&CounterClockwiseContourIntegral;", b"&#99999999;", b"&#x1000000;"):
        entity_case(raw)
    for byte in range(256):
        unit = bytes([byte])
        for raw in (unit + b"&copy;", b"&" + unit + b";", b"&amp;" + unit,
                    b"&#" + unit + b";", b"&#x" + unit + b";"):
            entity_case(raw)
    entity_generator = random.Random(7132169)
    entity_atoms = [b"&", b"amp;", b"#", b"X", b"x", b"000", b"65;", b"copy;", b"&amp;",
                    b"&#x2E;", b"&#0;", b";", b"<>'\"", b"\0", b"\xff"]
    for _ in range(1000):
        entity_case(entity_generator.randbytes(entity_generator.randrange(129)))
        entity_case(b"".join(entity_generator.choice(entity_atoms) for _ in range(entity_generator.randrange(21))))

    # Same original-amp/numeric boundaries, but an explicit fixed UTF-8 XML
    # profile. Preserve HTML cases unchanged; record both profile counts.
    html_cases = [case for case in result if case["api"] == "wp_kses_normalize_entities"]
    for case in html_cases:
        add("wp_kses_normalize_entities", case["input"], context="xml")
    for name in entity_names.read_names():
        raw = name.encode("ascii")
        for value in (b"&" + raw + b";&amp;", b"&amp;amp;" + raw + b";",
                      b"&#38;" + raw + b";", b"&#x26;" + raw + b";"):
            add("wp_kses_normalize_entities", string(value), context="xml")
    for value in (b"&fjlig;&NotEqualTilde;&acE;&nGg;", b"&quot;&apos;&amp;&lt;&gt;",
                  b"&amp;#x26;copy;", b"\x00\xff&euro;\xc0\x80&copy;", b"&copy;;"):
        add("wp_kses_normalize_entities", string(value), context="xml")

    # Explicit UTF-8 and selected quote arguments; no generic ENT mask/casts.
    special_inputs = [b"", b"plain", b"&<>\"'", b"\xff", b"\xff&", b"\0&", b"\0",
                      b"&copy;&euro;&apos;&quot;&amp;&lt;&gt;", b"&fjlig;&NotEqualTilde;&COPY;",
                      b"&amp;copy;&#38;copy;&#x26;copy;&amp;#x26;copy;",
                      b"'\"&#039;&amp;apos;", b"&copy &copy;; &; &&copy;", "日本語😀&copy;".encode()]
    special_inputs.extend(b"&" + name.encode() + b";" for name in entity_names.read_names())
    for point in sorted(entity_points):
        special_inputs.extend((f"&#{point};".encode(), f"&#X000{point:X};".encode()))
    for point in sorted({0, 1, 9, 10, 13, 0x7f, 0xfdd0, 0xfdef, 0xfffe, 0xffff}
                        | {plane * 65536 + tail for plane in range(1, 17) for tail in (0xfffe, 0xffff)}):
        special_inputs.append(chr(point).encode() + b"&<>\"'")
    for byte in range(256):
        unit = bytes([byte])
        special_inputs.extend((unit, unit + b"&copy;", b"'" + unit + b"&amp;"))
    for invalid in (b"\xc0\x80", b"\xc1\xbf", b"\xe0\x80\x80", b"\xed\xa0\x80",
                    b"\xf4\x90\x80\x80", b"\xf5\x80\x80\x80", b"\xe2\x82", b"\x80\xbf"):
        special_inputs.append(invalid)
        special_inputs.extend(invalid + bytes([byte]) for byte in b"&<>\"'")
    for raw in (b"&#" + b"0" * 4096 + b"39;", b"&#X" + b"0" * 4096 + b"27;",
                b"&#" + b"9" * 4096 + b";", b"&#x" + b"F" * 4096 + b";",
                b"&amp;apos;&#039;&amp;#039;", b"&amp;amp;"):
        special_inputs.append(raw)
    special_generator = random.Random(713945)
    special_atoms = entity_atoms + [b"&apos;", b"&#xFDD0;", b"&#x10FFFF;", "日本語".encode()]
    for _ in range(300):
        special_inputs.append(special_generator.randbytes(special_generator.randrange(129)))
        special_inputs.append(b"".join(special_generator.choice(special_atoms)
                                       for _ in range(special_generator.randrange(21))))
    for raw in special_inputs:
        for style in registry_tool.SPECIALCHARS_STYLES:
            for double in (False, True):
                add("_wp_specialchars", string(raw), quote_style=style, double_encode=double)
    for raw in special_inputs:
        for api in ("esc_html", "esc_attr"):
            add(api, string(raw), filter_profile="empty")

    # Public fixed dummy hashes only. Never generate a credential or select an
    # authentication policy. Costs above an explicit host allowance are refusal
    # observations, not successful differential boolean cases.
    def bcrypt_case(password, name="dummy", maximum=4):
        add("wp_check_password", string(password), password_hash=string(wp_bcrypt.VECTORS[name].encode()),
            max_cost=maximum, password_profile="wp_bcrypt_empty")
    for name, password in (("empty", b""), ("dummy", b"dummy-pass"), ("binary", b"dummy\0\xffpass"),
                           ("spaced", b" spaced dummy "), ("max4096", b"x" * 4096)):
        bcrypt_case(password, name)
        bcrypt_case(password + b"X", name)
        bcrypt_case(b"wrong-public-dummy", name)
    for byte in range(256):
        bcrypt_case(bytes([byte]))
        bcrypt_case(b"dummy-pass" + bytes([byte]))
    for password in (b" dummy-pass", b"dummy-pass ", b"dummy-pass\0tail", b"\0dummy-pass",
                     b"x" * 71, b"x" * 72, b"x" * 73, b"x" * 4095, b"x" * 4096,
                     b"x" * 4097, b"x" * 8192, "日本語🦀".encode()):
        bcrypt_case(password)
    bcrypt_case(b"dummy-pass", "cost12", 12)
    bcrypt_case(b"wrong-public-dummy", "cost12", 12)
    # The source length guard runs before parsing this otherwise high-cost hash.
    bcrypt_case(b"x" * 4097, "cost12", 4)

    # Byte-value domain only. The real decimal pass can generate a new hex
    # reference, but neither pass recursively repeats itself.
    def decode_entity_case(raw):
        add("wp_kses_decode_entities", string(raw))

    for value in range(256):
        decimal, hex_lower, hex_upper = str(value).encode(), format(value, "x").encode(), format(value, "X").encode()
        for raw in (b"&#" + decimal + b";", b"&#000" + decimal + b";",
                    b"&#x" + hex_lower + b";", b"&#X00" + hex_upper + b";",
                    b"&#38;#x" + hex_lower + b";", b"&#38;&#35;x" + hex_lower + b";",
                    b"&#38;#&#120;" + hex_lower + b";", b"&#38;#" + decimal + b";",
                    b"&#x26;#x" + hex_lower + b";",
                    b"&#x" + format(value >> 4, "x").encode() + b"&#" + str(ord(format(value & 15, "x"))).encode() + b";;"):
            decode_entity_case(raw)
    for length in (0, 1, 8, 64, 4096):
        for value in (0, 9, 65, 128, 255):
            decode_entity_case(b"&#" + b"0" * length + str(value).encode() + b";")
            decode_entity_case(b"&#X" + b"0" * length + format(value, "X").encode() + b";")
    for raw in (b"", b"&copy; &amp;#x41;", b"&#-1; &#x-1;", b"&#256x; &#x100G;",
                b"&#65 &#xFF", b"&#38;#65;", b"&#x26;#x41;", b"\xff\0&#0;&#128;",
                b"&#38;&#35;&#120;&#52;&#49;&#59;", b"&#38;#x41&#59;",
                b"&#38;#x&#0;41;", b"&#38;#x&#255;41;", b"&#x26;&#35;&#120;&#52;&#49;&#59;",
                b"&#" + b"9" * 4096 + b"x;", b"&#x" + b"F" * 4096 + b"G;"):
        decode_entity_case(raw)
    for byte in range(256):
        unit = bytes([byte])
        for raw in (unit + b"&#65;", b"&#65;" + unit, b"&" + unit + b"copy;",
                    b"raw\0" + unit + b"&#xFF;"):
            decode_entity_case(raw)
    decode_generator = random.Random(7132376)
    decode_atoms = [b"&#0;", b"&#38;", b"&#35;", b"&#120;", b"&#65;", b"&#xFF;",
                    b"&#x26;", b"&copy;", b"&amp;", b"raw<>", b"\xff", b"\0", b"|"]
    for _ in range(2000):
        raw = decode_generator.randbytes(decode_generator.randrange(257))
        if b"&#" not in raw:
            decode_entity_case(raw)
        decode_entity_case(b"".join(decode_generator.choice(decode_atoms) for _ in range(decode_generator.randrange(21))))

    # Explicit source option strings only. The first control filter can create
    # slash/zero runs; TAB/LF/CR survive and interrupt those runs.
    def no_null_case(raw):
        for mode in ("remove", "keep"):
            add("wp_kses_no_null", string(raw), slash_zero=mode)

    for byte in range(256):
        unit = bytes([byte])
        for raw in (unit, b"\\" + unit + b"0", b"\\0" + unit, unit + b"\\0",
                    b"A" + unit + b"B", b"\\" + unit + b"\\0", b"0" + unit + b"0",
                    b"\\" + unit + b"0\\"):
            no_null_case(raw)
    for raw in (b"", b"\\0", b"\\\\000", b"\\0\\00", b"\\0\\", b"\\x\\0", b"\\\\x0",
                b"\\\0\x1f00", b"\\\x1f\\0", b"x\\\0\x1f00y", b"\\0\n0", b"\\\t0",
                b"\\\r0", b"\0\\0\xff\r\n\t\x7f", b"\\n\\r\\t&#0;", b"0\\x0"):
        no_null_case(raw)
    for slashes, zeros in itertools.product((0, 1, 2, 3, 8, 64, 4096), repeat=2):
        no_null_case(b"\\" * slashes + b"0" * zeros)
        no_null_case(b"\\\0" * slashes + b"0\x1f" * zeros)
    no_null_generator = random.Random(7132020)
    no_null_atoms = [b"\\", b"0", b"\\0", b"\\000", b"\0", b"\x08", b"\x0b", b"\x0c", b"\x1f",
                     b"\t", b"\n", b"\r", b"\x7f", b"\xff", b"A", b"x0", b"&#0;", b"\\n"]
    for _ in range(1000):
        no_null_case(no_null_generator.randbytes(no_null_generator.randrange(257)))
        no_null_case(b"".join(no_null_generator.choice(no_null_atoms) for _ in range(no_null_generator.randrange(33))))

    # One original backslash/quote pair is replaced once; preceding slashes
    # are not recursively processed like general stripslashes/wp_unslash.
    slash, double_quote = bytes((92,)), bytes((34,))
    quote_pair = slash + double_quote
    for byte in range(256):
        unit = bytes([byte])
        for raw in (slash + unit, unit + quote_pair, quote_pair + unit,
                    slash + unit + double_quote, unit + quote_pair + unit):
            add("wp_kses_stripslashes", string(raw))
    for count in (*range(9), 64, 4096):
        for tail in (double_quote, b"'", b"0", b"x", b"", b"\0", b"\xff"):
            add("wp_kses_stripslashes", string(slash * count + tail))
    for raw in (b"", quote_pair * 4096, b"a\0\xff" + quote_pair + b"b", quote_pair * 2,
                b"&quot;" + quote_pair + b"&#34;", slash + b"n" + slash + b"r" + slash + b"t",
                "日本語".encode() + quote_pair + b"\xff"):
        add("wp_kses_stripslashes", string(raw))
    kses_slash_generator = random.Random(7132044)
    kses_slash_atoms = [slash, double_quote, quote_pair, slash * 2, slash * 3 + double_quote,
                        b"'", slash + b"'", slash + b"0", slash + b"x", b"\0", b"\xff", b"&quot;", b"raw"]
    for _ in range(1500):
        add("wp_kses_stripslashes", string(kses_slash_generator.randbytes(kses_slash_generator.randrange(257))))
        add("wp_kses_stripslashes", string(b"".join(kses_slash_generator.choice(kses_slash_atoms)
                                                    for _ in range(kses_slash_generator.randrange(33)))))

    # Explicit byte allowlist only: array keys do not affect source array_any.
    # Schemes use the inherited two-pass byte-value decoder admission domain.
    def protocol_case(raw, values, mixed_keys=False):
        entries = [{"key": string(f"key-{index}".encode()) if mixed_keys and index % 2 else integer(-index),
                    "value": string(value)} for index, value in enumerate(values)]
        add("wp_kses_bad_protocol_once2", string(raw), allowed_protocols={"type": "array", "entries": entries})

    for raw, values in [
        (b"HTTP", [b"http"]), (b"http", [b"HTTP"]), (b"h t\nt\rp", [b"http"]),
        (b"ht\\\t0tp", [b"http"]), (b"ht\\ 0tp", [b"http"]), (b"h\0t\x1ftp", [b"http"]),
        (b"", [b""]), (b"", []), (b"0", [b"0"]), (b"http:", [b"http:"]), (b"http:", [b"http"]),
        (b"123_+/@", [b"123_+/@"]), (b"&amp;", [b"&amp;"]), (b"ht tp", [b"ht tp"]),
        (b"http", [b" http"]), (b"&#104;ttp", [b"&#104;ttp"]),
        (b"h&#116;tp", [b"http"]), (b"h&#38;#x74;tp", [b"http"]),
        (b"\x80 \0", [b"\x80"]), (b"\x80 \0", [b"\x80 \0"]), (b"\xff", [b"\xff"]),
        ("Ä".encode(), ["ä".encode()]), (b"\0\\0", [b""]),
        (b"&#38;#256;", [b"&#256;"]), (b"&#x26;#x100;", [b"&#x100;"]),
        (b"&#38;#\\000x100;", [b"&#x100;"]),
        (b"&#256x;", [b"&#256x;"]), (b"HTTP", [b"not-http", b"HtTp", b"HTTP"]),
    ]:
        protocol_case(raw, values, mixed_keys=True)
    for byte in range(256):
        unit = bytes([byte])
        for raw in (unit, b"h" + unit + b"ttp", b"ht\\" + unit + b"0tp", b"HTTP" + unit):
            for values in ([], [b""], [b"http"], [unit]):
                protocol_case(raw, values)
        for raw in (b"&#" + str(byte).encode() + b";", b"&#38;#x" + format(byte, "x").encode() + b";"):
            protocol_case(raw, [unit])
            protocol_case(raw, [b""])
    for length in (1, 64, 4096):
        protocol_case(b" " * length + b"h&#116;tp", [b"HTTP"])
        protocol_case(b" " * length + b"h&#116;tp", [])
        protocol_case(b"&#" + b"0" * length + b"255;", [b"\xff"])
    protocol_case(b"proto-99", [f"PROTO-{index}".encode() for index in range(100)], mixed_keys=True)
    protocol_generator = random.Random(7132137)
    protocol_pool = [b"http", b"HTTP", b"", b"0", b"123_+/@", b"\xff", b"http:", b"ht tp", b"\0"]
    for _ in range(1500):
        raw = protocol_generator.randbytes(protocol_generator.randrange(129))
        if b"&#" not in raw:
            protocol_case(raw, [raw, *protocol_generator.sample(protocol_pool, 3)])
        raw = protocol_generator.choice([b"h t t p", b"h&#116;tp", b"ht\\\t0tp", *protocol_pool])
        protocol_case(raw, protocol_generator.sample(protocol_pool, protocol_generator.randrange(5)))

    # Default count=1 only. Decoder admission is conditional on the exact
    # source branch: neither the untouched body nor the raw /? guard is decoded.
    def protocol_once_case(raw, values):
        entries = [{"key": integer(index), "value": string(value)} for index, value in enumerate(values)]
        add("wp_kses_bad_protocol_once", string(raw), allowed_protocols={"type": "array", "entries": entries})

    once_profiles = [[], [b"http", b"https", b"feed"], [b"", b"feed", b"http", b"/?x"]]
    once_edges = [b"http: x", b"HTTP: x", b"http:a:b", b"bad:http:x", b":rest", b"no delimiter",
                  b"http&#58; x", b"http&#58 x", b"http&#058x", b"http&#0589x", b"http&#58",
                  b"http&#x3a x", b"http&#X003AZ", b"http&#x3af", b"http&#x3ag", b"http&COLON;x",
                  b"http&colonx", b"http:a&#58b", b"raw/?path&#58x", b"x/?p:http:x", b"&#47;?x:http:a",
                  b"/\0?x:http:a", b"http:&#256;", b"plain&#256;", b"raw/?&#256;:rest", b"bad:&#256;:rest",
                  b"feed:&#256;", b"http:\0 \t\n\x0b\rX\0 \t\n\x0b\r", b"http:\x0cX\x0c",
                  b"feed:http:x", b"feed:feed:http:x", b"feed:feed:feed:http:x", b"feed:", b"feed:0", b"feed:00",
                  b"feed:feed:0", b"feed:bad:javascript:x", b"FEED&COLON;HTTP:x", b"\xff:http:\0x"]
    for raw in once_edges:
        for values in once_profiles:
            protocol_once_case(raw, values)
    protocol_once_case(b"feed:feed:&#256;:rest", [])
    protocol_once_case(b"feed:feed:feed:&#256;:x", [b"feed", b"http"])
    once_delimiters = [b":", b"&#58;", b"&#058;", b"&#58", b"&#x3a;", b"&#X003A;", b"&#x3a",
                       b"&colon;", b"&COLON;", b"&colonx", b"&#0589", b"&#x3af"]
    once_tails = [b"", b"0", b"00", b" x\t\0 ", b"\x0cX\x0c", b"http:x", b"feed:http:x",
                  b"feed:feed:http:x", b"raw/?http:x", b"\xff:tail"]
    for prefix, delimiter, tail in itertools.product([b"http", b"feed", b"bad", b"", b"raw/?x"], once_delimiters, once_tails):
        protocol_once_case(prefix + delimiter + tail, [b"http", b"feed"])
    for byte in range(256):
        unit = bytes([byte])
        for raw in (unit + b"http:x", b"http:" + unit + b"tail", b"feed:" + unit + b":x",
                    b"raw/?" + unit + b":tail", b"http&#58" + unit + b"tail", b"http&#x3a" + unit + b"tail"):
            for values in once_profiles:
                protocol_once_case(raw, values)
    for length in (0, 1, 8, 64, 4096):
        for delimiter in (b"&#" + b"0" * length + b"58", b"&#X" + b"0" * length + b"3A"):
            protocol_once_case(b"http" + delimiter + b"Z", [b"http"])
            protocol_once_case(b"raw/?x" + delimiter + b"Z", [])
    once_generator = random.Random(7132100)
    for _ in range(1000):
        tail = once_generator.randbytes(once_generator.randrange(129))
        if b"&#" in tail:
            continue
        values = once_generator.choice(once_profiles)
        protocol_once_case(tail, values)
        prefix = once_generator.choice([b"http", b"bad", b"", b"f&#101;ed", b"raw/?x", b"&#47;?x", b"\xff"])
        complete_delimiter = once_generator.choice([b":", b"&#58;", b"&#058;", b"&#x3a;", b"&#X003A;", b"&colon;", b"&COLON;"])
        raw = b"feed:" * once_generator.randrange(5) + prefix + complete_delimiter + tail
        protocol_once_case(raw, values)

    def protocol_outer_case(raw, values):
        entries = [{"key": integer(index), "value": string(value)} for index, value in enumerate(values)]
        add("wp_kses_bad_protocol", string(raw), allowed_protocols={"type": "array", "entries": entries})

    outer_profiles = [[], [b"http", b"https", b"feed"], [b"HTTP", b"HTTPS", b"FEED"],
                      [b"", b"0", b"http:", b"ht tp", b"\xff"]]
    outer_edges = [b"", b"0", b"http://x?a&#58b ", b"HTTP://x?a&#58b ", b"https://x\t",
                   b"h\0ttp://x\\0y ", b"httpx://x", b"http:&#256;", b"plain&#256;",
                   b"raw/?&#256;:x", b"/\0?x:http:x", b"feed:0", b"feed:00",
                   b"feed:feed:http:x", b"feed:feed:feed:http:x", b"http:\xff\0x",
                   b"bad:" * 6 + b"&#256;:x"]
    for raw, values in itertools.product(outer_edges, outer_profiles):
        protocol_outer_case(raw, values)
    for count, tail, values in itertools.product(range(9), [b"x", b"http:x", b"HTTP:x", b"feed:0", b"feed:feed:http:x"], outer_profiles):
        protocol_outer_case(b"bad:" * count + tail, values)
    for byte, values in itertools.product(range(256), outer_profiles):
        unit = bytes([byte])
        for raw in [unit, b"http://x" + unit + b"tail ", b"h" + unit + b"ttp:x", b"bad:" + unit + b"http:x", b"raw/?" + unit + b":&#256;"]:
            protocol_outer_case(raw, values)
    outer_generator = random.Random(7131984)
    for _ in range(1200):
        raw = outer_generator.randbytes(outer_generator.randrange(129))
        if b"&#" in raw:
            continue
        protocol_outer_case(raw, outer_generator.choice(outer_profiles))
        protocol_outer_case(b"bad:" * outer_generator.randrange(9) + outer_generator.choice([b"http:", b"feed:", b"raw/?:"]) + raw,
                            outer_generator.choice(outer_profiles))

    scalars = [{"type": "null"}, {"type": "bool", "value": False}, {"type": "bool", "value": True}]
    scalars += [integer(value) for value in [-(2**63), -1, 0, 1, 2**63 - 1]]
    scalars += [{"type": "float", "value": value} for value in ["0", "-0", "0.001", "-1", "nan", "inf", "-inf"]]
    for value in scalars:
        # PHP 8.5 emits a NaN-cast warning. Rust's diagnostic contract is pending;
        # that input explicitly returns UnsupportedValue and is not claimed here.
        if value != {"type": "float", "value": "nan"}:
            add("wp_validate_boolean", value)
        add("wp_is_numeric_array", value)
    strings = [b"", b"0", b"00", b"false", b"FALSE", b"FaLsE", b" false ", b"true", b"no", b"off", b"UPPER_key-09 <script>", "日本語Ä".encode()]
    strings += [bytes([byte]) for byte in range(256)]
    generator = random.Random(713)
    strings += [generator.randbytes(generator.randrange(257)) for _ in range(100)]
    for value in strings:
        add("wp_validate_boolean", string(value))
        add("wp_is_numeric_array", string(value))
        add("sanitize_key", string(value))
        add("sanitize_key", string(value), "append_raw")
    # Explicit string argument only: no null/get_option or nonstring coercion.
    # All sixteen case aliases, exhaustive byte edits and binary random strings
    # distinguish exact ASCII comparison from trimming or C-string termination.
    charset_aliases = [bytes(parts) for alias in (b"utf8", b"utf-8")
                       for parts in itertools.product(*[(byte, byte - 32) if 97 <= byte <= 122 else (byte,)
                                                        for byte in alias])]
    for alias in charset_aliases:
        add("is_utf8_charset", string(alias))
    for byte in range(256):
        add("is_utf8_charset", string(bytes([byte])))
        for alias in (b"UTF8", b"UTF-8"):
            for offset in range(len(alias) + 1):
                add("is_utf8_charset", string(alias[:offset] + bytes([byte]) + alias[offset:]))
            for offset in range(len(alias)):
                add("is_utf8_charset", string(alias[:offset] + bytes([byte]) + alias[offset + 1:]))
    for alias in (b"UTF8", b"UTF-8"):
        for offset in range(len(alias)):
            add("is_utf8_charset", string(alias[:offset] + alias[offset + 1:]))
    for sample in [b"", b"UTF", b"UTF--8", b"UTF_8", b"UTF-08", b"UTF-8 UTF8", b"utf-8; charset=utf8",
                   b"\0UTF-8", b"UTF-8\0", b"UTF-8\0UTF-8", b"\xef\xbb\xbfUTF-8", b"\xffUTF-8\xff",
                   "ＵＴＦ－８".encode(), "UTF−8".encode(), "UTF–8".encode(), "UTF‑8".encode(),
                   "UTF－8".encode(), "UTF-８".encode(), "UTF⁸".encode(), "UTF₈".encode(),
                   "Utf-8\u00a0".encode(), "\u200bUTF-8".encode(), "UTF-8\u2028".encode()]:
        add("is_utf8_charset", string(sample))
    charset_generator = random.Random(7137715)
    for _ in range(3000):
        add("is_utf8_charset", string(charset_generator.randbytes(charset_generator.randrange(65))))
    for _ in range(512):
        sample = "".join(chr(charset_generator.choice([0x20, 0x2D, 0x38, 0x55, 0x54, 0x46, 0x75, 0x74, 0x66,
                                                      0xA0, 0x178, 0x2010, 0x200B, 0x2028, 0xFF18, 0x1F980]))
                         for _ in range(charset_generator.randrange(17)))
        add("is_utf8_charset", string(sample.encode()))
    # Fixed UTF-8 site/no-mbstring profile; explicit strip bool only. The real
    # wrapper's empty/valid fast paths and invalid maximal-subpart handling are
    # compared without expanding PHP string coercion or changing site options.
    invalid_utf8_samples = [b"", b"ASCII\0\r\n\t\x7f", b"\xef\xbb\xbf", b"\xef\xbf\xbd",
                           b"\xc0\x80", b"\xc1\xbf", b"\xe0\x80\x80", b"\xed\xa0\x80",
                           b"\xf0\x80\x80\x80", b"\xf4\x90\x80\x80", b"\xf5\x80\x80\x80",
                           b"\xff\xff", b"\xe2\x82A\x80", b"\xf0\x9fA\x80\x80", b"\xed\xa0\x80\0\xe2\x82",
                           "日本語🦀\u2028\u2029\ufffe\U0010ffff".encode()]
    for codepoint in [0, 0x7F, 0x80, 0x7FF, 0x800, 0xD7FF, 0xE000, 0xFDD0, 0xFFFE, 0xFFFF,
                      0x10000, 0x1FFFF, 0x10FFFF]:
        raw = chr(codepoint).encode()
        invalid_utf8_samples.append(raw)
        for end in range(1, len(raw)):
            invalid_utf8_samples.extend([raw[:end], b"A" + raw[:end] + b"Z"])
    for byte in range(256):
        invalid_utf8_samples.extend([bytes([byte]), b"A" + bytes([byte]) + b"Z"])
        for raw in (b"\xc2\xa2", b"\xe2\x82\xac", b"\xf0\x9f\xa6\x80"):
            for offset in range(len(raw)):
                invalid_utf8_samples.append(raw[:offset] + bytes([byte]) + raw[offset + 1:])
    check_utf8_generator = random.Random(7131127)
    invalid_utf8_samples.extend(check_utf8_generator.randbytes(check_utf8_generator.randrange(257))
                                for _ in range(1500))
    for raw in invalid_utf8_samples:
        for strip in (False, True):
            add("wp_check_invalid_utf8", string(raw), strip=strip)
    # Fixed PCRE/C byte semantics, not RFC MIME parsing. The subtype class
    # contains the literal ASCII range & through ^, including slash/brackets.
    media_samples = [b"", b"application/json", b"APPLICATION/JSON", b"application/json+oembed",
                     b"application/problem+json", b"application/a+b+json", b"application/+json",
                     b"application/jsonp", b"application/json+oembedx", b"application/json+other",
                     b"application/json; charset=utf-8", b"text/plain,application/json",
                     b"garbage application/json garbage", b"garbageapplication/json",
                     b"application/json\0", b"\0application/json", b"\xff,application/json;\xff",
                     b"application/json\n", b"application/json\r\n", b"application/a/json+json",
                     b"application/\\[]^+json", b"application/%+json", b"application/`+json",
                     b"application/json! application/json;", b"application/json+bad,application/json+oembed",
                     b"application/json\0,application/problem+json", b"application/a+jsonx,application/json",
                     b"application/a+json+oembed;", b"application/json+oembed+oembed",
                     b"application/x,application/+json", b"application/x,application/++json",
                     b"application/x,application/a+json+oembed", b"application/x;application/+json",
                     b"application/x,application/json! application/json;",
                     "application/é+json".encode(), "application/json\u00a0".encode(),
                     "\u00a0application/json".encode(), "application/ＪＳＯＮ".encode()]
    for byte in range(256):
        unit = bytes([byte])
        media_samples.extend([unit + b"application/json", b"application/json" + unit,
                              b"application/" + unit + b"+json", b"noise" + unit + b"application/json",
                              b"application/json+oembed" + unit, b"application/" + unit + b"json",
                              b"application/json" + unit + b"application/json",
                              b"application/" + unit + b"++json+oembed;"])
    for left, right in itertools.product([b"", b" ", b"\t", b"\n", b"\v", b"\f", b"\r", b",", b";", b"\0", b"\x85", b"x"], repeat=2):
        media_samples.append(b"noise" + left + b"application/json" + right + b"tail")
    for prefix in [b"a", b"json", b"json+oembed", b"!#$", b"&/09:;=?@[\\]^", b"x+x", b"_", b"%", b"`", b"{", b"\xff"]:
        for suffix in [b"+json", b"+JSON", b"+json+oembed", b"++json", b"+jsonx", b"+json+other"]:
            for ending in [b"", b" ", b";x", b",application/json", b"\0"]:
                media_samples.append(b"application/" + prefix + suffix + ending)
    media_generator = random.Random(7131964)
    for _ in range(512):
        media_samples.append(bytes(byte + 32 if 65 <= byte <= 90 and media_generator.randrange(2) else byte
                                   for byte in b"APPLICATION/PROBLEM+JSON+OEMBED"))
    media_samples.extend(media_generator.randbytes(media_generator.randrange(257)) for _ in range(2000))
    atoms = [b"application/", b"json", b"+", b"+oembed", b"a", b"x", b" ", b";", b",", b"\n", b"\0", b"\xff", b"[\\]^", b"%", b"application/json"]
    for _ in range(2000):
        media_samples.append(b"".join(media_generator.choice(atoms) for _ in range(media_generator.randrange(25))))
    for raw in media_samples:
        add("wp_is_json_media_type", string(raw))
    # Source %s padding is byte-counted, including the sign and raw binary.
    # Width/output allowances are internal host limits, not PHP equivalence.
    zeroise_widths = [0, 1, 2, 3, 4, 8, 20, 64, 4095, 4096]
    for number in [-(2**63), -(2**63) + 1, -100, -10, -1, 0, 1, 9, 10, 100, 2**63 - 1]:
        for width in zeroise_widths:
            add("zeroise", integer(number), width=width)
    for raw in [b"", b"0", b"-1", b"+1", b"00001", b"1.5", b"1e3", b" 1 ", b"\t\n", b"a\0b",
                b"\0", b"\xff", b"\xc0\xaf", b"-000", b"0x10", "é".encode(), "日本語🦀".encode()]:
        for width in zeroise_widths:
            add("zeroise", string(raw), width=width)
    for byte in range(256):
        for width in [0, 1, 2, 3, 8]:
            add("zeroise", string(bytes([byte])), width=width)
    zeroise_generator = random.Random(7132792)
    for _ in range(600):
        add("zeroise", string(zeroise_generator.randbytes(zeroise_generator.randrange(257))),
            width=zeroise_generator.randrange(257))
        add("zeroise", integer(zeroise_generator.randrange(-(2**63), 2**63)), width=zeroise_generator.randrange(65))
    for length in [4095, 4096, 4097]:
        for width in [0, 4095, 4096]:
            add("zeroise", string(b"x" * length), width=width)
    # Fixed default-unit decimal grammar only; the actual WP function handles
    # broader casts/contains matching that this explicit subset rejects.
    hr_suffixes = [(b"", 1), (b"k", 1024), (b"K", 1024), (b"m", 1024**2),
                   (b"M", 1024**2), (b"g", 1024**3), (b"G", 1024**3)]
    for suffix, factor in hr_suffixes:
        boundary = (2**63 - 1) // factor
        values = {0, 1, 9, 10, 1023, 1024, 2**63 - 1, 2**63, 2**64 - 1, 10**50 - 1}
        values.update(max(0, boundary + delta) for delta in range(-3, 4))
        for value in sorted(values):
            for leading in (b"", b"0", b"000000"):
                add("wp_convert_hr_to_bytes", string(leading + str(value).encode() + suffix))
        for length in (32, 256):
            add("wp_convert_hr_to_bytes", string(b"9" * length + suffix))
        # The old unrestricted saturation hypothesis failed for 4096 nines:
        # PHP returns 0. Those seven cases are retained as protocol refusals in
        # target/hr-bytes-significant-domain-refusals.json, not equivalence cases.
        for length in (32, 256, 4096):
            add("wp_convert_hr_to_bytes", string(b"0" * length + b"1" + suffix))
            add("wp_convert_hr_to_bytes", string(b"0" * length + suffix))
        for tail in (str(2**63 - 1).encode(), b"9" * 256):
            add("wp_convert_hr_to_bytes", string(b"0" * 4096 + tail + suffix))
    hr_generator = random.Random(7131687)
    for _ in range(2400):
        length = hr_generator.randrange(1, 129)
        digits = bytes(hr_generator.randrange(48, 58) for _ in range(length))
        suffix, _ = hr_generator.choice(hr_suffixes)
        add("wp_convert_hr_to_bytes", string(digits + suffix))
    # $_SERVER is a per-case context, not an HTTP adapter. Missing and explicit
    # null both model isset=false; the PHP fixture still sets them distinctly.
    missing_header = object()

    def media_request(accept=missing_header, content_type=missing_header):
        fields = {}
        for name, value in (("accept", accept), ("content_type", content_type)):
            if value is not missing_header:
                fields[name] = {"type": "null"} if value is None else string(value)
        for api in ("wp_is_json_request", "wp_is_xml_request"):
            add(api, {"type": "null"}, **fields)

    request_states = [missing_header, None, b"", b"application/json", b"application/problem+json",
                      b"APPLICATION/JSON", b"application/json;q=0", b"application/json+oembed",
                      b"text/xml", b"application/rss+xml", b"application/atom+xml", b"application/rdf+xml",
                      b"text/xml+oembed", b"application/xml+oembed", b"application/xml", b"TEXT/XML",
                      b"text/xml; charset=utf-8", b" text/xml ", b"prefixtext/xmlsuffix", b"text/xml\0",
                      b"\xfftext/xml\xff", b"\0application/json", b"application/json\0", b"text/plain,application/json",
                      b"text/xml;q=0,application/json;q=0", b"\xff,application/a+json;\xff"]
    for accept, content_type in itertools.product(request_states, repeat=2):
        media_request(accept, content_type)
    for byte in range(256):
        unit = bytes([byte])
        media_request(b"application/json" + unit, None)
        media_request(unit + b"text/xml", missing_header)
        media_request(b"x" + unit + b"application/json", b"text/xml")
        media_request(missing_header, b"text/xml" + unit)
        media_request(None, b"application/" + unit + b"+json")
        media_request(b"application/json" + unit, b"application/rss+xml")
    for literal in [b"text/xml", b"application/rss+xml", b"application/atom+xml", b"application/rdf+xml",
                    b"text/xml+oembed", b"application/xml+oembed"]:
        for variant in [literal, literal.upper(), b"X" + literal + b"Y", literal + b";x=1",
                        b"\0" + literal + b"\xff", literal + b"\0", literal + b";q=0"]:
            media_request(variant)
            media_request(content_type=variant)
            media_request(variant, b"application/json")
    request_generator = random.Random(71319171982)
    for _ in range(1500):
        def random_header():
            if request_generator.randrange(3):
                return request_generator.choice(request_states)
            if request_generator.randrange(2):
                return request_generator.randbytes(request_generator.randrange(129))
            return b"".join(request_generator.choice(atoms + [b"text/xml", b"application/rss+xml", b"application/xml"])
                            for _ in range(request_generator.randrange(17)))
        media_request(random_header(), random_header())
    # Alternating true and empty/missing/null contexts exposes $_SERVER leakage.
    for _ in range(32):
        media_request(b"application/json", b"text/xml")
        media_request()
        media_request(None, None)
        media_request(b"", b"")
    filter_values = scalars + [string(b""), string(b"\0\xff"), array([]), array([integer(-2), string(b"01")])]
    for value in filter_values:
        add("sanitize_key", string(b"RAW_!\0\xff"), "constant", filter_return=value)
        for day in (29, 30):
            add("wp_checkdate", string(b"raw\0\xff"), "constant", month="2", day=str(day), year="2024", filter_return=value)
    basename_paths = [b"", b"/", b"////\\", b".", b"..", b"C:", b"C:\\file.txt", b"/file.txt/", b"/x\xff", b"/x ", b"/x+", b"/percent%2F", "日本語/🦀.txt".encode()]
    basename_paths += strings
    basename_paths += [b"/prefix/" + value + b"/\\" for value in strings]
    for path in basename_paths:
        for suffix in [b"", b".txt", b"txt", b"+", b"%FF", b"FF", b"F", b"%00", b"\0", b"\xff", path]:
            add("wp_basename", string(path), suffix=string(suffix))
    for byte in range(256):
        add("wp_basename", string(b"/tail/bytes\0\xff.txt"), suffix=string(bytes([byte])))
    utf8_samples = list(strings)
    points = [0, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000, 0xfeff, 0x10000, 0x10ffff]
    points += list(range(0xfdd0, 0xfdf0)) + [plane * 65536 + low for plane in range(17) for low in (65534, 65535)]
    for point in points:
        encoded = chr(point).encode()
        utf8_samples += [encoded, b"prefix" + encoded + b"suffix", b"\xff" + encoded + b"\xff"]
        utf8_samples += [encoded[:cut] for cut in range(len(encoded))]
        for offset in range(len(encoded)):
            for byte in (0, 0x7f, 0x80, 0xbf, 0xc0, 0xed, 0xf4, 0xff):
                utf8_samples.append(encoded[:offset] + bytes([byte]) + encoded[offset+1:])
    for sample in utf8_samples:
        for api in ("wp_is_valid_utf8", "wp_scrub_utf8", "wp_has_noncharacters"):
            add(api, string(sample))
    for first in range(256):
        for second in range(256):
            sample = bytes([first, second])
            add("wp_is_valid_utf8", string(sample))
            if first >= 0xc0:
                add("wp_scrub_utf8", string(sample))
                add("wp_has_noncharacters", string(sample))
    queries = [b"", b"&", b"&&a=b&&", b"bare", b"=ignored", b" =ignored", b"a=1&a=2&b=3", b"a.b=1&a+b=2", b" a =x", b"\ta=x", b"a=%00%ff+%2B", b"0=a&00=b&-0=c&-1=d&9223372036854775808=e", b"?a=x#value", b"a=x;y=z", b"a=%&b=%2&c=%GG&d=%41", b"a=b=c"]
    queries += [f"k%{byte:02X}=value".encode() for byte in range(1,256) if byte not in (91,93)]
    queries += [f"key=%{byte:02X}".encode() for byte in range(256)]
    queries += [b"key=" + b"".join(f"%{byte:02X}".encode() for byte in generator.randbytes(generator.randrange(128))) for _ in range(100)]
    queries += [b"&".join(f"key{index}=value".encode() for index in range(count)) for count in (999,1000)]
    queries += [b"a[]=x&a[]=y", b"a[-5]=x&a[]=y&a[01]=z&a[x.y]=dot&a[]=last", b"a=x&a[b]=y&a[b][]=z&a[]=tail", b"a[]=x&a=y", b"a[][b]=x&a[][c]=y", b"a[0]=x&a[0][b]=y", b"0[]=x&0[]=y&-1[a]=z", b"a[%FF]=%00%FF", b"a[%2B1]=x&a[1]=y", b"a[9223372036854775807]=max", b"a[-9223372036854775808]=min&a[]=next"]
    queries += [("a" + "[x]" * depth + "=value").encode() for depth in (1,2,16,32,63,64)]
    for _ in range(400):
        parts=[]
        for _ in range(generator.randrange(1,16)):
            base=generator.choice([b"a", b"b", b"a.b", b"0", b"-1", b"01", "日本語".encode()])
            path=quote_from_bytes(base,safe="").encode()
            for _ in range(generator.randrange(5)):
                key=generator.choice([b"",b"x",b"y",b"x.y",b"-5",b"-1",b"0",b"7",b"01",b"+1",b"\xff", "🦀".encode()])
                path += b"["+quote_from_bytes(key,safe="").encode()+b"]"
            value=generator.randbytes(generator.randrange(32))
            parts.append(path+b"="+quote_from_bytes(value,safe="").encode())
        queries.append(b"&".join(parts))
    defaults_values = [array([]), {"type":"array","entries":[{"key":string(b"a"),"value":string(b"default")},{"key":integer(7),"value":string(b"number")}] }]
    for query in queries:
        add("wp_parse_str", string(query))
        for defaults in defaults_values:
            add("wp_parse_args", string(query), defaults=defaults)
    for value in filter_values:
        add("wp_parse_str", string(b"a=b"), "constant", filter_return=value)
        add("wp_parse_args", string(b"a=b"), "constant", filter_return=value, defaults=array([]))
        if value["type"] == "array":
            add("wp_parse_args", string(b"a=b"), "constant", filter_return=value, defaults=defaults_values[1])
    uuid_base = b"123e4567-e89b-42d3-a456-426614174000"
    uuid_samples = [uuid_base, b"00000000-0000-0000-0000-000000000000", b"", uuid_base.upper(), uuid_base+b"\n", uuid_base+b"\r\n", uuid_base+b"\n\n", uuid_base+b" ", uuid_base+b"\0"]
    for offset in range(36):
        for byte in range(256):
            uuid_samples.append(uuid_base[:offset]+bytes([byte])+uuid_base[offset+1:])
    for _ in range(200):
        raw=generator.randbytes(16).hex().encode()
        uuid_samples.append(b"-".join([raw[:8],raw[8:12],raw[12:16],raw[16:20],raw[20:]]))
    for sample in uuid_samples:
        for version in (None,"4"):
            add("wp_is_uuid",string(sample),version=version)
    for value in scalars + [array([]),array([integer(1)])]:
        for version in (None,"4","5"):
            add("wp_is_uuid",value,version=version)
    jsonp_samples = [b"", b".", b"...", b"1callback", b"_a.B09", b"a..b", b".callback.", b"alert(1)", b"a[0]", b"a$", b"a-b", b"callback\n", b"callback\r\n", "日本語".encode(), b"\xef\xbb\xbfcallback"]
    for byte in range(256):
        jsonp_samples += [bytes([byte]), b"a"+bytes([byte]), bytes([byte])+b"b", b"a"+bytes([byte])+b"b"]
    jsonp_generator = random.Random(4699)
    jsonp_samples += [jsonp_generator.randbytes(jsonp_generator.randrange(129)) for _ in range(1000)]
    for sample in jsonp_samples:
        add("wp_check_jsonp_callback",string(sample))
    for value in scalars + [array([]),array([integer(1)])]:
        add("wp_check_jsonp_callback",value)
    # Exhaust short combinations: trim's NUL/VT domain differs from the two
    # collapse patterns, and CR replacement occurs before collapsing LF runs.
    whitespace_alphabet = [0, 9, 10, 11, 12, 13, 32, 97, 255]
    for length in range(5):
        for parts in itertools.product(whitespace_alphabet, repeat=length):
            add("normalize_whitespace", string(bytes(parts)))
    for byte in range(256):
        for sample in [bytes([byte]), b" \t"+bytes([byte])+b"\r\n", b"a"+bytes([byte])+b"b"]:
            add("normalize_whitespace", string(sample))
    whitespace_generator = random.Random(5580)
    for _ in range(1000):
        add("normalize_whitespace", string(whitespace_generator.randbytes(whitespace_generator.randrange(257))))
    for sample in [b" \r\n\t\0", b"a \t\r\n \t b", b"a\n \n\tb", b"\x0c \t a \x0c",
                   "\u00a0 \t日本語\r\n\u2003".encode(), b"\xff\0\x0b \t\xfe"]:
        add("normalize_whitespace", string(sample))
    # WP splits its own byte regex, including incomplete markup and quotes.
    # Preserve empty captures instead of substituting an HTML parser.
    for length in range(5):
        for parts in itertools.product(b"<>!-[]a\0", repeat=length):
            add("wp_html_split", string(bytes(parts)))
    for prefix, terminator in [(b"<!--",b"-->"),(b"<![CDATA[",b"]]>"),(b"<tag ",b">")]:
        for byte in range(256):
            for sample in [b"left"+prefix+bytes([byte])+terminator+b"right",
                           prefix+bytes([byte])+b"<inside>"+terminator,
                           b"a"+prefix+terminator[:-1]+bytes([byte])+b"tail"]:
                add("wp_html_split", string(sample))
    html_generator = random.Random(626)
    html_atoms = [b"<", b">", b"!", b"-", b"-->", b"<!--", b"<![CDATA[", b"]]>",
                  b"<![cdata[", b"\0", b"\xff", b"a", b"'", b'"', b" ", b"\r\n"]
    for _ in range(2000):
        add("wp_html_split", string(b"".join(html_generator.choice(html_atoms) for _ in range(html_generator.randrange(33)))))
    for _ in range(1000):
        add("wp_html_split", string(html_generator.randbytes(html_generator.randrange(257))))
    for sample in [b"", b"<>", b"<a><b>", b"text<!-- x > <y> -->tail", b"<!-->", b"<!--->x", b"<!---->x",
                   b"<a title='>'>body</a>", b"<![CDATA[>x<y>]]>tail", b"<![CDATA[]]]>x",
                   b"<![CDATA[unterminated <b>", b"<!--unterminated > <b>", b"a<<b>c", b"<", b"<!--"]:
        add("wp_html_split", string(sample))

    def html_replace(sample, pairs):
        replacements = {"type":"array", "entries":[{"key":string(key), "value":string(value)} for key, value in pairs]}
        add("wp_replace_in_html_tags", string(sample), replacements=replacements)

    replacement_maps = [[], [(b"", b"ignored")], [(b"a", b"aa")], [(b"a", b"b"), (b"b", b"c")],
                        [(b"a", b"X"), (b"ab", b"Y")], [(b"ab", b"Y"), (b"a", b"X")],
                        [(b"aa", b"X"), (b"aaa", b"Y")], [(b"<", b""), (b">", b"<a>")],
                        [(b"\0", b"\xff"), (b"\xff", b"\0")], [(b"--", b"]]"), (b"]]", b"--")]]
    for length in range(6):
        for parts in itertools.product(b"ab<>", repeat=length):
            for pairs in replacement_maps[:6]:
                html_replace(bytes(parts), pairs)
    replacement_samples = [b"", b"ababa", b"<ababa>ababa", b"<aaaa>", b"<a>a", b"<a><b>",
                           b"<!-- ababa > <a> -->ababa", b"<![CDATA[ababa<a>]]>ababa", b"<!-->a",
                           b"<a title='>'>ababa</a>", b"ab<a", b"<a\0\xff>a\0\xff", b"<![CDATA[ababa"]
    for sample in replacement_samples:
        for pairs in replacement_maps:
            html_replace(sample, pairs)
    for byte in range(256):
        for pairs in [replacement_maps[2], replacement_maps[3], replacement_maps[4], replacement_maps[8]]:
            html_replace(b"a"+bytes([byte])+b"<a"+bytes([byte])+b"b>a"+bytes([byte]), pairs)
    replacement_generator = random.Random(757)
    for _ in range(1000):
        sample = b"".join(replacement_generator.choice(html_atoms) for _ in range(replacement_generator.randrange(25)))
        pairs = replacement_generator.choice(replacement_maps)
        html_replace(sample, pairs)
    relative_prefixes = [b"http://", b"https://", b"HTTP://", b"HtTpS://", b"//", b"ftp://", b"http:/", b"https:////", b"", b" http://", b"\n//", b"HTTPs://"]
    authorities = [b"host", b"", b"a?b", b"u:p@h:80", b"\0", b"\n", b"\xff", b"host\nx", b"[::1]", b"?x=", b"#f"]
    relative_tails = [b"", b"/", b"/a", b"/a?x=y#z", b"/a\nrest", b"\n/a", b"?x=/a", b"#f/a", b"/\0\xff", b"\\windows"]
    for prefix, authority, tail in itertools.product(relative_prefixes, authorities, relative_tails):
        add("wp_make_link_relative", string(prefix+authority+tail))
    for byte in range(256):
        for sample in [b"https://"+bytes([byte])+b"/path", b"//host/"+bytes([byte])+b"tail",
                       b"http://host"+bytes([byte]), b"http://host/a"+bytes([byte])+b"b"]:
            add("wp_make_link_relative", string(sample))
    for prefix in [b"http://", b"https://"]:
        for index in range(len(prefix)):
            for byte in range(256):
                add("wp_make_link_relative", string(prefix[:index]+bytes([byte])+prefix[index+1:]+b"host/path\nend"))
    url_generator = random.Random(4905)
    for _ in range(1000):
        add("wp_make_link_relative", string(url_generator.choice(relative_prefixes)+url_generator.randbytes(url_generator.randrange(129))))
    shorten_samples = [b"", b"https://www.example.test/", b"HTTP://WWW.EXAMPLE/", b"www.www.example/\\",
                       b"http://x/https://y/www.z", b"hhttp://ttps://x", b"wwwww.w.example", b"wwwhttp://.example",
                       b"a"*35, b"a"*36, b"a"*37, b"a\0b\xffc", "https://例え.test/日本語🦀".encode(),
                       b"///\\", b"plain/path?url=https://www.example/", b"http://http://www.www.test"]
    for sample in shorten_samples:
        for length in [3,4,5,8,10,35,36,128,2**63-1]:
            add("url_shorten",string(sample),length=str(length))
    for byte in range(256):
        for length in [3,4,8,35]:
            add("url_shorten",string(b"https://www.a"+bytes([byte])+b"b/\\"),length=str(length))
    shorten_generator = random.Random(6318)
    for _ in range(1000):
        sample = shorten_generator.choice(shorten_samples)+shorten_generator.randbytes(shorten_generator.randrange(129))
        for length in [3,35,shorten_generator.randrange(3,129)]:
            add("url_shorten",string(sample),length=str(length))
    for sample in shorten_samples:
        for length in [-(2**63)+offset for offset in range(5)]+[-100,-4,-3,-2,-1,0,1,2]:
            add("url_shorten",string(sample),length=str(length))
    for byte in range(256):
        for length in [-1,0,1,2]:
            add("url_shorten",string(b"ab"+bytes([byte])+b"cd"),length=str(length))
    for _ in range(250):
        add("url_shorten",string(shorten_generator.randbytes(shorten_generator.randrange(129))),length=str(shorten_generator.randrange(-256,3)))

    def colors(sample):
        for api in ["sanitize_hex_color", "sanitize_hex_color_no_hash", "maybe_hash_hex_color"]:
            add(api,string(sample))

    for sample in [b"", b"#", b"###", b"0", b"000", b"#000", b"###aBc", b"abc", b"a1B2c3", b"#AbC",
                   b"#abcd", b"#12345678", b"red", b"rgba(0,0,0,1)", b" #abc", b"#abc ", b"#abc\0",
                   b"#abc\n", b"#abc\n\n", b"#abc\r\n", b"#abc\r", b"###abc\n", b"#a1B2c3\n", b"\xff"]:
        colors(sample)
    for base in [b"#abc",b"#a1B2c3"]:
        for index in range(len(base)):
            for byte in range(256):
                colors(base[:index]+bytes([byte])+base[index+1:])
    for parts in itertools.product(b"0123456789abcdef",repeat=3):
        colors(b"#"+bytes(parts))
    color_generator = random.Random(6339)
    for _ in range(1000):
        digits = bytes(color_generator.choice(b"0123456789aAbBcCdDeEfF") for _ in range(6))
        colors(color_generator.choice([b"",b"#",b"###"])+digits+color_generator.choice([b"",b"\n",b"\r\n"]))
    for _ in range(1000):
        colors(color_generator.randbytes(color_generator.randrange(33)))

    def html_class(sample, fallback, mode=None, **extra):
        add("sanitize_html_class",string(sample),mode,fallback=string(fallback),**extra)

    for sample, fallback in itertools.product(
            [b"", b"0", b"00", b"!!!", b"%41", b"%2541", b"%4%3132", b"space class", b"A.B_C-09", "日本語".encode(), b"\0\xff"],
            [b"", b"0", b"00", b"valid-Fallback", b"A.b", b"!!!", b"%41", b"\0\xff"]):
        for mode in [None,"class_arguments"]:
            html_class(sample,fallback,mode)
    for byte in range(256):
        for sample in [bytes([byte]), b"a"+bytes([byte])+b"b", b"%"+bytes([byte])+b"A", b"%A"+bytes([byte])]:
            for fallback in [b"0",b"_fallback"]:
                for mode in [None,"class_arguments"]:
                    html_class(sample,fallback,mode)
    for high, low in itertools.product(b"0123456789aAbBcCdDeEfF",repeat=2):
        for mode in [None,"class_arguments"]:
            html_class(b"left%"+bytes([high,low])+b"%2541right",b"A.b",mode)
    class_generator = random.Random(2438)
    for _ in range(1000):
        sample = class_generator.randbytes(class_generator.randrange(129))
        fallback = class_generator.choice([b"",b"0",b"00",b"A.b",b"!!!",b"%41"])
        for mode in [None,"class_arguments"]:
            html_class(sample,fallback,mode)
    for value in [{"type":"null"},{"type":"bool","value":False},integer(0),string(b"0"),array([integer(7)])]:
        for sample,fallback in [(b"a-b",b""),(b"!!!",b"A.b"),(b"",b"0")]:
            html_class(sample,fallback,"constant",filter_return=value)

    def form_attributes(helper, current):
        for api in ["checked","selected","disabled","wp_readonly"]:
            add(api,helper,current=current,display=False)

    form_scalars = [{"type":"null"},{"type":"bool","value":False},{"type":"bool","value":True}]
    form_scalars += [integer(value) for value in [-(2**63),-1,0,1,2**63-1]]
    form_scalars += [string(value) for value in [b"",b"0",b"1",b"01",b"true",b"false",b"-9223372036854775808",b"9223372036854775807",b"\0\xff"]]
    for helper,current in itertools.product(form_scalars,repeat=2):
        form_attributes(helper,current)
    for byte in range(256):
        for helper,current in [(bytes([byte]),bytes([byte])),(bytes([byte]),b""),
                               (b"a"+bytes([byte])+b"b",b"a"+bytes([byte])+b"b"),
                               (bytes([byte]),bytes([(byte+1)%256]))]:
            form_attributes(string(helper),string(current))
    form_generator = random.Random(5597)
    for _ in range(1000):
        value = form_generator.randrange(-(2**63),2**63)
        form_attributes(integer(value),string(str(value).encode()))
        form_attributes(integer(value),string(b"0"+str(value).encode()))
        raw = form_generator.randbytes(form_generator.randrange(129))
        form_attributes(string(raw),string(raw))
    for api,seed in [("sanitize_locale_name",2468),("sanitize_mime_type",5800)]:
        samples=[b"",b"en_US",b"en-US",b"pt_BR.UTF-8",b"A.B_C-09+*",b"text/html; charset=utf-8",
                 b"*/*",b"text//html",b"application/vnd.example+json",b"TEXT/HTML",b"%41",b"a\0b",b"a\xffb",
                 "日本語".encode(),b" ../\\ ",b"x\r\ny"]
        for byte in range(256):
            samples += [bytes([byte]),b"a"+bytes([byte]),bytes([byte])+b"b",b"a"+bytes([byte])+b"b"]
        generator2=random.Random(seed)
        samples += [generator2.randbytes(generator2.randrange(129)) for _ in range(1000)]
        for sample in samples:
            add(api,string(sample))
            add(api,string(sample),"append_raw")
        for value in [{"type":"null"},{"type":"bool","value":False},integer(42),string(b"0"),array([integer(7)])]:
            for sample in [b"",b"A.B_C-09+*",b"\0\xff"]:
                add(api,string(sample),"constant",filter_return=value)
    email_samples=[b"",b"a@b.c",b"a@b.cd",b"abcdef",b"@a.bc",b"@abc@d.ef",b"aa@@b.cd",
                   b"a..b@example.test",b".a.@b.cd",b"a\n@b.cd",b"a\n\n@b.cd",b"a\r\n@b.cd",
                   b"a@b..cd",b"a@.b.cd",b"a@b.cd.",b"a@localhost",b"a@-b.cd",b"a@b-.cd",
                   b"a@b.c_d",b"a@b.c\nd",b"a@b.cd\n",b"a@b.\vcd",b"a@b.\fcd",b"a@b.0",
                   b"A+B!#$%&'*-/=?^_`{|}~@EXAMPLE.TEST",b"a@xn--bcher-kva.example",
                   "a@日本語.example".encode(),b"a"*65+b"@"+b"b"*64+b".c"]
    for byte in range(256):
        unit=bytes([byte])
        email_samples += [unit+b"@b.cd",b"a"+unit+b"@b.cd",b"a"+unit+b"b@c.de",
                          b"a@"+unit+b"b.cd",b"a@b"+unit+b".cd",b"a@b.c"+unit,
                          b"a@b."+unit+b"cd",b"a@b.c"+unit+b"d"]
    email_generator=random.Random(3613)
    for _ in range(1500):
        raw=bytearray(b"local@example.test")
        for _ in range(email_generator.randrange(1,5)):
            raw[email_generator.randrange(len(raw))]=email_generator.randrange(256)
        email_samples.append(bytes(raw))
    email_samples += [email_generator.randbytes(email_generator.randrange(129)) for _ in range(500)]
    for sample in email_samples:
        add("is_email",string(sample))
        add("is_email",string(sample),"email_arguments")
    for value in [{"type":"null"},{"type":"bool","value":False},integer(42),string(b"0"),array([integer(7)])]:
        for sample in [b"",b"abcdef",b"a@b.cd",b"a\n@b.cd",b"a@b..cd"]:
            add("is_email",string(sample),"constant",filter_return=value)
    # These two APIs intentionally retain state through the entire input batch,
    # matching PHP function statics and one native UniqueIds context per process.
    unique_prefixes=[b"",b"a",b"a1",b"a",b"",b"1",b"01",b"0",b"-0",b"-1",b"+1",
                     b"A",b"a",b"p-",b"\0",b"\xff",b"p\0\xff",b" ",b"\n", "日本語".encode()]
    unique_prefixes += [bytes([byte]) for byte in range(256)]
    unique_generator=random.Random(8156)
    pool=unique_prefixes.copy()
    for _ in range(1500):
        unique_prefixes.append(unique_generator.choice(pool) if unique_generator.randrange(2)
                               else unique_generator.randbytes(unique_generator.randrange(129)))
    unique_prefixes += [b"a"]*25 + [b"a1"]*12 + [b"",b"p-",b"0",b"01"]*64
    for prefix in unique_prefixes:
        add("wp_unique_id",string(prefix))
        add("wp_unique_prefixed_id",string(prefix))
    key_sets = [[], [integer(-5), integer(99)], [integer(1), string(b"1")], [integer(0), integer(2)]]
    key_sets += [[string(value)] for value in [b"0", b"-0", b"01", b"+1", b" 1", b"1 ", b"1.0", b"1e2", b"-9223372036854775808", b"9223372036854775807", b"9223372036854775808", b"\xff"]]
    for keys in key_sets:
        add("wp_validate_boolean", array(keys))
        add("wp_is_numeric_array", array(keys))
    # PCRE's byte-mode whitespace/comma splitting and scalar array filtering.
    for value in strings + [b"a,b c\td\ne\vf\fg\rh", b" a,,b ", b"a\xc2\xa0b\x85c", b"a\0b"]:
        add("wp_parse_list", string(value))

    def values_array(values):
        return {"type": "array", "entries": [{"key": integer(index), "value": value} for index, value in enumerate(values)]}

    json_generator=random.Random(4443)
    json_strings=[b"",b"/",b"\"\\/",bytes(range(128)),b"<>&'\"/",b"0",b"01",b"-0",b"1e0",b"NaN"]
    json_strings += [chr(value).encode("utf-8") for value in range(256)]
    json_strings += [chr(value).encode("utf-8") for value in [0x7ff,0x800,0xd7ff,0xe000,0xfffe,0xffff,0x10000,0x10001,0x103ff,0x10400,0x1f980,0x10ffff,0x2028,0x2029]]
    for _ in range(3000):
        point=json_generator.randrange(0x110000)
        while 0xd800 <= point <= 0xdfff:
            point=json_generator.randrange(0x110000)
        json_strings.append(b"<\"/"+chr(point).encode("utf-8")+b"\\&\0")
    json_leaves=[{"type":"null"},{"type":"bool","value":False},{"type":"bool","value":True}]
    json_leaves += [integer(value) for value in [-(2**63),-999,-1,0,1,999,2**63-1]]
    json_leaves += [string(value) for value in json_strings]
    for value in json_leaves:
        add("wp_json_encode",value)
    json_keys=[integer(value) for value in [-(2**63),-1,0,1,2,2**63-1]]
    json_keys += [string(value) for value in [b"",b"0",b"00",b"01",b"-0",b"+0",b"-01",b"1e0",b"x",b"\0",b"a/b",b"\"",b"9223372036854775808","日本語".encode()]]
    def json_value(depth):
        if depth==0 or json_generator.randrange(3)==0:
            return json_generator.choice(json_leaves)
        if json_generator.randrange(2):
            return values_array([json_value(depth-1) for _ in range(json_generator.randrange(7))])
        return {"type":"array","entries":[{"key":json_generator.choice(json_keys),"value":json_value(depth-1)} for _ in range(json_generator.randrange(7))]}
    for keys in [[],[integer(0),integer(1)],[integer(1),integer(0)],[integer(0),integer(2)],
                 [string(b"0"),integer(1),integer(0)],json_keys,list(reversed(json_keys))]:
        data=array(keys)
        for entry in data["entries"]:
            entry["value"]=json_generator.choice(json_leaves)
        add("wp_json_encode",data)
    for _ in range(2000):
        add("wp_json_encode",json_value(4))
    # The JSON transport itself is bounded at 128 nesting levels; deeper host
    # limits are tested directly in Rust and a separate PHP exploration.
    for depth in [1,2,3,7,16,24]:
        data=integer(42)
        for _ in range(depth):
            data=values_array([data])
        add("wp_json_encode",data)
        for flags in [128,144,2527]:
            add("wp_json_encode",data,flags=flags)

    flag_bits=list(registry_tool.JSON_ENCODE_FLAGS.values())
    flag_combinations=[sum(flag for offset,flag in enumerate(flag_bits) if choice & (1 << offset))
                       for choice in range(1 << len(flag_bits))]
    flag_text=bytes(range(128))+"é語🦀\u2028\u2029\u0085\uffff\U0010ffff".encode()
    flag_object={"type":"array","entries":[
        {"key":string(flag_text),"value":string(flag_text)},
        {"key":integer(0),"value":string(b"/\"<>&'")},
        {"key":integer(2),"value":values_array([])}]}
    flag_values=[string(flag_text),values_array([]),values_array([string(flag_text),integer(42),{"type":"null"},values_array([])]),
                 flag_object,values_array([values_array([]),values_array([integer(1),integer(2)]),flag_object]),
                 integer(-(2**63)),integer(2**63-1),{"type":"null"},{"type":"bool","value":False},{"type":"bool","value":True}]
    for flags in flag_combinations:
        for value in flag_values:
            add("wp_json_encode",value,flags=flags)
    for point in range(256):
        raw=chr(point).encode()
        for flags in [0,15,16,64,128,144,256,2048,2304,2399,2527]:
            add("wp_json_encode",string(raw),flags=flags)
            add("wp_json_encode",{"type":"array","entries":[{"key":string(raw),"value":string(raw)}]},flags=flags)
    flag_generator=random.Random(2399)
    for _ in range(1000):
        add("wp_json_encode",json_value(4),flags=flag_generator.choice(flag_combinations))

    # Explicit UTF-8 blog_charset / no mb_convert_encoding profile. This is
    # WordPress's whole-graph retry, not JSON_INVALID_UTF8_SUBSTITUTE.
    def fallback_case(value, flags=0):
        add("wp_json_encode", value, flags=flags, utf8_profile="utf8_no_mbstring")

    def pairs(entries):
        return {"type": "array", "entries": [{"key": key, "value": value} for key, value in entries]}

    replacement = string("\ufffd".encode())
    collision = pairs([(string(b"\xff"), string(b"first")),
                       (string(b"middle"), {"type": "bool", "value": True}),
                       (string(b"\xfe"), string(b"last"))])
    literal_collision = pairs([(replacement, string(b"first")),
                               (string(b"\xff"), string(b"last"))])
    retry_samples = [string(b"\xff<>&'\"/\0"), collision, literal_collision,
                     values_array([string(b"valid"), string(b"\xed\xa0\x80"), collision]),
                     pairs([(string(b"0"), string(b"\xff")), (integer(1), array([]))]),
                     values_array([array([]), values_array([string(b"\xf4\x90\x80\x80")])])]
    for flags in flag_combinations:
        for value in retry_samples:
            fallback_case(value, flags)
    # Byte boundaries, maximal ill-formed subparts, BOM/noncharacters, and
    # already-valid first-pass values exercise both paths of this profile.
    fallback_bytes = [bytes([byte]) for byte in range(256)]
    fallback_bytes += [b"", b"\xc0\xaf", b"\xe0\x80\xaf", b"\xed\xa0\x80",
                       b"\xf0\x80\x80\xaf", b"\xf4\x90\x80\x80", b"\xe2\x82",
                       b"\xf0\x9f\x98", b"\xef\xbb\xbf", b"\xef\xbf\xbe", b"\xff\x80A"]
    for raw in fallback_bytes:
        for flags in (0, 128, 256, 2527):
            fallback_case(string(raw), flags)
            fallback_case(pairs([(string(raw), string(raw))]), flags)
    fallback_rng = random.Random(4475)
    for _ in range(1000):
        raw = fallback_rng.randbytes(fallback_rng.randrange(129))
        fallback_case(pairs([(string(raw), string(raw)),
                             (string(b"stable"), values_array([string(raw), collision]))]),
                      fallback_rng.choice(flag_combinations))
    for depth in range(1, 25):
        value = string(b"\xe2\x82")
        for _ in range(depth):
            value = values_array([value])
        for flags in (0, 128, 2527):
            fallback_case(value, flags)
    for value in json_leaves[:10] + [array([]), values_array([string(b"valid")])]:
        fallback_case(value)

    pool = scalars + [string(value) for value in [b"", b"a,b", b"\0\xff"]] + [array([]), values_array([string(b"nested")])]
    for index, keys in enumerate(key_sets):
        data = array(keys)
        for item, entry in enumerate(data["entries"]):
            entry["value"] = pool[(index + item) % len(pool)]
        add("wp_parse_list", data)
        for requested in [[], keys, list(reversed(keys)), keys + keys + [string(b"missing")]]:
            add("wp_array_slice_assoc", data, keys=values_array(requested))
    key_pool = [integer(value) for value in [-(2**63), -7, 0, 1, 9, 2**63-1]]
    key_pool += [string(value) for value in [b"", b"0", b"01", b"-0", b"+1", b"x", b"null", b"\0\xff", b"9223372036854775808"]]
    for _ in range(250):
        keys = generator.choices(key_pool, k=generator.randrange(20))
        data = array(keys)
        for entry in data["entries"]:
            entry["value"] = generator.choice(pool)
        add("wp_parse_list", data)
        requested = generator.choices(key_pool, k=generator.randrange(25))
        add("wp_array_slice_assoc", data, keys=values_array(requested))
        defaults = array(generator.choices(key_pool, k=generator.randrange(20)))
        for entry in defaults["entries"]:
            entry["value"] = generator.choice(pool)
        add("wp_parse_args", data, defaults=defaults)
        add("wp_parse_args", data, defaults=array([]))
    slash_strings = strings + [b"\\" + bytes([byte]) for byte in range(256)]
    slash_strings += [b"'\"\\\0", b"ends\\", b"\\0", b"\\\\0", bytes(range(256))]
    slash_values = scalars + [string(value) for value in slash_strings]
    slash_values += [values_array(pool), values_array([values_array(pool)])]
    for value in slash_values:
        add("wp_slash", value)
        add("wp_unslash", value)
    # WP_List_Util handles duplicate indices, isset(null), canonical keys and
    # PHP 8.3+ appends after a negative integer. No missing-field warnings here.
    def record(field, value, index_marker=None):
        entries = [{"key": field, "value": value}]
        if index_marker is not None:
            entries.append({"key": string(b"index"), "value": index_marker})
        return {"type": "array", "entries": entries}

    def pluck_input(rows):
        return {"type": "array", "entries": [{"key": generator.choice(key_pool), "value": row} for row in rows]}

    selectors = [None, integer(0), string(b"0"), string(b""), string(b"index")]
    for value in scalars + [string(b"not an array"), array([])]:
        for index in selectors:
            add("wp_list_pluck", value, field=string(b"value"), index=index)
    indices = [None, {"type": "null"}, {"type": "bool", "value": False}, {"type": "bool", "value": True}]
    indices += [integer(value) for value in [-(2**63), -5, -1, 0, 1, 9]]
    indices += [string(value) for value in [b"", b"-5", b"0", b"01", b"+1", b"x", b"\0\xff", b"9223372036854775808"]]
    for iteration in range(400):
        field = generator.choice([integer(0), string(b"0"), string(b"value"), string(b"\0\xff")])
        rows = [record(field, generator.choice(pool), generator.choice(indices)) for _ in range(generator.randrange(18))]
        add("wp_list_pluck", pluck_input(rows), field=field, index=selectors[iteration % len(selectors)])
    # Explicit append boundaries, with distinct outer keys so no rows disappear.
    for marker in [integer(-(2**63)), integer(-5), string(b"-5"), string(b"x"), integer(9)]:
        rows = [record(string(b"value"), integer(1), marker), record(string(b"value"), integer(2)), record(string(b"value"), integer(3), {"type": "null"}), record(string(b"value"), integer(4), marker)]
        add("wp_list_pluck", values_array(rows), field=string(b"value"), index=string(b"index"))
    add("wp_list_pluck", values_array([record(string(b"value"), {"type": "null"}, integer(2**63-1))]), field=string(b"value"), index=string(b"index"))
    paths = strings + [b"/", b"//", b"///", b"c:\\a\\\\b//c", b"\\\\server\\share", b"a/./b/../c", b"http:\\\\host", b"a\n///b\r///c"]
    paths += [bytes([byte]) + b"////a\\\\b" for byte in range(256)]
    paths += [generator.choice([b"", b"/", b"//", b"a", b"c:", b"\n"]) + generator.choice([b"/", b"\\"]) * generator.randrange(20) + b"x\0\xff" for _ in range(200)]
    for path in paths:
        add("trailingslashit", string(path))
        add("untrailingslashit", string(path))
        if b"://" not in path:
            add("wp_normalize_path", string(path))
    date_parts = [(month, day, year)
                  for year in [-(2**63),-1,0,1,4,100,400,1600,1700,1900,2000,2024,2100,9999,32767,32768,2**63-1]
                  for month in [-1,0,*range(1,14),2**63-1]
                  for day in [-1,0,1,28,29,30,31,32,2**63-1]]
    date_parts += [(generator.randrange(-1,15),generator.randrange(-1,34),generator.randrange(-10,40000)) for _ in range(400)]
    for month,day,year in date_parts:
        for mode in [None,"append_raw"]:
            add("wp_checkdate",string(b"source\0\xff"),mode,month=str(month),day=str(day),year=str(year))
    serialized = [b"N;",b"s:3:\"a\0b\";",b"a:0:{}",b"b:222;",b"i:+-E..;",b"d:1e2;",b"d:1E2;",b"s:not-a-length\";",b"a:999:garbage}"]
    serialized += [token+b":"+length+tail
                   for token in [b"N",b"s",b"a",b"O",b"E",b"b",b"i",b"d",b"C",b"R",b"r",b"x"]
                   for length in [b"",b"0",b"00",b"1",b"-1",b"+1",b"1.0",b"01",b"999999999999999999999",b"x",b"1e2",b"1E2"]
                   for tail in [b"",b":",b':";',b':"a";',b":{}",b":garbage}",b";",b";tail",b"}",b"\n;",b";}"]]
    serialized += [bytes([byte])+valid+bytes([byte]) for byte in range(256) for valid in [b"N;",b"b:1;",b's:1:"a";',b"a:0:{}"]]
    for value in scalars + [array([])] + [string(value) for value in strings + serialized]:
        add("is_serialized",value,strict=True)
        add("is_serialized",value,strict=False)
        add("is_serialized_string",value)
    integers = [-(2**63),-(2**63)+1,-2,-1,0,1,2,2**63-1]
    integers += [generator.randrange(-(2**63),2**63) for _ in range(1000)]
    # Root Null is intentionally excluded: preg_split emits E_DEPRECATED on
    # PHP 8.5. Bool/integer string coercions below emit no diagnostics.
    for value in [{"type":"bool","value":False},{"type":"bool","value":True}] + [integer(value) for value in integers]:
        add("wp_parse_list", value)
        add("wp_parse_id_list", value)
    for value in integers:
        add("absint",integer(value))
    id_pool = [integer(value) for value in integers[:8]] + [{"type":"null"},array([])]
    add("wp_parse_id_list", values_array([integer(value) for value in [0,-1,1,2,-2,-(2**63),2**63-1,-(2**63),-(2**63)+1]]))
    for keys in key_sets + [generator.choices(key_pool,k=generator.randrange(30)) for _ in range(500)]:
        data = array(keys)
        for entry in data["entries"]:
            entry["value"] = generator.choice(id_pool)
        add("wp_parse_id_list",data)
    integer_strings = [b"",b" \t\n\r\x0b\x0c",b"-0",b"+0001",b"0"*100+b"1",b"-"+b"0"*100+b"9223372036854775808"]
    integer_strings += [str(value).encode() for value in integers[:8]]
    for _ in range(300):
        text = str(generator.choice(integers)).encode()
        left = generator.choice([b"",b" ",b"\t",b"\n",b"\r",b"\x0b",b"\x0c"])
        right = generator.choice([b"",b" ",b"\t",b"\n",b"\r",b"\x0b",b"\x0c"])
        integer_strings.append(left+text+right)
    supported_casts = [{"type":"null"},{"type":"bool","value":False},{"type":"bool","value":True},array([]),values_array([{"type":"float","value":"nan"}])]
    supported_casts += [string(value) for value in integer_strings]
    for value in supported_casts:
        add("absint",value)
    float_casts = [0.0, -0.0, 1.9, -1.9, 0.9999999999999999, -0.9999999999999999,
                   float.fromhex('0x0.0000000000001p-1022'), -float.fromhex('0x0.0000000000001p-1022'),
                   -(2.0**63), math.nextafter(-(2.0**63), 0), math.nextafter(2.0**63, 0)]
    for _ in range(3000):
        candidate = struct.unpack('>d', generator.randbytes(8))[0]
        if math.isfinite(candidate) and -(2.0**63) <= candidate < 2.0**63:
            float_casts.append(candidate)
    float_values = [{"type":"float", "value":repr(value)} for value in float_casts]
    for value in float_values:
        add("absint",value)
    for _ in range(200):
        data=values_array(generator.choices(float_values + [integer(-1), integer(0), integer(1), integer(-(2**63)), integer(2**63-1)],k=generator.randrange(20)))
        add("wp_parse_id_list",data)
    id_cast_pool = [integer(value) for value in integers[:8]] + [string(value) for value in integer_strings[:14]]
    id_cast_pool += [{"type":"null"},{"type":"bool","value":False},{"type":"bool","value":True},array([])]
    for _ in range(300):
        data=array(generator.choices(key_pool,k=generator.randrange(30)))
        for entry in data["entries"]:
            entry["value"]=generator.choice(id_cast_pool)
        add("wp_parse_id_list",data)
        values=[str(generator.choice(integers)).encode() for _ in range(generator.randrange(20))]
        add("wp_parse_id_list",string(generator.choice([b",",b" ",b"\t",b"\x0b",b"\x0c"]).join(values)))
    add("wp_parse_id_list",string(b"1,-1,+002 0,-0 -9223372036854775808 9223372036854775807"))
    for value in scalars + [string(value) for value in strings + serialized]:
        add("maybe_serialize",value)
    serialization_pool=[value for value in scalars if value["type"]!="float"]
    serialization_pool += [string(value) for value in [b"",b"a\0b",b"\0\xff\";",b"N;","🦀日本語".encode()]]
    serialization_pool += [array([]),values_array([string(b"nested"),integer(-(2**63))])]
    for keys in key_sets + [generator.choices(key_pool,k=generator.randrange(20)) for _ in range(300)]:
        data=array(keys)
        for entry in data["entries"]:
            entry["value"]=generator.choice(serialization_pool)
        add("maybe_serialize",data)
    def serialized_fixture(value):
        kind=value["type"]
        if kind=="null":
            return b"N;"
        if kind=="bool":
            return b"b:1;" if value["value"] else b"b:0;"
        if kind=="int":
            return b"i:"+value["value"].encode()+b";"
        if kind=="string":
            data=bytes.fromhex(value["hex"])
            return b"s:"+str(len(data)).encode()+b':"'+data+b'";'
        if kind=="array":
            entries=value["entries"]
            return b"a:"+str(len(entries)).encode()+b":{"+b"".join(serialized_fixture(entry["key"])+serialized_fixture(entry["value"]) for entry in entries)+b"}"
        raise ValueError("unsupported serialized fixture domain")

    def serializable_fixture(depth):
        if not depth or generator.random()<0.6:
            return generator.choice(serialization_pool)
        return {"type":"array","entries":[{"key":integer(index) if generator.randrange(2) else string(b"k"+str(index).encode()+b"\0\xff"),"value":serializable_fixture(depth-1)} for index in range(generator.randrange(8))]}

    for value in scalars + [array([]),values_array(scalars)] + [string(value) for value in [b"",b"plain",b"0",b"\0\xff",b"R:1;",b"d:NAN;"] + [bytes([byte]) for byte in range(256)]]:
        add("maybe_unserialize",value)
    fixtures=serialization_pool+[string(bytes([byte])) for byte in range(256)]
    fixtures += [serializable_fixture(3) for _ in range(400)]
    fixtures += [{"type":"array","entries":[{"key":key,"value":string(b'O:8:"stdClass":0:{}')} for key in [integer(-(2**63)),integer(2**63-1),string(b"01"),string(b"-0"),string(b"\0\xff"),string(b"9223372036854775808")]]}]
    for value in fixtures:
        encoded=serialized_fixture(value)
        add("maybe_unserialize",string(encoded))
        add("maybe_unserialize",string(b" \0\t"+encoded+b"\r\n\x0b"))
    add("maybe_unserialize",string(b"a:1:{i:0;"*64+b"N;"+b"}"*64))
    # Append-only API65 corpus keeps every preceding case ID and value stable.
    def rehash_case(raw):
        add('wp_password_needs_rehash', string(raw), rehash_profile='default_bcrypt_empty')
    nonprefixed = [b'', b'*', b'$WP', b' $wp', b'\0$wp', b'$w', b'$wP', b'$wp'.upper(),
        b'd41d8cd98f00b204e9800998ecf8427e', b'$P$512345678YsQn8dJMnXIqYTKyUqL7q1',
        wp_bcrypt.VECTORS['cost12'][3:].encode(), b'x'*4096, b'x'*4097, b'x'*8192, b'\xff'*4097]
    for raw in nonprefixed: rehash_case(raw)
    for byte in range(256):
        for raw in (bytes([byte]), bytes([byte])+b'$wp\0\xff', b'$w'+bytes([byte])+b'raw'):
            if not raw.startswith(b'$wp'): rehash_case(raw)
    generator = random.Random(20261029065)
    for _ in range(1024):
        raw = generator.randbytes(generator.randrange(129))
        if not raw.startswith(b'$wp'): rehash_case(raw)
    base = wp_bcrypt.VECTORS['cost12'].encode()
    for cost in range(4,32):
        rehash_case(base[:7]+f'{cost:02}'.encode()+base[9:])
    alphabet = b'./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
    for position in (10,31,32,62):
        for index, byte in enumerate(alphabet):
            if position==31 and index%16 or position==62 and index%4: continue
            rehash_case(base[:position]+bytes([byte])+base[position+1:])
    for _ in range(200):
        body = bytearray(generator.choice(alphabet) for _ in range(53))
        body[21] = alphabet[generator.randrange(4)*16]
        body[52] = alphabet[generator.randrange(16)*4]
        rehash_case(b'$wp$2y$'+f'{generator.randrange(4,32):02}'.encode()+b'$'+body)
    # API66 remains append-only after the complete API65 corpus.
    def accent_case(raw, locale):
        add('remove_accents',string(raw),locale=string(locale),accent_profile=accent_profile.NAME)
    tables=accent_data.read_tables()
    main_locales=[b'en_US',b'de',b'da_DK',b'ca',b'sr_RS',b'bs_BA']
    locales=main_locales+[b'de_DE',b'devil',b'de\0\xff',b'DE_DE',b'da_DK\0',b'ca_ES',b'sr_RS\0',
        b'\xff',b'\0',b'00',b'0\0',b'0 ',b'd',b'da_DKx',b'bs_BAx']
    base_keys=[bytes.fromhex(key)for key,_ in tables['BASE']]
    override_keys=[bytes.fromhex(key)for name,pairs in tables.items()if name!='BASE'for key,_ in pairs]
    for locale in main_locales:
        for key in base_keys: accent_case(key,locale)
        accent_case(b''.join(base_keys),locale)
        for byte in range(128): accent_case(bytes([byte]),locale)
    for locale in locales:
        for key in override_keys: accent_case(key,locale)
        for raw in [b'',bytes(range(128)),'Caf\u00e9 Cafe\u0301 A\u0308'.encode(),
            '\u00a3'.encode()*4096,'\u00c6'.encode()*2048,'l\u00b7l\u00b7l'.encode(),
            'll\u00b7l l\u00b7ll\u00b7l l\u00b7l\u00b7l\u00b7l'.encode(),
            '\u0391\u0410\u65e5\u672c\U0001f980\ufeff\ufffe\U0010ffff'.encode(),
            b'&eacute;%C3%A9\0\t\n\r\x7f']:
            accent_case(raw,locale)
    generator=random.Random(20261029066)
    palette=[key.decode('utf-8')for key in base_keys]+['l\u00b7l','\u0301','\u0308','\u1e9e','\0','A','z','\u65e5','\U0001f980','\uffff']
    for _ in range(512):
        raw=''.join(generator.choice(palette)for _ in range(generator.randrange(17))).encode()
        accent_case(raw,generator.choice(locales))
    # Preserve the preceding complete 66-API corpus and API66 explicit-locale subset.
    old_accent=[case for case in result if case['api']=='remove_accents']
    if (len(result)!=user_profile.OLD_COUNT or canonical_list_hash(result)!=user_profile.OLD_INPUT_SHA256
        or len(old_accent)!=user_profile.ACCENT_COUNT or canonical_list_hash(old_accent)!=user_profile.ACCENT_INPUT_SHA256):
        raise ValueError('Frozen API66 corpus changed before appending strip/user subsets')
    def strip_case(raw,remove_breaks):
        add('wp_strip_all_tags',string(raw),remove_breaks=remove_breaks,strip_profile=user_profile.STRIP_NAME)
    raw_strip=[b'',b'\0'*8192,b'\xff'*4096,b' \t\n\r\v\f ',b'\f\t\f',b'\v \v',b'A\0B\t\r\nC',b'>"\'?>',b'\xc3\0\xa9']
    for byte in range(256):
        if byte!=60:raw_strip.extend([bytes([byte]),b' X'+bytes([byte])+b'Y ',b'\0'+bytes([byte])+b'\0'])
    generator=random.Random(20261029067)
    raw_strip.extend(generator.randbytes(generator.randrange(129)).replace(b'<',b'=')for _ in range(512))
    for raw in raw_strip:
        for flag in (False,True):strip_case(raw,flag)
    def user_case(raw,strict,locale):
        add('sanitize_user',string(raw),strict=strict,locale=string(locale),user_profile=user_profile.USER_NAME)
    for locale in main_locales:
        for key in base_keys:
            for strict in (False,True):user_case(key,strict,locale)
        for byte in range(128):
            if byte!=60:
                for strict in (False,True):user_case(bytes([byte]),strict,locale)
    user_examples=[b'',b' ADMIN.Name-@_ ',b'\xc3\0\xa9',b'\xc2\0\xa3',b'\xf0\0\x9f\0\xa6\0\x80',
        'Caf\u00e9 Cafe\u0301 A\u0308'.encode(),b'\fuser\f',b'\f',b'\v\f\v',b'%20\f',
        b'%4%30A',b'%4A',b'%2541',b'&;',b'&;;',b'&a\n;b',b'&a ;b',b'&a\r;b',b'&&;',b'&x%00;',
        b'&lt;TAG&gt;',b'%3CTAG%3E',b'a>b?>c','\u00a3'.encode()*4096,'l&x;\u00b7l'.encode(),
        'l\u00b7l'.encode(),'\u0391\u0410\u65e5\U0001f980\u212a\u00a0\u2028\ufffe'.encode()]
    for locale in locales:
        for raw in user_examples:
            for strict in (False,True):user_case(raw,strict,locale)
    generator=random.Random(20261029068)
    for _ in range(512):
        original=''.join(generator.choice(palette)for _ in range(generator.randrange(17))).encode()
        # Insert NUL even inside multibyte code units: post-strip validity, not raw validity, is the domain.
        raw=b''.join(bytes([byte])+ (b'\0'if generator.randrange(3)==0 else b'')for byte in original)
        for strict in (False,True):user_case(raw,strict,generator.choice(locales))
    if len(result)!=slug_profile.OLD_COUNT or canonical_list_hash(result)!=slug_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API68 corpus changed before appending URI/title subsets')
    def uri_case(raw,length,flag):
        add('utf8_uri_encode',string(raw),length=length,encode_ascii=flag,uri_profile=slug_profile.URI_NAME)
    uri_examples=[b'',b'\0',b' A/Z%41~._-+?#&="\'<>',b'\t\n\r\v\f\x7f',
        'é Ä İ 日本語 🦀 \u2028\u2029\ufffe'.encode(),b'%41%aB%%41%',
        'aé日🦀z'.encode(),b'A'*199+'é'.encode()+b'Z']
    for byte in range(128):uri_examples.append(b'X'+bytes([byte])+b'Y')
    for point in [0x80,0x7ff,0x800,0xd7ff,0xe000,0xffff,0x10000,0x10ffff]:uri_examples.append(chr(point).encode())
    for raw in uri_examples:
        for length in (0,1,2,3,4,5,6,7,8,9,11,12,13,199,200,201,2**63-1):
            for flag in (False,True):uri_case(raw,length,flag)
    generator=random.Random(20261029069)
    uri_palette=['A','z','0','%','/',' ','\0','\n','é','Ä','日','🦀','\u0301','\uffff']
    for _ in range(512):
        raw=''.join(generator.choice(uri_palette)for _ in range(generator.randrange(24))).encode()
        for flag in (False,True):uri_case(raw,generator.choice([0,1,2,3,5,6,9,12,15,199,200]),flag)
    def title_case(raw,context):
        add('sanitize_title_with_dashes',string(raw),context=context,raw_title=string(b''),title_profile=slug_profile.TITLE_NAME)
    title_examples=[b'',b'\0'*4096,b'\xc3\0\x84',b'\0A\0 B\0',b'A/B.C_D',b'---41---',b'---4A---',b'-%%--41---',
        b'%41%aB%4%31%%---ab---',b'---ab---cd---',b'---ab------cd---',b'&;',b'&;;',b'A&x\n;B',b'A&x\r;B',
        b'&a;'*67,b' '*200+b'Tail',b'a'*199+b'%41',b'A'*198+b'%ABZ',b'a'*199+'é'.encode(),
        'Ä é İ Ａ 日本語 🦀 e\u0301'.encode(),b'A&nbsp;B&#160;C&#0160;D&#xA0;E',b'&times;']
    for byte in range(128):
        if byte!=60:title_examples.extend([bytes([byte]),b'A'+bytes([byte])+b'B'])
    # Every source save replacement spelling and original scalar is exercised.
    save_dash=['%c2%a0','%e2%80%91','%e2%80%93','%e2%80%94']
    save_remove=['%c2%ad','%c2%a1','%c2%bf','%c2%ab','%c2%bb','%e2%80%b9','%e2%80%ba',
        *['%e2%80%'+tail for tail in ('98','99','9c','9d','9a','9b','9e','9f','a2')],
        '%c2%a9','%c2%ae','%c2%b0','%e2%80%a6','%e2%84%a2','%c2%b4','%cb%8a','%cc%81','%cd%81','%cc%80','%cc%84','%cc%8c',
        *['%e2%80%'+tail for tail in ('8b','8c','8d','8e','8f','aa','ab','ac','ad','ae')],'%ef%bb%bf','%ef%bf%bc']
    save_width=['%e2%80%'+tail for tail in ('80','81','82','83','84','85','86','87','88','89','8a','a8','a9','af')]
    for entity in (b'&nbsp;',b'&#8209;',b'&#160;',b'&ndash;',b'&#8211;',b'&mdash;',b'&#8212;'):
        title_examples.append(b'A'+entity+b'B')
    for spelling in save_dash+save_remove+save_width+['%c3%97']:
        raw=bytes.fromhex(spelling.replace('%',''))
        title_examples.extend([b'A'+raw+b'B',b'A'+spelling.encode()+b'B',b'A'+spelling.upper().encode()+b'B'])
    for index,removed in enumerate(save_remove):
        target=save_remove[(index+1)%len(save_remove)]
        title_examples.append((target[:3]+removed+target[3:]).encode())
    for raw in title_examples:
        for context in ('display','save'):title_case(raw,context)
    generator=random.Random(20261029070)
    title_palette=['A','z','0','%41','---41---','---ab---','%','/',' ','\0','\n','&a;','&a\n;','é','Ä','日','🦀','\u0301',*save_remove,*save_width]
    for _ in range(768):
        raw=''.join(generator.choice(title_palette)for _ in range(generator.randrange(40))).encode()
        # NUL can repair a valid code-unit sequence; raw validity is not required.
        raw=b''.join(bytes([byte])+(b'\0'if generator.randrange(7)==0 else b'')for byte in raw)
        for context in ('display','save'):title_case(raw,context)
    if len(result)!=default_title_profile.OLD_COUNT or canonical_list_hash(result)!=default_title_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API70 corpus changed before appending default sanitize_title')
    default_title_profile.check_callback()
    def default_title_case(raw,fallback,context,locale):
        add('sanitize_title',string(raw),fallback=string(fallback),context=context,locale=string(locale),default_title_profile=default_title_profile.NAME)
    for key in base_keys:
        for context in ('save','display','query'):
            for locale in main_locales if context=='save'else[b'en_US']:
                default_title_case(key,b'fallback',context,locale)
    for byte in range(128):
        if byte!=60:
            for context in ('save','display','query'):default_title_case(bytes([byte]),b'\xff\0<RAW>',context,b'en_US')
    default_examples=[b'',b'0',b'00',b'%',b'\0',b'?',b'---41---',b'a'*199+b'%41',b'&a;'*67,
        b' '*200+b'End',b'A/B.C',b'A\tB\nC',b'A&nbsp;B',b'&x;0',b'%30',b'0?',
        '£'.encode()*256+b'END','Ä l·l Æ Đ ß'.encode(),'e\u0301'.encode(),b'\0'+ 'é'.encode()+b'\0']
    for raw in default_examples:
        for fallback in (b'',b'0',b'\xff\0<RAW>',b' No Cleanup/INPUT '):
            for context in ('save','display','query'):
                for locale in (b'en_US',b'de\0\xff',b'ca',b'00'):default_title_case(raw,fallback,context,locale)
    generator=random.Random(20261029071)
    default_palette=['A','0',' ','\n','\0','é','Ä','£','l·l','\u0301','日','🦀','%41','---41---','&a;','/','_']
    for _ in range(512):
        raw=''.join(generator.choice(default_palette)for _ in range(generator.randrange(30))).encode()
        for context in ('save','display','query'):
            default_title_case(raw,generator.choice([b'',b'\xff\0<RAW>',b'0',b'fallback']),context,generator.choice(locales))
    if len(result)!=truncate_profile.OLD_COUNT or canonical_list_hash(result)!=truncate_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API71 corpus changed before appending conditional truncate slug')
    def truncate_case(raw,length):
        if truncate_profile.admitted(raw,length):
            add('_truncate_post_slug',string(raw),length=length,truncate_profile=truncate_profile.NAME)
    truncate_examples=[b'',b'-',b'---',b'0',b'A--B---',b'\0-\xff-',b'%FF',b'%ff---',b'%C3%A9',b'%c3%a9-',b'\xc3\xa9-',
        b'+',b'%2B',b'%252B',b'%25',b'%2520',b'%2d',b'%2D---',b'%',b'%4',b'%GG',b'%0g',b'%%41',b'%4%31',b'A+B',
        b' A/B?C#D ',b'%00',b'A%00B',b'A%0AB',b'%E6%97%A5',b'%f0%9f%a6%80',b'\xff+Z',
        b'A'*199+b'%41',b'A'*200+b'+Z',b'-'*4096,b'\xff'*64,b'\xc3\x00\xa9',b'%41%FF',b'A%41'+b'\xff']
    for byte in range(256):
        truncate_examples.extend([bytes([byte]),b'A'+bytes([byte])+b'Z',f'%{byte:02x}'.encode(),f'%{byte:02X}'.encode()])
    for raw in truncate_examples:
        for length in sorted({0,1,2,3,4,5,6,8,9,12,199,200,201,len(raw),max(0,len(raw)-1),len(raw)+1,2**63-1}):
            truncate_case(raw,length)
    generator=random.Random(20261029072)
    truncate_palette=[b'A',b'0',b'-',b'---',b' ',b'+',b'%2B',b'%25',b'%00',b'%',b'%4',b'%GG',b'%41',b'\0',b'\xff',b'\xc3\xa9',b'%c3%a9',b'%E6%97%A5',b'%f0%9f%a6%80']
    for _ in range(768):
        raw=b''.join(generator.choice(truncate_palette)for _ in range(generator.randrange(32)))
        for length in (0,generator.randrange(40),len(raw),max(0,len(raw)-1)):truncate_case(raw,length)
    if len(result) != post_meta_profile.OLD_COUNT or canonical_list_hash(result) != post_meta_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API72 corpus changed before appending cached post metadata')
    for case in post_meta_profile.cases():
        add(case['api'], case['input'], **{key: value for key, value in case.items() if key not in ('api', 'input')})
    if len(result) != texturize_profile.OLD_COUNT or canonical_list_hash(result) != texturize_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API73 corpus changed before appending English texturize')
    for case in texturize_profile.cases():
        add(case['api'], case['input'], **{key: value for key, value in case.items() if key not in ('api', 'input')})
    if len(result) != title_profile.OLD_COUNT or canonical_list_hash(result) != title_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API74 corpus changed before appending prepared title')
    for case in title_profile.cases():
        add(case['api'], case['input'], **{key: value for key, value in case.items() if key not in ('api', 'input')})
    if len(result) != auth_cookie_profile.OLD_COUNT or canonical_list_hash(result) != auth_cookie_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API75 corpus changed before appending explicit auth cookies')
    for case in auth_cookie_profile.cases():
        add(case['api'], case['input'], **{key: value for key, value in case.items() if key not in ('api', 'input')})
    if len(result) != auth_cookie_parse_profile.OLD_COUNT or canonical_list_hash(result) != auth_cookie_parse_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API76 corpus changed before appending explicit cookie parser')
    for case in auth_cookie_parse_profile.cases():
        add(case['api'], case['input'], **{key: value for key, value in case.items() if key not in ('api', 'input')})
    if len(result) != sanitize_text_profile.OLD_COUNT or canonical_list_hash(result) != sanitize_text_profile.OLD_INPUT_SHA256:
        raise ValueError('Frozen API77 corpus changed before appending text sanitizers')
    for case in sanitize_text_profile.cases():
        add(case['api'], case['input'], **{key: value for key, value in case.items() if key not in ('api', 'input')})
    return result


def compare(inputs, rust, php):
    if len(inputs) != len(rust) or len(inputs) != len(php):
        raise ValueError("comparison result count mismatch")
    failures = []
    seen = set()
    for case, actual, expected in zip(inputs, rust, php):
        identifier = case["id"]
        if identifier in seen or actual.get("id") != identifier or expected.get("id") != identifier:
            raise ValueError("comparison case IDs duplicated, reordered, or missing")
        seen.add(identifier)
        if canonical(actual) != canonical(expected):
            failures.append({"case": case, "rust": actual, "wordpress": expected})
    return failures


def main(php):
    entity_names.check()
    xml_entities.check()
    accent_data.check()
    default_title_profile.check_callback()
    title_profile.check_callbacks()
    registry = json.loads(registry_tool.REGISTRY.read_text(encoding="utf-8"))
    registry_tool.validate(registry, check_evidence=False)
    source_root = reference.REFERENCE / f"wordpress-{reference.VERSION}"
    manifest = json.loads((source_root / reference.MANIFEST_NAME).read_text(encoding="utf-8"))
    recorded = {entry["path"]: entry["sha256"] for entry in manifest["files"]}
    sources = {api["source_path"] for api in registry["apis"]}
    for api in registry["apis"]:
        sources.update(api.get("source_dependencies", {}))
    sources.update(["wp-includes/plugin.php", "wp-includes/class-wp-hook.php", "wp-includes/class-wp-filter-sentinel.php"])
    source_hashes = {}
    for path in sorted(sources):
        digest = reference.digest_file(source_root / path)
        if digest != recorded[path]:
            raise ValueError(f"oracle source changed: {path}")
        source_hashes[path] = digest
    subprocess.run(["cargo", "build", "-p", "recastpress-wp", "--example", "compat_probe", "--offline", "--locked"], cwd=reference.ROOT, check=True, timeout=120)
    probe = reference.ROOT / "target" / "debug" / "examples" / ("compat_probe.exe" if os.name == "nt" else "compat_probe")
    inputs = cases()
    if not inputs or len({case["id"] for case in inputs}) != len(inputs):
        raise ValueError("primitive inputs must have globally unique case IDs")
    failures, identity = [], None
    empty_filter_counts = Counter({"esc_html": 0, "attribute_escape": 0})
    wp_bcrypt_calls = 0
    wp_rehash_hooks = Counter(dict.fromkeys(wp_rehash.HOOKS, 0))
    accent_calls=0
    strip_calls=0;user_calls=0;user_filter_calls=0;user_locale_calls=Counter({'zero':0,'one':0})
    uri_calls=0;title_calls=0
    previous68_rust=hashlib.sha256(b'[');previous68_php=hashlib.sha256(b'[')
    previous70_rust=hashlib.sha256(b'[');previous70_php=hashlib.sha256(b'[')
    previous71_rust=hashlib.sha256(b'[');previous71_php=hashlib.sha256(b'[')
    previous72_rust=hashlib.sha256(b'[');previous72_php=hashlib.sha256(b'[')
    previous73_rust=hashlib.sha256(b'[');previous73_php=hashlib.sha256(b'[')
    previous74_rust=hashlib.sha256(b'[');previous74_php=hashlib.sha256(b'[')
    previous75_rust=hashlib.sha256(b'[');previous75_php=hashlib.sha256(b'[')
    previous76_rust=hashlib.sha256(b'[');previous76_php=hashlib.sha256(b'[')
    previous77_rust=hashlib.sha256(b'[');previous77_php=hashlib.sha256(b'[')
    post_meta_counts=Counter(dict.fromkeys(post_meta_profile.COUNT_KEYS,0))
    texturize_counts=Counter(dict.fromkeys(texturize_profile.COUNT_KEYS,0))
    texturize_expected=Counter(dict.fromkeys(texturize_profile.COUNT_KEYS,0))
    title_counts=Counter(dict.fromkeys(title_profile.COUNT_KEYS,0))
    title_expected=Counter(dict.fromkeys(title_profile.COUNT_KEYS,0))
    auth_cookie_counts=Counter(dict.fromkeys(auth_cookie_profile.COUNT_KEYS,0))
    auth_cookie_expected=Counter(dict.fromkeys(auth_cookie_profile.COUNT_KEYS,0))
    auth_cookie_parse_counts=Counter(dict.fromkeys(auth_cookie_parse_profile.COUNT_KEYS,0))
    auth_cookie_parse_expected=Counter(dict.fromkeys(auth_cookie_parse_profile.COUNT_KEYS,0))
    sanitize_text_counts=Counter(dict.fromkeys(sanitize_text_profile.COUNT_KEYS,0))
    sanitize_text_expected=Counter(dict.fromkeys(sanitize_text_profile.COUNT_KEYS,0))
    truncate_calls=0;truncate_branches=Counter(dict.fromkeys(truncate_profile.BRANCHES,0))
    default_title_calls=0;default_title_filters=0;default_title_restored=0;default_title_locale=Counter({'zero':0,'one':0})
    rust_hash, php_hash = hashlib.sha256(b"["), hashlib.sha256(b"[")
    processed, batch_count, largest_batch = 0, 0, 0
    for batch_count, batch in enumerate(batches(inputs), 1):
        wire = canonical(batch)
        largest_batch = max(largest_batch, len(wire))
        rust = subprocess.run([str(probe)], input=wire, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=30)
        oracle = subprocess.run([str(php), "-n", str(reference.ROOT / "scripts" / "wp_oracle.php"), str(source_root)], input=wire, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=30)
        if oracle.stderr or rust.stderr:
            raise ValueError(f"batch {batch_count} emitted unexpected diagnostics: PHP={oracle.stderr.decode(errors='replace')}, Rust={rust.stderr.decode(errors='replace')}")
        php_output = json.loads(oracle.stdout)
        if php_output["parse_str_settings"] != {"arg_separator.input": "&", "max_input_vars": "1000", "max_input_nesting_level": "64"}:
            raise ValueError("query fixture requires the pinned default parse_str settings")
        if php_output["ctype_locale"] != "C":
            raise ValueError("JSONP callback fixture requires the C LC_CTYPE locale")
        if php_output["mbstring_loaded"] is not False:
            raise ValueError("this UTF-8 fixture requires the no-mbstring WordPress fallback")
        registry_tool.validate_json_encode_flags(php_output.get("json_encode_flags"))
        registry_tool.validate_json_utf8_fallback_profile(php_output.get("json_utf8_fallback_profile"))
        registry_tool.validate_hr_byte_units(php_output.get("hr_byte_units"))
        registry_tool.validate_hr_byte_domain(php_output.get("hr_byte_max_significant_digits"))
        registry_tool.validate_default_html_entity_profile(php_output.get("default_html_entity_profile"))
        registry_tool.validate_xml_entity_profile(php_output.get("xml_utf8_entity_profile"))
        registry_tool.validate_specialchars_profile(php_output.get("specialchars_utf8_profile"))
        registry_tool.validate_empty_filter_escaping_profile(php_output.get("empty_filter_escaping_profile"))
        registry_tool.validate_empty_filter_hook_counts(php_output.get("empty_filter_escaping_hook_counts"), Counter(case["api"] for case in batch))
        empty_filter_counts.update(php_output["empty_filter_escaping_hook_counts"])
        registry_tool.validate_wp_bcrypt_profile(php_output.get("wp_bcrypt_profile"))
        batch_password_calls=php_output.get("wp_bcrypt_check_password_calls")
        if type(batch_password_calls) is not int or batch_password_calls != sum(case["api"] == "wp_check_password" for case in batch):
            raise ValueError("WP bcrypt actual hook count differs from batch")
        wp_bcrypt_calls += batch_password_calls
        registry_tool.validate_wp_rehash_profile(php_output.get('wp_rehash_profile'))
        batch_rehash_total = sum(case['api']=='wp_password_needs_rehash' for case in batch)
        registry_tool.validate_wp_rehash_hooks(php_output.get('wp_rehash_hook_counts'), batch_rehash_total)
        wp_rehash_hooks.update(php_output['wp_rehash_hook_counts'])
        registry_tool.validate_remove_accents_profile(php_output.get('remove_accents_profile'))
        registry_tool.validate_remove_accents_calls(php_output.get('remove_accents_calls'),php_output.get('remove_accents_get_locale_calls'),
            sum(case['api']=='remove_accents'for case in batch))
        accent_calls+=php_output['remove_accents_calls']
        registry_tool.validate_strip_all_tags_profile(php_output.get('strip_all_tags_profile'))
        registry_tool.validate_strip_calls(php_output.get('strip_all_tags_calls'),sum(case['api']=='wp_strip_all_tags'for case in batch))
        strip_calls+=php_output['strip_all_tags_calls']
        registry_tool.validate_sanitize_user_profile(php_output.get('sanitize_user_profile'))
        registry_tool.validate_sanitize_user_calls(php_output.get('sanitize_user_calls'),php_output.get('sanitize_user_filter_calls'),
            php_output.get('sanitize_user_locale_hook_counts'),php_output.get('locale_db_branch_calls'),sum(case['api']=='sanitize_user'for case in batch))
        user_calls+=php_output['sanitize_user_calls'];user_filter_calls+=php_output['sanitize_user_filter_calls'];user_locale_calls.update(php_output['sanitize_user_locale_hook_counts'])
        registry_tool.validate_slug_profile(php_output.get('utf8_uri_profile'))
        registry_tool.validate_slug_profile(php_output.get('title_dash_profile'),True)
        registry_tool.validate_strip_calls(php_output.get('utf8_uri_calls'),sum(case['api']=='utf8_uri_encode'for case in batch))
        registry_tool.validate_strip_calls(php_output.get('title_dash_calls'),sum(case['api']=='sanitize_title_with_dashes'for case in batch))
        uri_calls+=php_output['utf8_uri_calls'];title_calls+=php_output['title_dash_calls']
        registry_tool.validate_default_title_profile(php_output.get('default_sanitize_title_profile'))
        registry_tool.validate_default_title_calls(php_output.get('default_title_calls'),php_output.get('default_title_filter_calls'),
            php_output.get('default_title_locale_calls'),php_output.get('default_title_restored'),sum(case['api']=='sanitize_title'for case in batch))
        default_title_calls+=php_output['default_title_calls'];default_title_filters+=php_output['default_title_filter_calls']
        default_title_restored+=php_output['default_title_restored'];default_title_locale.update(php_output['default_title_locale_calls'])
        registry_tool.validate_truncate_slug_profile(php_output.get('truncate_slug_profile'))
        registry_tool.validate_truncate_slug_calls(php_output.get('truncate_slug_calls'),php_output.get('truncate_slug_branches'),sum(case['api']=='_truncate_post_slug'for case in batch))
        truncate_calls+=php_output['truncate_slug_calls'];truncate_branches.update(php_output['truncate_slug_branches'])
        post_meta_profile.validate_profile(php_output.get('post_meta_cached_profile'))
        post_meta_profile.validate_counts(php_output.get('post_meta_cached_counts'), post_meta_profile.expected_counts(batch))
        post_meta_counts.update(php_output['post_meta_cached_counts'])
        texturize_profile.validate_profile(php_output.get('texturize_profile'))
        expected_texturize_batch = texturize_profile.expected_counts(batch)
        texturize_profile.validate_counts(php_output.get('texturize_counts'), expected_texturize_batch)
        texturize_counts.update(php_output['texturize_counts'])
        texturize_expected.update(expected_texturize_batch)
        title_profile.validate_profile(php_output.get('title_profile'))
        expected_title_batch = title_profile.expected_counts(batch)
        title_profile.validate_counts(php_output.get('title_counts'), expected_title_batch)
        title_counts.update(php_output['title_counts'])
        title_expected.update(expected_title_batch)
        auth_cookie_profile.validate_profile(php_output.get('auth_cookie_profile'))
        expected_auth_cookie_batch = auth_cookie_profile.expected_counts(batch)
        auth_cookie_profile.validate_counts(php_output.get('auth_cookie_counts'), expected_auth_cookie_batch)
        auth_cookie_counts.update(php_output['auth_cookie_counts'])
        auth_cookie_expected.update(expected_auth_cookie_batch)
        auth_cookie_parse_profile.validate_profile(php_output.get('auth_cookie_parse_profile'))
        expected_auth_cookie_parse_batch = auth_cookie_parse_profile.expected_counts(batch)
        auth_cookie_parse_profile.validate_counts(php_output.get('auth_cookie_parse_counts'), expected_auth_cookie_parse_batch)
        auth_cookie_parse_counts.update(php_output['auth_cookie_parse_counts'])
        auth_cookie_parse_expected.update(expected_auth_cookie_parse_batch)
        sanitize_text_profile.validate_profile(php_output.get('sanitize_text_profile'))
        expected_sanitize_text_batch = sanitize_text_profile.expected_counts(batch)
        sanitize_text_profile.validate_counts(php_output.get('sanitize_text_counts'), expected_sanitize_text_batch)
        sanitize_text_counts.update(php_output['sanitize_text_counts'])
        sanitize_text_expected.update(expected_sanitize_text_batch)
        registry_tool.validate_entity_decode_profile(php_output.get("entity_decode_profile"))
        registry_tool.validate_protocol_once_profile(php_output.get("protocol_once_profile"))
        registry_tool.validate_protocol_outer_profile(php_output.get("protocol_outer_profile"))
        current_identity = (php_output["php_version"], php_output["php_int_size"], php_output["mbstring_loaded"], php_output["ctype_locale"])
        if current_identity[1] != 8 or (identity is not None and current_identity != identity):
            raise ValueError("comparison PHP identity changed or is not 64-bit")
        identity = current_identity
        rust_results, php_results = json.loads(rust.stdout), php_output["results"]
        failures.extend(compare(batch, rust_results, php_results))
        for actual, expected in zip(rust_results, php_results):
            if processed<sanitize_text_profile.OLD_COUNT:
                if processed:previous77_rust.update(b',');previous77_php.update(b',')
                previous77_rust.update(canonical(actual));previous77_php.update(canonical(expected))
            if processed<auth_cookie_parse_profile.OLD_COUNT:
                if processed:previous76_rust.update(b',');previous76_php.update(b',')
                previous76_rust.update(canonical(actual));previous76_php.update(canonical(expected))
            if processed<auth_cookie_profile.OLD_COUNT:
                if processed:previous75_rust.update(b',');previous75_php.update(b',')
                previous75_rust.update(canonical(actual));previous75_php.update(canonical(expected))
            if processed<title_profile.OLD_COUNT:
                if processed:previous74_rust.update(b',');previous74_php.update(b',')
                previous74_rust.update(canonical(actual));previous74_php.update(canonical(expected))
            if processed<texturize_profile.OLD_COUNT:
                if processed:previous73_rust.update(b',');previous73_php.update(b',')
                previous73_rust.update(canonical(actual));previous73_php.update(canonical(expected))
            if processed<post_meta_profile.OLD_COUNT:
                if processed:previous72_rust.update(b',');previous72_php.update(b',')
                previous72_rust.update(canonical(actual));previous72_php.update(canonical(expected))
            if processed<truncate_profile.OLD_COUNT:
                if processed:previous71_rust.update(b',');previous71_php.update(b',')
                previous71_rust.update(canonical(actual));previous71_php.update(canonical(expected))
            if processed<default_title_profile.OLD_COUNT:
                if processed:previous70_rust.update(b',');previous70_php.update(b',')
                previous70_rust.update(canonical(actual));previous70_php.update(canonical(expected))
            if processed<slug_profile.OLD_COUNT:
                if processed:previous68_rust.update(b',');previous68_php.update(b',')
                previous68_rust.update(canonical(actual));previous68_php.update(canonical(expected))
            if processed:
                rust_hash.update(b",")
                php_hash.update(b",")
            rust_hash.update(canonical(actual))
            php_hash.update(canonical(expected))
            processed += 1
        print(f"Compared primitive batch {batch_count}: {len(batch)} cases", flush=True)
    if processed != len(inputs):
        raise ValueError("not all primitive cases were compared")
    rust_hash.update(b"]")
    php_hash.update(b"]")
    previous68_rust.update(b']');previous68_php.update(b']')
    if previous68_rust.hexdigest()!=slug_profile.OLD_OUTPUT_SHA256 or previous68_php.hexdigest()!=slug_profile.OLD_OUTPUT_SHA256:
        raise ValueError('Previous API68 exact outputs changed')
    previous70_rust.update(b']');previous70_php.update(b']')
    if previous70_rust.hexdigest()!=default_title_profile.OLD_OUTPUT_SHA256 or previous70_php.hexdigest()!=default_title_profile.OLD_OUTPUT_SHA256:
        raise ValueError('Previous API70 exact outputs changed')
    previous71_rust.update(b']');previous71_php.update(b']')
    if previous71_rust.hexdigest()!=truncate_profile.OLD_OUTPUT_SHA256 or previous71_php.hexdigest()!=truncate_profile.OLD_OUTPUT_SHA256:
        raise ValueError('Previous API71 exact outputs changed')
    previous72_rust.update(b']');previous72_php.update(b']')
    if previous72_rust.hexdigest()!=post_meta_profile.OLD_OUTPUT_SHA256 or previous72_php.hexdigest()!=post_meta_profile.OLD_OUTPUT_SHA256:
        raise ValueError('Previous API72 exact outputs changed')
    previous73_rust.update(b']');previous73_php.update(b']')
    texturize_profile.validate_previous(texturize_profile.OLD_COUNT, texturize_profile.OLD_INPUT_SHA256, previous73_rust.hexdigest(), previous73_php.hexdigest())
    previous74_rust.update(b']');previous74_php.update(b']')
    title_profile.validate_previous(title_profile.OLD_COUNT, title_profile.OLD_INPUT_SHA256, previous74_rust.hexdigest(), previous74_php.hexdigest())
    previous75_rust.update(b']');previous75_php.update(b']')
    auth_cookie_profile.validate_previous(auth_cookie_profile.OLD_COUNT, auth_cookie_profile.OLD_INPUT_SHA256, previous75_rust.hexdigest(), previous75_php.hexdigest())
    previous76_rust.update(b']');previous76_php.update(b']')
    auth_cookie_parse_profile.validate_previous(auth_cookie_parse_profile.OLD_COUNT, auth_cookie_parse_profile.OLD_INPUT_SHA256, previous76_rust.hexdigest(), previous76_php.hexdigest())
    previous77_rust.update(b']');previous77_php.update(b']')
    sanitize_text_profile.validate_previous(sanitize_text_profile.OLD_COUNT, sanitize_text_profile.OLD_INPUT_SHA256, previous77_rust.hexdigest(), previous77_php.hexdigest())
    evidence_root = reference.ROOT / "compatibility" / "evidence"
    evidence_root.mkdir(exist_ok=True)
    if failures:
        (evidence_root / "primitives-failures.json").write_text(json.dumps(failures, indent=2) + "\n", encoding="utf-8")
        raise ValueError(f"{len(failures)} mismatches; see compatibility/evidence/primitives-failures.json")
    (evidence_root / "primitives-failures.json").unlink(missing_ok=True)
    required = registry_tool.evidence_files(registry_tool.PRIMITIVE_SCOPE)
    implementation = sorted(required["implementation_files"])
    harness = sorted(required["harness_files"])
    evidence = {
        "format_version": 1,
        "scope": "isolated_function_subsets_with_real_wp_filter_dispatch",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "wordpress_version": reference.VERSION,
        "archive_sha256": registry["archive_sha256"],
        "php_version": php_output["php_version"],
        "php_int_size": php_output["php_int_size"],
        "php_mbstring_loaded": php_output["mbstring_loaded"],
        "jsonp_ctype_locale": php_output["ctype_locale"],
        "utf8_oracle_path": "no_mbstring_fallback",
        "php_parse_str_settings": php_output["parse_str_settings"],
        "php_json_encode_flags": php_output["json_encode_flags"],
        "php_json_utf8_fallback_profile": php_output["json_utf8_fallback_profile"],
        "php_hr_byte_units": php_output["hr_byte_units"],
        "php_hr_byte_max_significant_digits": php_output["hr_byte_max_significant_digits"],
        "php_default_html_entity_profile": php_output["default_html_entity_profile"],
        "php_xml_utf8_entity_profile": php_output["xml_utf8_entity_profile"],
        "entity_context_case_counts": dict(Counter(case["context"] for case in inputs if case["api"] == "wp_kses_normalize_entities")),
        "php_specialchars_utf8_profile": php_output["specialchars_utf8_profile"],
        "php_empty_filter_escaping_profile": php_output["empty_filter_escaping_profile"],
        "empty_filter_escaping_hook_counts": dict(empty_filter_counts),
        "php_wp_bcrypt_profile": php_output["wp_bcrypt_profile"],
        "php_wp_rehash_profile": php_output['wp_rehash_profile'],
        "php_remove_accents_profile": php_output['remove_accents_profile'],
        "php_strip_all_tags_profile": php_output['strip_all_tags_profile'],
        "strip_all_tags_reference_calls": strip_calls,
        "strip_all_tags_flag_case_counts": dict(Counter(str(case['remove_breaks']).lower()for case in inputs if case['api']=='wp_strip_all_tags')),
        "php_sanitize_user_profile": php_output['sanitize_user_profile'],
        "php_utf8_uri_profile":php_output['utf8_uri_profile'],"php_title_dash_profile":php_output['title_dash_profile'],
        "utf8_uri_reference_calls":uri_calls,"title_dash_reference_calls":title_calls,
        "utf8_uri_option_case_counts":dict(Counter(('zero'if case['length']==0 else'positive')+':'+str(case['encode_ascii']).lower()for case in inputs if case['api']=='utf8_uri_encode')),
        "title_dash_context_case_counts":dict(Counter(case['context']for case in inputs if case['api']=='sanitize_title_with_dashes')),
        "previous_api68_input_sha256":slug_profile.OLD_INPUT_SHA256,"previous_api68_results_sha256":previous68_rust.hexdigest(),
        "php_default_sanitize_title_profile":php_output['default_sanitize_title_profile'],
        "default_title_reference_calls":default_title_calls,"default_title_filter_calls":default_title_filters,
        "default_title_locale_hook_counts":dict(default_title_locale),"default_title_global_restorations":default_title_restored,
        "default_title_expected_locale_hook_counts":dict(Counter('one'if default_title_profile.locale_calls(bytes.fromhex(case['input']['hex']),case['context'])else'zero'for case in inputs if case['api']=='sanitize_title')),
        "default_title_context_case_counts":dict(Counter(case['context']for case in inputs if case['api']=='sanitize_title')),
        "previous_api70_input_sha256":default_title_profile.OLD_INPUT_SHA256,"previous_api70_results_sha256":previous70_rust.hexdigest(),
        "php_truncate_slug_profile":php_output['truncate_slug_profile'],"truncate_slug_reference_calls":truncate_calls,
        "truncate_slug_branch_counts":dict(truncate_branches),
        "truncate_slug_expected_branch_counts":dict(Counter(truncate_profile.branch(bytes.fromhex(case['input']['hex']),case['length'])for case in inputs if case['api']=='_truncate_post_slug')),
        "previous_api71_input_sha256":truncate_profile.OLD_INPUT_SHA256,"previous_api71_results_sha256":previous71_rust.hexdigest(),
        "php_post_meta_cached_profile":php_output['post_meta_cached_profile'],
        "post_meta_cached_counts":dict(post_meta_counts),
        "post_meta_cached_expected_counts":post_meta_profile.expected_counts(inputs),
        "previous_api72_case_count":post_meta_profile.OLD_COUNT,
        "previous_api72_input_sha256":post_meta_profile.OLD_INPUT_SHA256,
        "previous_api72_rust_results_sha256":previous72_rust.hexdigest(),
        "previous_api72_wordpress_results_sha256":previous72_php.hexdigest(),
        "php_texturize_profile":php_output['texturize_profile'],
        "texturize_counts":dict(texturize_counts),
        "texturize_expected_counts":dict(texturize_expected),
        "previous_api73_case_count":texturize_profile.OLD_COUNT,
        "previous_api73_input_sha256":texturize_profile.OLD_INPUT_SHA256,
        "previous_api73_rust_results_sha256":previous73_rust.hexdigest(),
        "previous_api73_wordpress_results_sha256":previous73_php.hexdigest(),
        "php_title_profile":php_output['title_profile'],
        "title_counts":dict(title_counts),
        "title_expected_counts":dict(title_expected),
        "previous_api74_case_count":title_profile.OLD_COUNT,
        "previous_api74_input_sha256":title_profile.OLD_INPUT_SHA256,
        "previous_api74_rust_results_sha256":previous74_rust.hexdigest(),
        "previous_api74_wordpress_results_sha256":previous74_php.hexdigest(),
        "php_auth_cookie_profile":php_output['auth_cookie_profile'],
        "auth_cookie_counts":dict(auth_cookie_counts),
        "auth_cookie_expected_counts":dict(auth_cookie_expected),
        "previous_api75_case_count":auth_cookie_profile.OLD_COUNT,
        "previous_api75_input_sha256":auth_cookie_profile.OLD_INPUT_SHA256,
        "previous_api75_rust_results_sha256":previous75_rust.hexdigest(),
        "previous_api75_wordpress_results_sha256":previous75_php.hexdigest(),
        "php_auth_cookie_parse_profile":php_output['auth_cookie_parse_profile'],
        "auth_cookie_parse_counts":dict(auth_cookie_parse_counts),
        "auth_cookie_parse_expected_counts":dict(auth_cookie_parse_expected),
        "previous_api76_case_count":auth_cookie_parse_profile.OLD_COUNT,
        "previous_api76_input_sha256":auth_cookie_parse_profile.OLD_INPUT_SHA256,
        "previous_api76_rust_results_sha256":previous76_rust.hexdigest(),
        "previous_api76_wordpress_results_sha256":previous76_php.hexdigest(),
        "php_sanitize_text_profile":php_output['sanitize_text_profile'],
        "sanitize_text_counts":dict(sanitize_text_counts),
        "sanitize_text_expected_counts":dict(sanitize_text_expected),
        "previous_api77_case_count":sanitize_text_profile.OLD_COUNT,
        "previous_api77_input_sha256":sanitize_text_profile.OLD_INPUT_SHA256,
        "previous_api77_rust_results_sha256":previous77_rust.hexdigest(),
        "previous_api77_wordpress_results_sha256":previous77_php.hexdigest(),
        "sanitize_user_reference_calls": user_calls,"sanitize_user_filter_calls":user_filter_calls,
        "sanitize_user_locale_hook_counts": dict(user_locale_calls),"locale_db_branch_calls":php_output['locale_db_branch_calls'],
        "sanitize_user_strict_case_counts": dict(Counter(str(case['strict']).lower()for case in inputs if case['api']=='sanitize_user')),
        "sanitize_user_expected_locale_hook_counts": dict(Counter('one'if user_profile.locale_calls(bytes.fromhex(case['input']['hex']))else'zero'
            for case in inputs if case['api']=='sanitize_user')),
        "previous_api66_input_sha256": user_profile.ACCENT_INPUT_SHA256,
        "remove_accents_reference_calls": accent_calls,
        "remove_accents_get_locale_calls": php_output['remove_accents_get_locale_calls'],
        "remove_accents_locale_branch_case_counts": dict(Counter(accent_profile.branch(bytes.fromhex(case['locale']['hex']))
            for case in inputs if case['api']=='remove_accents')),
        "wp_rehash_hook_counts": dict(wp_rehash_hooks),
        "wp_rehash_case_kind_counts": dict(Counter('canonical_prefixed' if bytes.fromhex(case['input']['hex']).startswith(b'$wp') else 'nonprefixed'
            for case in inputs if case['api']=='wp_password_needs_rehash')),
        "wp_rehash_cost_case_counts": dict(Counter(str(int(bytes.fromhex(case['input']['hex'])[7:9]))
            for case in inputs if case['api']=='wp_password_needs_rehash' and bytes.fromhex(case['input']['hex']).startswith(b'$wp'))),
        "wp_bcrypt_check_password_calls": wp_bcrypt_calls,
        "wp_bcrypt_hash_cost_case_counts": dict(Counter(str(int(bytes.fromhex(case["password_hash"]["hex"])[7:9])) for case in inputs if case["api"] == "wp_check_password")),
        "wp_bcrypt_host_max_cost_case_counts": dict(Counter(str(case["max_cost"]) for case in inputs if case["api"] == "wp_check_password")),
        "wp_bcrypt_length_case_counts": dict(Counter("over_4096" if len(bytes.fromhex(case["input"]["hex"])) > 4096 else "within_4096" for case in inputs if case["api"] == "wp_check_password")),
        "specialchars_style_double_case_counts": dict(Counter(case["quote_style"] + ":" + str(case["double_encode"]).lower()
            for case in inputs if case["api"] == "_wp_specialchars")),
        "php_entity_decode_profile": php_output["entity_decode_profile"],
        "php_protocol_once_profile": php_output["protocol_once_profile"],
        "php_protocol_outer_profile": php_output["protocol_outer_profile"],
        "php_binary_sha256": reference.digest_file(php),
        "rust_probe_sha256": reference.digest_file(probe),
        "platform": platform.platform(),
        "case_count": len(inputs),
        "case_counts_by_api": dict(Counter(case["api"] for case in inputs)),
        "json_encode_flag_case_counts": dict(Counter(str(case.get("flags",0)) for case in inputs if case["api"]=="wp_json_encode")),
        "json_encode_profile_case_counts": dict(Counter(case.get("utf8_profile", "first_encode_only") for case in inputs if case["api"]=="wp_json_encode")),
        "json_encode_utf8_fallback_flag_case_counts": dict(Counter(str(case.get("flags", 0)) for case in inputs if case["api"] == "wp_json_encode" and case.get("utf8_profile") == "utf8_no_mbstring")),
        "comparison_batches": batch_count,
        "batch_case_limit": BATCH_CASE_LIMIT,
        "batch_input_limit_bytes": BATCH_INPUT_LIMIT,
        "max_observed_batch_bytes": largest_batch,
        "unique_id_state_scope": registry_tool.UNIQUE_ID_STATE_SCOPE,
        "mismatches": 0,
        "input_sha256": canonical_list_hash(inputs),
        "rust_results_sha256": rust_hash.hexdigest(),
        "wordpress_results_sha256": php_hash.hexdigest(),
        "source_files": source_hashes,
        "implementation_files": {path: reference.digest_file(reference.ROOT / path) for path in implementation},
        "harness_files": {path: reference.digest_file(reference.ROOT / path) for path in harness},
        "limitations": ["No full WordPress bootstrap, DB, themes, or plugins were executed.", "Only supported primitive value domains were compared.", "JSONP callback byte classification was compared only with the C LC_CTYPE locale, checked by the PHP oracle.", "The API universe is not enumerated; these results are not complete compatibility or a coverage percentage."]
    }
    output = evidence_root / "primitives.json"
    output.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(f"Matched {len(inputs)} cases against WordPress {reference.VERSION}, PHP {php_output['php_version']}; evidence: {output.relative_to(reference.ROOT)}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--php", type=Path, default=reference.REFERENCE / "php-8.5.12" / "php.exe")
    options = parser.parse_args()
    try:
        main(options.php.resolve())
    except subprocess.CalledProcessError as error:
        parser.exit(1, f"Comparison command failed: {error}\n{(error.stderr or b'').decode(errors='replace')}\n{(error.stdout or b'').decode(errors='replace')}\n")
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f"Primitive comparison failed: {error}\n")
