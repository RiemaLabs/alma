#!/usr/bin/env bash
set -euo pipefail

# Grandine oracle wrapper for differential fuzzing.
#
# Contract with fuzz target:
#   - Args:   --type <kind> --state <path/to/state.ssz>
#   - Stdin:  SSZ bytes of the object to verify (attestation/block/...)
#   - Stdout: print one of OK / ERR / 1 / 0 / true / false
#   - Exit:   0 for success, non-zero for "skip" (fuzzer continues without comparing)
#
# Configuration:
#   - Provide per-type command via env, with placeholders {INPUT} (tmp file with stdin) and {STATE}
#     GRANDINE_ATTESTATION_CMD
#     GRANDINE_BLOCK_CMD
#     GRANDINE_BLOCK_HEADER_CMD
#     GRANDINE_ATTESTER_SLASHING_CMD
#     GRANDINE_PROPOSER_SLASHING_CMD
#     GRANDINE_VOLUNTARY_EXIT_CMD
#     GRANDINE_DEPOSIT_CMD
#   - Or provide a fallback GRANDINE_CMD used for all types.
#   - This wrapper does not assume a specific Grandine CLI; plug the command you use.
#     Example (pseudo):
#       export GRANDINE_ATTESTATION_CMD='grandine verify attestation --state {STATE} --input {INPUT}'
#       export GRANDINE_BLOCK_CMD='grandine verify block --state {STATE} --input {INPUT}'

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

# Read stdin to a temporary file for CLIs that need a path
TMP_INPUT=$(mktemp)
trap 'rm -f "$TMP_INPUT"' EXIT
cat > "$TMP_INPUT"

# Pick the right command template
case "$TYPE" in
  attestation)      CMD_TEMPLATE="${GRANDINE_ATTESTATION_CMD:-}" ;;
  block)            CMD_TEMPLATE="${GRANDINE_BLOCK_CMD:-}" ;;
  block_header)     CMD_TEMPLATE="${GRANDINE_BLOCK_HEADER_CMD:-}" ;;
  attester_slashing)CMD_TEMPLATE="${GRANDINE_ATTESTER_SLASHING_CMD:-}" ;;
  proposer_slashing)CMD_TEMPLATE="${GRANDINE_PROPOSER_SLASHING_CMD:-}" ;;
  voluntary_exit)   CMD_TEMPLATE="${GRANDINE_VOLUNTARY_EXIT_CMD:-}" ;;
  deposit)          CMD_TEMPLATE="${GRANDINE_DEPOSIT_CMD:-}" ;;
  *) echo "[oracle] unsupported type: $TYPE" >&2; exit 3 ;;
esac

if [[ -z "${CMD_TEMPLATE:-}" ]]; then
  CMD_TEMPLATE="${GRANDINE_CMD:-}"
fi

if [[ -z "${CMD_TEMPLATE:-}" ]]; then
  # No command configured; signal the fuzzer to skip comparison
  echo "[oracle] GRANDINE_CMD not set; skipping" >&2
  exit 3
fi

# Substitute placeholders
CMD=${CMD_TEMPLATE//\{STATE\}/$STATE}
CMD=${CMD//\{INPUT\}/$TMP_INPUT}

# Run the command in a shell to allow complex templates (pipes, redirects)
if ! OUT=$(bash -o pipefail -c "$CMD" 2>/dev/null); then
  # Signal skip to the fuzzer
  exit 3
fi

OUT_LOWER=$(echo "$OUT" | tr '[:upper:]' '[:lower:]' | tr -d '\r' | tr -d '\n' )
if [[ "$OUT_LOWER" == *"ok"* || "$OUT_LOWER" == "1" || "$OUT_LOWER" == "true" ]]; then
  echo OK
  exit 0
elif [[ "$OUT_LOWER" == *"err"* || "$OUT_LOWER" == "0" || "$OUT_LOWER" == "false" ]]; then
  echo ERR
  exit 0
else
  # Unknown output; skip
  exit 3
fi

