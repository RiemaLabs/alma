#!/bin/sh
set -e

TARGET_NAME="${TARGET}"
SEG="${SEG:-2}"
NTHREADS="${NTHREADS:-1}"

if [ -z "$TARGET_NAME" ]; then
  echo "TARGET env not set" >&2
  exit 1
fi

export CARGO_TERM_PROGRESS_WHEN=never
LOG_DIR="/eth2fuzz/workspace/hfuzz/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/${TARGET_NAME}.log"

# Mark start time (epoch) for offline summarization
echo "HFUZZ_START=$(date +%s)" > "$LOG_FILE"

# Run a single segment, hide cargo noise by redirecting all output to log
/eth2fuzz/eth2fuzz target "$TARGET_NAME" --fuzzer honggfuzz -t "$SEG" -n "$NTHREADS" \
  >> "$LOG_FILE" 2>&1 || true

# Mark end time (epoch)
echo "HFUZZ_END=$(date +%s)" >> "$LOG_FILE"

exit 0
