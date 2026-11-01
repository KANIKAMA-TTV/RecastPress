"""Record caller-selected source identities without executing or copying code."""
import json
from pathlib import Path
import sys

USAGE = "Usage: python scripts/prepare_migration_source_inventory.py <source-root> <selection.json> <new-dir>"
FAILURE = b"Migration source inventory failed\n"


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
    if len(arguments) != 3:
        return _failed()
    try:
        from migration_source_inventory import inventory_migration_sources

        report = inventory_migration_sources(*(Path(value) for value in arguments))
        encoded = (json.dumps(report, ensure_ascii=True, sort_keys=True,
            separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")
        sys.stdout.buffer.write(encoded)
        sys.stdout.buffer.flush()
    except (Exception, KeyboardInterrupt):
        # Input paths/text and arbitrary IO diagnostics never enter stderr.
        # An output delivery failure retains the already completed inventory.
        return _failed()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
