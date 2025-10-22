#!/usr/bin/env bash
set -euo pipefail

# Example Grandine oracle wrapper for differential fuzzing.
# Contract:
#   $0 --type attestation --state <state.ssz>
#   SSZ input on stdin
#   Print OK or ERR (or 1/0, true/false) and exit 0

TYPE=""
STATE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --type) TYPE="$2"; shift 2 ;;
    --state) STATE="$2"; shift 2 ;;
    *) echo "[oracle] unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "${TYPE}" || -z "${STATE}" ]]; then
  echo "[oracle] missing --type/--state" >&2
  exit 2
fi

# Read SSZ from stdin to a temp file (if your checker needs a file)
TMP_IN=$(mktemp)
trap 'rm -f "$TMP_IN"' EXIT
cat > "$TMP_IN"

# TODO: Replace the following stub with a call into a Grandine verifier.
# For now, just echo OK to let the diff harness proceed.
echo OK

