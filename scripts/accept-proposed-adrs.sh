#!/usr/bin/env bash
set -euo pipefail

product adr list --format json \
  | jq -r '.[] | select(.status == "proposed") | .id' \
  | while read -r id; do
      echo "accepting $id"
      product adr status "$id" accepted
    done
