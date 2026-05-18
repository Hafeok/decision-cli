#!/usr/bin/env bash
# TC-013
# Spec: .product/tests/TC-013-*.md
set -euo pipefail

cat >&2 <<MSG
TC-013 not yet implemented.

Every CodeChange in product-cli's graph must resolve a Session in decision-cli's graph via PROV-O (cross-graph).

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
