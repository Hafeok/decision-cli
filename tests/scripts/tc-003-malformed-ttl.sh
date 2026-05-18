#!/usr/bin/env bash
# TC-003
# Spec: .product/tests/TC-003-*.md
set -euo pipefail

cat >&2 <<MSG
TC-003 not yet implemented.

dec init --from <malformed.ttl> must fail with a SHACL message and write NO state.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
