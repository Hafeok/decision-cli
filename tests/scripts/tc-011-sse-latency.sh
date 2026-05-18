#!/usr/bin/env bash
# TC-011 — SSE dispatch event delivered within one second (FT-003 / FT-004)
# Spec: .product/tests/TC-011-sse-dispatch-event-delivered-within-one-second.md
#
# Drives the axum SSE router through a real HTTP client (reqwest) so the
# measured latency includes chunked-transfer framing and TCP delivery —
# not just an in-process broadcast. For N >= 10 successive dispatches we
# assert `t_recv - t_emit < 1.000 s`. The Python worker hop will be
# bolted on once FT-013 lands and the slice-1 binary surface exists.
set -euo pipefail

cd "$(dirname "$0")/../.."

exec cargo test \
    --quiet \
    -p oxi-events \
    --test tc_011_sse_latency \
    -- --nocapture --test-threads=1
