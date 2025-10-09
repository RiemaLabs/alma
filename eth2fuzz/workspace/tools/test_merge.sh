#!/usr/bin/env bash
set -euo pipefail

# Simple one-shot test for libFuzzer merge (global coverage contribution)
# Usage:
#   workspace/tools/test_merge.sh <target> <global_dir> <new_dir> [beaconstate]
# Example:
#   workspace/tools/test_merge.sh lighthouse_attestation \
#     workspace/logs/merge_test/global \
#     workspace/logs/merge_test/new \
#     workspace/corpora/beaconstate

if [ "$#" -lt 3 ]; then
  echo "Usage: $0 <target> <global_dir> <new_dir> [beaconstate]" >&2
  exit 1
fi

TARGET="$1"
GLOBAL_DIR_HOST="$2"
NEW_DIR_HOST="$3"
BEACONSTATE_HOST="${4:-eth2fuzz/workspace/corpora/beaconstate}"

# Resolve to absolute paths
ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"    # .../eth2fuzz
WS_HOST="$ROOT_DIR/workspace"                         # .../eth2fuzz/workspace

# Normalize input paths to absolute
abspath() {
  python3 - "$1" <<'PY'
import os,sys
print(os.path.abspath(sys.argv[1]))
PY
}

GLOBAL_DIR_HOST_ABS="$(abspath "$GLOBAL_DIR_HOST")"
NEW_DIR_HOST_ABS="$(abspath "$NEW_DIR_HOST")"
BEACONSTATE_HOST_ABS="$(abspath "$BEACONSTATE_HOST")"

# Ensure dirs exist
mkdir -p "$GLOBAL_DIR_HOST_ABS" "$NEW_DIR_HOST_ABS"

# Ensure they are under workspace
case "$GLOBAL_DIR_HOST_ABS" in
  "$WS_HOST"/*) ;;
  *) echo "global_dir must be under $WS_HOST" >&2; exit 2;;
esac
case "$NEW_DIR_HOST_ABS" in
  "$WS_HOST"/*) ;;
  *) echo "new_dir must be under $WS_HOST" >&2; exit 2;;
esac
case "$BEACONSTATE_HOST_ABS" in
  "$WS_HOST"/*) ;;
  *) echo "beaconstate must be under $WS_HOST" >&2; exit 2;;
esac

# Convert to container paths
GLOBAL_DIR_CTN="/eth2fuzz/workspace${GLOBAL_DIR_HOST_ABS#${WS_HOST}}"
NEW_DIR_CTN="/eth2fuzz/workspace${NEW_DIR_HOST_ABS#${WS_HOST}}"
BEACONSTATE_CTN="/eth2fuzz/workspace${BEACONSTATE_HOST_ABS#${WS_HOST}}"

echo "[test-merge] target=$TARGET"
echo "[test-merge] global_dir(host)=$GLOBAL_DIR_HOST_ABS"
echo "[test-merge] new_dir(host)=$NEW_DIR_HOST_ABS"

# Snapshot before
BEFORE_LIST="$(mktemp)"; AFTER_LIST="$(mktemp)"
# portable listing of file basenames in dir (avoid BSD find -printf)
(
  shopt -s nullglob 2>/dev/null || true
  for f in "$GLOBAL_DIR_HOST_ABS"/*; do [ -f "$f" ] && basename "$f"; done
) | sort -u > "$BEFORE_LIST" || true
BEFORE_CNT=$(wc -l < "$BEFORE_LIST" | tr -d ' ')
echo "[test-merge] before count: $BEFORE_CNT"

# Run merge in container
set -x
docker run --rm -v "$WS_HOST":/eth2fuzz/workspace \
  -e ETH2FUZZ_BEACONSTATE="$BEACONSTATE_CTN" \
  --entrypoint bash \
  eth2fuzz_lighthouse -lc "set -e; cd /eth2fuzz/workspace/libfuzzer/fuzz; \
    cargo +nightly fuzz run $TARGET -- -merge=1 -runs=0 \"$GLOBAL_DIR_CTN\" \"$NEW_DIR_CTN\""
set +x

# Snapshot after
(
  shopt -s nullglob 2>/dev/null || true
  for f in "$GLOBAL_DIR_HOST_ABS"/*; do [ -f "$f" ] && basename "$f"; done
) | sort -u > "$AFTER_LIST" || true
AFTER_CNT=$(wc -l < "$AFTER_LIST" | tr -d ' ')
echo "[test-merge] after count:  $AFTER_CNT"

ADDED_LIST="$(mktemp)"
comm -13 "$BEFORE_LIST" "$AFTER_LIST" > "$ADDED_LIST" || true
ADDED_CNT=$(wc -l < "$ADDED_LIST" | tr -d ' ')
echo "[test-merge] added:       $ADDED_CNT"
if [ "$ADDED_CNT" -gt 0 ]; then
  echo "[test-merge] added files:"
  sed -n '1,200p' "$ADDED_LIST"
fi

rm -f "$BEFORE_LIST" "$AFTER_LIST" "$ADDED_LIST"
exit 0
