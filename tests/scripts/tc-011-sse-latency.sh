#!/usr/bin/env bash
# TC-011
# Spec: .product/tests/TC-011-*.md
set -euo pipefail

cat >&2 <<MSG
TC-011 not yet implemented.

Remote Python worker must receive every dispatch event within 1.000s of emission across N>=10 successive dispatches.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
