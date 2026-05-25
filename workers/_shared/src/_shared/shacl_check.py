"""CLI wrapper — read N-Quads from stdin, exit 0 (conforms) / 1 (violation) / 2 (deps missing)."""

from __future__ import annotations

import sys


def main() -> int:
    try:
        from _shared.shacl import validate_to_json
    except ModuleNotFoundError:
        # Allow the script to run from the workers/_shared/src/ directory
        # without requiring the package to be installed.
        import os

        here = os.path.dirname(os.path.abspath(__file__))
        src = os.path.normpath(os.path.join(here, ".."))
        sys.path.insert(0, src)
        try:
            from _shared.shacl import validate_to_json  # type: ignore  # noqa: F401
        except ModuleNotFoundError:
            print("shacl_check: _shared package not importable", file=sys.stderr)
            return 2
    blob = sys.stdin.read()
    report_json = validate_to_json(blob)
    sys.stdout.write(report_json)
    sys.stdout.write("\n")
    # Exit 0 when the conformance flag is true, else exit 1.
    import json

    parsed = json.loads(report_json)
    return 0 if parsed.get("conforms") else 1


if __name__ == "__main__":
    sys.exit(main())
