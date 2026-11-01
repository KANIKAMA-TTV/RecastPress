"""Fixed public-dummy, stable-margin relation cases; not a static WP API corpus.

Cookie MAC fixtures use Python's standard HMAC providers, not a replacement
implementation of the actual PHP validation being observed. Empty/zero tokens
are manually signed and are NOT passed through WP's generation API.
"""
from copy import deepcopy
import hashlib
import hmac
import time

LIMITS = {"max_input_bytes": 65536, "max_sessions": 64, "max_session_key_bytes": 4096}
ACTIONS = ("auth_cookie_malformed", "auth_cookie_expired", "auth_cookie_bad_username",
           "auth_cookie_bad_hash", "auth_cookie_bad_session_token", "auth_cookie_valid")
COUNT_KEYS = ("calls", "ajax_filter", "user_lookup", "hash_salt", "manager_filter", "user_meta",
              "writes", "db", "cache", "options", "rng", "ssl", "cookie_accesses", "restored")
HASH = b"$wp$2y$12$public-placeholder-endingABCD"
USERNAME = b"public-cookie-user"
TOKEN = b"public-fixed-token"
SALT = b"public-resolved-salt"


def signed(username, expiry, token, password_hash=HASH, salt=SALT):
    fragment = password_hash[8:12] if password_hash.startswith((b"$P$", b"$2y$")) else password_hash[-4:]
    expiration = str(expiry).encode("ascii")
    key = hmac.new(salt, b"|".join((username, fragment, expiration, token)), "md5").hexdigest().encode("ascii")
    tag = hmac.new(key, b"|".join((username, expiration, token)), "sha256").hexdigest().encode("ascii")
    return b"|".join((username, expiration, token, tag))


def session(token, expiry, legacy=False):
    return {"verifier": hashlib.sha256(token).hexdigest(), "expiration": str(expiry), "legacy": legacy}


def expected_counts(stage):
    counts = dict.fromkeys(COUNT_KEYS, 0)
    counts.update(calls=1, restored=1)
    counts["ajax_filter"] = int(stage != "malformed")
    counts["user_lookup"] = int(stage not in ("malformed", "expired"))
    counts["hash_salt"] = int(stage in ("bad_mac", "bad_session", "valid"))
    counts["manager_filter"] = counts["user_meta"] = int(stage in ("bad_session", "valid"))
    return counts


def expected_actions(stage):
    selected = {"malformed": ACTIONS[0], "expired": ACTIONS[1], "unknown_user": ACTIONS[2],
                "bad_mac": ACTIONS[3], "bad_session": ACTIONS[4], "valid": ACTIONS[5]}[stage]
    return {name: int(name == selected) for name in ACTIONS}


