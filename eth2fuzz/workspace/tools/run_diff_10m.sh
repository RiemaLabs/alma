#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
FUZZ_DIR="$ROOT/workspace/libfuzzer/fuzz"
CORPORA="$ROOT/workspace/corpora"
LOG_DIR="$ROOT/workspace/logs/diff_10m"
mkdir -p "$LOG_DIR"

export CARGO_HOME="$ROOT/workspace/cargo-home"
export ETH2FUZZ_BEACONSTATE="$CORPORA/beaconstate"

# Detect cores and compute workers per target (at least 1)
NCORES=$( (nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1) )
TARGETS=(
  diff_lighthouse_grandine_attestation:$CORPORA/attestation
  diff_lighthouse_grandine_block:$CORPORA/block
  diff_lighthouse_grandine_block_header:$CORPORA/block_header
  diff_lighthouse_grandine_attester_slashing:$CORPORA/attester_slashing
  diff_lighthouse_grandine_proposer_slashing:$CORPORA/proposer_slashing
  diff_lighthouse_grandine_voluntary_exit:$CORPORA/voluntary_exit
  diff_lighthouse_grandine_deposit:$CORPORA/deposit
)
NT=${#TARGETS[@]}
W=$(( NCORES / (NT>0?NT:1) ))
if [ "$W" -lt 1 ]; then W=1; fi

# Avoid over-subscription inside workers
export RAYON_NUM_THREADS=1

echo "Using $NCORES cores total, $NT targets => $W workers/jobs per target"

run_one_bg() {
  local target="$1"; shift
  local corpus_dir="$1"; shift
  echo "[run] target=$target corpus=$corpus_dir time=3600s workers=$W jobs=$W"
  (
    cd "$FUZZ_DIR" && \
    cargo +nightly fuzz run "$target" "$corpus_dir" -- -max_total_time=3600 -workers=$W -jobs=$W \
      2>&1 | tee -a "$LOG_DIR/${target}.log"
  ) || true &
}

for entry in "${TARGETS[@]}"; do
  IFS=":" read -r T C <<< "$entry"
  run_one_bg "$T" "$C"
done

wait || true
echo "All runs finished. Logs under $LOG_DIR"
