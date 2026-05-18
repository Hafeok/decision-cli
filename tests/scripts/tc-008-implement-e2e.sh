#!/usr/bin/env bash
# TC-008
# Spec: .product/tests/TC-008-*.md
set -euo pipefail

cat >&2 <<MSG
TC-008 not yet implemented.

dec implement FT-XXX must produce a Session (decision-cli graph) + CodeChange (product-cli graph) linked by PROV-O, with Session.dec:inStream set.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
