"""Compare an explicit local pair list without fetching or migration verdicts."""
import json
from pathlib import Path
import sys

USAGE = "Usage: python scripts/compare_http_baseline_pairs.py <pairs.json>"
FAILURE = b"HTTP baseline pair comparison failed\n"


def _failed():
    try:
        sys.stderr.buffer.write(FAILURE)
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
    if len(arguments) != 1:
        return _failed()
    try:
        from http_baseline_pair_list import compare_http_baseline_pairs

        report = compare_http_baseline_pairs(Path(arguments[0]))
        encoded = (json.dumps(report, ensure_ascii=True, sort_keys=True,
            separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")
        sys.stdout.buffer.write(encoded)
        sys.stdout.buffer.flush()
    except (Exception, KeyboardInterrupt):
        # Neither partial comparisons nor input/peer diagnostics are printed.
        # A failed delivery cannot retract bytes already written to the pipe.
        return _failed()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
