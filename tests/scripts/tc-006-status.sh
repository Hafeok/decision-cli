#!/usr/bin/env bash
# TC-006
# Spec: .product/tests/TC-006-*.md
set -euo pipefail

cat >&2 <<MSG
TC-006 not yet implemented.

dec status must display stream identity, definition source, content hash, and ontology version matching what dec init recorded.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
