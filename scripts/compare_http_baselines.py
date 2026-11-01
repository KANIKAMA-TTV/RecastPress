"""Compare two local HTTP records; differences are observations, not verdicts."""
import json
from pathlib import Path
import sys

USAGE = "Usage: python scripts/compare_http_baselines.py <left-dir> <right-dir>"
FAILURE = b"HTTP baseline comparison failed\n"


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
    if len(arguments) != 2:
        return _failed()
    try:
        from http_baseline_compare import compare_http_baselines

        report = compare_http_baselines(Path(arguments[0]), Path(arguments[1]))
        encoded = (json.dumps(report, ensure_ascii=True, sort_keys=True,
            separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")
        sys.stdout.buffer.write(encoded)
        sys.stdout.buffer.flush()
    except (Exception, KeyboardInterrupt):
        # A partially delivered report cannot be retracted after an IO error.
        return _failed()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
