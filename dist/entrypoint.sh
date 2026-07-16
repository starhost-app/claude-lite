#!/usr/bin/env bash
set -euo pipefail

: "${BASE_URL:?BASE_URL environment variable is required (e.g. -e BASE_URL=https://...)}"
: "${API_KEY:?API_KEY environment variable is required (e.g. -e API_KEY=sk-...)}"

export ANTHROPIC_BASE_URL="${BASE_URL}"
export ANTHROPIC_AUTH_TOKEN="${API_KEY}"
export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
export CLAUDE_CODE_ATTRIBUTION_HEADER=0

exec claude "$@"
