#!/bin/sh
set -e

TARGET_NAME="${TARGET}"
SEG="${SEG:-2}"
NTHREADS="${NTHREADS:-1}"
ETH2FUZZ_TAG="${ETH2FUZZ_TAG:-default}"

if [ -z "$TARGET_NAME" ]; then
  echo "TARGET env not set" >&2
  exit 1
fi

export CARGO_TERM_PROGRESS_WHEN=never
export ETH2FUZZ_RUN_MODE=base
LOG_DIR="/eth2fuzz/workspace/logs/${ETH2FUZZ_TAG}/base/hfuzz/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/${TARGET_NAME}.log"

# Mark start time (epoch) for offline summarization
echo "HFUZZ_START=$(date +%s)" > "$LOG_FILE"

# Prepare read-only seed copy to avoid writing into workspace/corpora
LABEL="$(/eth2fuzz/eth2fuzz corpora-label "$TARGET_NAME" 2>/dev/null || echo '')"
SEED_DIR="/eth2fuzz/workspace/logs/${ETH2FUZZ_TAG}/base/hfuzz/seed/${TARGET_NAME}"
mkdir -p "$SEED_DIR"
if [ -n "$LABEL" ] && [ -z "$(ls -A "$SEED_DIR" 2>/dev/null)" ]; then
  SRC_DIR="/eth2fuzz/workspace/corpora/${LABEL}"
  if [ -d "$SRC_DIR" ]; then
    # copy once
    cp -f "$SRC_DIR"/* "$SEED_DIR"/ 2>/dev/null || true
  fi
fi

# Run a single segment, using seed dir override, redirecting output to log
ETH2FUZZ_CORPORA_OVERRIDE="$SEED_DIR" /eth2fuzz/eth2fuzz target "$TARGET_NAME" --fuzzer honggfuzz -t "$SEG" -n "$NTHREADS" \
  >> "$LOG_FILE" 2>&1 || true

# Mark end time (epoch)
echo "HFUZZ_END=$(date +%s)" >> "$LOG_FILE"

exit 0
# Mirror new inputs under logs tree for reproducibility
INPUT_DIR="/eth2fuzz/workspace/hfuzz/hfuzz_workspace/${TARGET_NAME}/input"
MIRROR_DIR="/eth2fuzz/workspace/logs/${ETH2FUZZ_TAG}/base/hfuzz/input/${TARGET_NAME}"
mkdir -p "$MIRROR_DIR"
if [ -d "$INPUT_DIR" ]; then
  for f in "$INPUT_DIR"/*; do
    [ -f "$f" ] || continue
    base=$(basename "$f")
    [ -e "$MIRROR_DIR/$base" ] || cp "$f" "$MIRROR_DIR/$base"
  done
fi
