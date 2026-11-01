"""Prepare one fixed Component-generated example for the existing sealed server."""
import json
from pathlib import Path
import sys

USAGE = "Usage: python scripts/prepare_wasm_publication_example.py <new-dir>"
FAILURE = b"Wasm publication example failed\n"


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
        from wasm_publication_example import prepare_example

        observation = prepare_example(Path(arguments[0]))
        encoded = (json.dumps(observation["summary"], ensure_ascii=True,
            sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")
        sys.stdout.buffer.write(encoded)
        sys.stdout.buffer.flush()
    except (Exception, KeyboardInterrupt):
        # Completed or partial artifacts are retained if delivery itself fails.
        return _failed()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
