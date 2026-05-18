"""Entry point for the code-writer worker.

Slice 1 contract (FT-013 / ADR-008):
  - Subscribe to the dispatch event stream over SSE (FT-004).
  - Filter for "dispatch available for code-writer" events targeting this worker.
  - Parse the bundle payload (markdown).
  - Call Claude via the anthropic SDK with structured output.
  - Write files to the configured workspace directory.
  - Publish a "dispatch completed" payload back to the harness.

This module is the failing stub — the worker isn't built yet.
"""

import sys


def main() -> int:
    sys.stderr.write(
        "code-writer worker: not yet implemented.\n"
        "See FT-013 / ADR-008 for the contract this worker must satisfy.\n"
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
