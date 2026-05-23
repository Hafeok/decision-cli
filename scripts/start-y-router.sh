#!/usr/bin/env bash
# scripts/start-y-router.sh — Start the y-router proxy for FT-066.
#
# y-router (https://github.com/luohy15/y-router) translates Anthropic API
# calls into OpenAI-compatible calls so Claude Code can route through
# Scaleway's Generative APIs per ADR-037. The implementer's subprocess
# spawn (workers/code-writer/src/code_writer/env_routing.py, FT-066)
# points `claude -p` at $DEC_YROUTER_URL (default http://localhost:8787)
# whenever the resolved capability has endpoint=scaleway.
#
# This script automates the one-time setup the operator would otherwise
# do by hand: clone y-router, configure wrangler.toml so it forwards to
# Scaleway, run `docker compose up -d`, poll until the proxy responds.
# Idempotent — re-running while the proxy is already up is a no-op.
#
# y-router lives under $DEC_YROUTER_HOME (default
# ${XDG_DATA_HOME:-$HOME/.local/share}/decision-cli/y-router). The proxy
# is a per-machine resource — one TCP port, one container — so
# machine-global storage avoids re-cloning per decision-cli workdir.
#
# Exit 0: proxy responds on $DEC_YROUTER_URL when the script exits.
# Exit 1: prereq missing, clone/config failed, or readiness probe timed out.

set -euo pipefail

DEC_YROUTER_URL=${DEC_YROUTER_URL:-http://localhost:8787}
DEC_YROUTER_HOME=${DEC_YROUTER_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/decision-cli/y-router}
SCALEWAY_BASE_URL=${SCALEWAY_BASE_URL:-https://api.scaleway.ai/v1}
READY_TIMEOUT_S=${READY_TIMEOUT_S:-30}

log() { printf '%s\n' "$*"; }

require_bin() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log "ERROR: required binary '$1' not on PATH"
        exit 1
    fi
}

for bin in git docker curl; do
    require_bin "$bin"
done

if ! docker compose version >/dev/null 2>&1; then
    log "ERROR: 'docker compose' v2 plugin not available — install it and retry"
    exit 1
fi

# Idempotent fast path: already responding.
if curl -sf --max-time 1 "$DEC_YROUTER_URL/" >/dev/null 2>&1; then
    log "y-router already responding at $DEC_YROUTER_URL — nothing to do"
    if [ -z "${SCW_SECRET_KEY:-}" ]; then
        log "NOTE: \$SCW_SECRET_KEY is unset — dispatches will fail until you export it"
    fi
    exit 0
fi

# Clone if missing.
if [ ! -d "$DEC_YROUTER_HOME/.git" ]; then
    log "cloning y-router into $DEC_YROUTER_HOME"
    mkdir -p "$(dirname "$DEC_YROUTER_HOME")"
    git clone --depth 1 https://github.com/luohy15/y-router.git "$DEC_YROUTER_HOME"
fi

cd "$DEC_YROUTER_HOME"

WRANGLER=wrangler.toml
if [ ! -f "$WRANGLER" ]; then
    log "ERROR: $DEC_YROUTER_HOME/$WRANGLER missing — y-router repo layout changed?"
    exit 1
fi

# Configure forwarding target idempotently. Replace any existing [vars]
# block (only OPENROUTER_BASE_URL) so re-runs converge on the canonical
# shape regardless of prior state.
if ! grep -qE "^OPENROUTER_BASE_URL[[:space:]]*=[[:space:]]*\"${SCALEWAY_BASE_URL//\//\\/}\"" "$WRANGLER"; then
    log "configuring $WRANGLER → OPENROUTER_BASE_URL=$SCALEWAY_BASE_URL"
    python3 - "$WRANGLER" "$SCALEWAY_BASE_URL" <<'PY'
import sys, re, pathlib
path = pathlib.Path(sys.argv[1])
url = sys.argv[2]
src = path.read_text()
# Strip any existing decision-cli-managed [vars] block. The naïve regex
# matches the canonical shape we write; non-canonical operator edits are
# left alone (we just append a fresh one which takes precedence).
src = re.sub(r"\n\[vars\]\s*\nOPENROUTER_BASE_URL\s*=\s*\"[^\"]*\"\s*\n", "\n", src)
if not src.endswith("\n"):
    src += "\n"
src += f"\n[vars]\nOPENROUTER_BASE_URL = \"{url}\"\n"
path.write_text(src)
PY
fi

log "starting y-router via docker compose"
docker compose up -d

log "waiting up to ${READY_TIMEOUT_S}s for proxy on $DEC_YROUTER_URL"
deadline=$(( $(date +%s) + READY_TIMEOUT_S ))
while [ "$(date +%s)" -lt "$deadline" ]; do
    if curl -sf --max-time 1 "$DEC_YROUTER_URL/" >/dev/null 2>&1; then
        log "y-router up at $DEC_YROUTER_URL"
        if [ -z "${SCW_SECRET_KEY:-}" ]; then
            log "NOTE: \$SCW_SECRET_KEY is unset — dispatches will fail until you export it"
        fi
        exit 0
    fi
    sleep 1
done

log "ERROR: proxy did not respond within ${READY_TIMEOUT_S}s — check 'docker compose logs' in $DEC_YROUTER_HOME"
exit 1
