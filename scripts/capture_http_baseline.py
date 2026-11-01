"""Experimental explicit-URL HTTP baseline capture; no crawler or publication."""
import sys
from pathlib import Path

USAGE = "Usage: python scripts/capture_http_baseline.py <http-or-https-url> <new-dir>"
SUCCESS = "HTTP baseline recorded\n"
FAILURE = "HTTP baseline capture failed\n"


def _failed():
    try:
        sys.stderr.buffer.write(FAILURE.encode("ascii"))
        sys.stderr.buffer.flush()
    except (OSError, ValueError):
        pass
    return 2


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else list(argv)
    if arguments in (["--help"], ["-h"]):
        try:
            sys.stdout.buffer.write((USAGE + "\n").encode("ascii"))
            sys.stdout.buffer.flush()
        except (OSError, ValueError):
            return _failed()
        return 0
    if len(arguments) != 2:
        return _failed()
    try:
        from http_baseline import capture_http_baseline

        capture_http_baseline(arguments[0], Path(arguments[1]))
        sys.stdout.buffer.write(SUCCESS.encode("ascii"))
        sys.stdout.buffer.flush()
    except (Exception, KeyboardInterrupt):
        # Neither parser/peer diagnostics nor URL/path arguments enter stderr.
        # A failed success-message write retains the already completed files.
        return _failed()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