def cases(anchor=None):
    """Return [{name, expected_stage, request_without_now}], finite and ordered.

    Anchor is fixture construction only. PHP keeps builtin time(); the controller
    must independently check both real endpoints for EVERY supplied expiration.
    No exact equality/extrema actual-PHP clock parity is claimed here.
    """
    now = int(time.time()) if anchor is None else anchor
    if type(now) is not int or not 3600 < now < (1 << 63) - 7200:
        raise ValueError("public fixture anchor outside profile")
    future, past = now + 3600, now - 3600
    output = []

    def add(name, stage="valid", *, username=USERNAME, token=TOKEN, password_hash=HASH,
            salt=SALT, expiry=future, row_login=USERNAME, user=True, rows=None, cookie=None, scheme=b"auth"):
        if cookie is None:
            cookie = signed(username, expiry, token, password_hash, salt)
        request = {"id": len(output), "cookie_hex": cookie.hex(), "scheme_hex": scheme.hex(), "salt_hex": salt.hex(),
                   "user": {"id": "7", "login_hex": row_login.hex(), "hash_hex": password_hash.hex()} if user else None,
                   "sessions": [session(token, future)] if rows is None else deepcopy(rows), "limits": LIMITS.copy()}
        output.append({"name": name, "expected_stage": stage, "request_without_now": request})

    good = signed(USERNAME, future, TOKEN)
    lower_tag = good.rsplit(b"|", 1)[1]
    if lower_tag.upper() == lower_tag:
        raise ValueError("Public fixture requires a MAC with alphabetic hex digits")
    add("current_cookie_and_record_session")
    add("raw_username_differs_from_returned_canonical_row", row_login=b"Canonical Row")
    add("canonical_row_username_must_not_replace_raw_mac_username", "bad_mac", row_login=b"Canonical Row",
        cookie=b"|".join((USERNAME, str(future).encode(), TOKEN, signed(b"Canonical Row", future, TOKEN).rsplit(b"|", 1)[1])))
    add("binary_username_token_salt_and_unknown_binary_scheme", username=b"public\0\xffuser", token=b"token\0\xff",
        salt=b"salt\0\xff", scheme=b"custom\0\xffscheme", row_login=b"canonical")
    add("manual_empty_token_is_valid_not_generation_input", token=b"")
    add("manual_ascii_zero_token_is_valid_not_generation_input", token=b"0")
    add("expired_cookie_precedes_missing_user_and_bad_mac", "expired", expiry=past, user=False,
        cookie=signed(USERNAME, past, TOKEN)[:-1] + b"z")
    add("malformed_pipe_count", "malformed", cookie=b"public|missing|field")
    add("missing_user_precedes_bad_mac", "unknown_user", user=False, cookie=good[:-1] + b"z")
    add("uppercase_mac_is_not_lowercase_identity", "bad_mac", cookie=good.rsplit(b"|", 1)[0] + b"|" + lower_tag.upper())
    add("short_mac", "bad_mac", cookie=good[:-1])
    add("nonhex_mac", "bad_mac", cookie=good.rsplit(b"|", 1)[0] + b"|" + b"z" * 64)
    add("tampered_raw_username", "bad_mac", cookie=b"changed" + good[len(USERNAME):])
    add("tampered_token", "bad_mac", cookie=good.replace(TOKEN, b"changed-token"))
    add("tampered_raw_expiration", "bad_mac", cookie=good.replace(str(future).encode(), str(future + 1).encode()))
    add("changed_resolved_salt", "bad_mac", cookie=good, salt=b"other-public-salt")
    add("changed_password_fragment", "bad_mac", cookie=good, password_hash=HASH[:-4] + b"WXYZ")
    add("revoked_same_cookie_as_first_complete_cache_empty", "bad_session", cookie=good, rows=[])
    add("nonmatching_session_verifier", "bad_session", rows=[session(b"different-token", future)])
    add("expired_record_session", "bad_session", rows=[session(TOKEN, past)])
    add("valid_legacy_integer_session", rows=[session(TOKEN, future, True)])
    add("expired_legacy_integer_session", "bad_session", rows=[session(TOKEN, past, True)])
    add("unselected_expired_rows_are_processed_without_removing_selected", rows=[session(b"unselected", past, True), session(TOKEN, future)])
    add("phpass_prefix_offset8_fragment", password_hash=b"$P$12345678abcdTAIL")
    add("short_phpass_prefix_empty_fragment", password_hash=b"$P$")
    add("plain_bcrypt_prefix_offset8_fragment", password_hash=b"$2y$12$abcdefghijklmnop")
    add("short_plain_bcrypt_prefix_empty_fragment", password_hash=b"$2y$")
    add("short_other_hash_uses_entire_tail", password_hash=b"xyz")
    add("empty_other_hash_uses_empty_tail", password_hash=b"")
    add("all_64_prepared_rows_fit_explicit_complete_cache_bound", rows=[session(TOKEN, future)] +
        [session(f"public-extra-{index}".encode(), past if index % 2 else future, bool(index % 2)) for index in range(63)])
    if len(output) != 30:
        raise AssertionError("fixed public case count changed")
    return output
