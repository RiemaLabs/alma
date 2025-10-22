#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
FUZZ_DIR="$ROOT/workspace/libfuzzer/fuzz"
CORPORA="$ROOT/workspace/corpora"
LOG_DIR="$ROOT/workspace/logs/diff_10m"
mkdir -p "$LOG_DIR"

export CARGO_HOME="$ROOT/workspace/cargo-home"
export ETH2FUZZ_BEACONSTATE="$CORPORA/beaconstate"

run_one() {
  local target="$1"; shift
  local corpus_dir="$1"; shift
  echo "[run] target=$target corpus=$corpus_dir time=600s"
  (cd "$FUZZ_DIR" && cargo +nightly fuzz run "$target" "$corpus_dir" -- -max_total_time=600) \
    2>&1 | tee -a "$LOG_DIR/${target}.log" || true
}

run_one diff_lighthouse_grandine_attestation    "$CORPORA/attestation"
run_one diff_lighthouse_grandine_block          "$CORPORA/block"
run_one diff_lighthouse_grandine_block_header   "$CORPORA/block_header"
run_one diff_lighthouse_grandine_attester_slashing "$CORPORA/attester_slashing"
run_one diff_lighthouse_grandine_proposer_slashing "$CORPORA/proposer_slashing"
run_one diff_lighthouse_grandine_voluntary_exit "$CORPORA/voluntary_exit"
run_one diff_lighthouse_grandine_deposit        "$CORPORA/deposit"

echo "All runs finished. Logs under $LOG_DIR"
