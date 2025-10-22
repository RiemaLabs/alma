#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
FUZZ_DIR="$ROOT/workspace/libfuzzer/fuzz"
CORPORA="$ROOT/workspace/corpora"
LOG_DIR="$ROOT/workspace/logs/rl_cov_compare"
mkdir -p "$LOG_DIR"

export CARGO_HOME="$ROOT/workspace/cargo-home"
export ETH2FUZZ_BEACONSTATE="$CORPORA/beaconstate"

DUR=${1:-600}
SEG=${2:-60}

get_targets() {
  (cd "$ROOT" && cargo run -- list | rg '^diff_')
}

corpora_label() {
  (cd "$ROOT" && cargo run -- corpora-label "$1" 2>/dev/null)
}

extract_cov() {
  # Extract first and last coverage numbers from any line that contains 'cov: <num>'
  # Works for both baseline (DONE cov: ...) and RL (pulse cov: ... / REDUCE cov: ...)
  rg -n "cov:\s+[0-9]+" -N "$1" 2>/dev/null | awk '{for(i=1;i<=NF;i++){if($i=="cov:"){print $(i+1)}}}' | \
    awk 'NR==1{fc=$1} {lc=$1} END{ if(NR==0){print "0 0"} else {print fc, lc} }'
}

echo "target,mode,first_cov,last_cov,growth_per_min,logfile"

for t in $(get_targets); do
  label=$(corpora_label "$t")
  if [ -z "$label" ]; then label="attestation"; fi
  # baseline
  BLOG="$LOG_DIR/${t}_baseline.log"
  echo "[baseline] $t for ${DUR}s"
  (cd "$FUZZ_DIR" && cargo +nightly fuzz run "$t" "$CORPORA/$label" -- -max_total_time=$DUR) 2>&1 | tee "$BLOG" >/dev/null || true
  read fc lc < <(extract_cov "$BLOG") || true
  # growth per minute
  gpm=0; if [ "$fc" != "" ] && [ "$lc" != "" ]; then gpm=$(python3 - <<PY
fc=$fc; lc=$lc; dur=$DUR
print((lc-fc)/max(dur/60.0,1.0))
PY
); fi
  echo "$t,baseline,$fc,$lc,$gpm,$BLOG"

  # RL (single-target filter)
  RLOG="$LOG_DIR/${t}_rl.log"
  echo "[rl] $t total=${DUR}s segment=${SEG}s"
  (cd "$ROOT" && cargo run -- rl-fuzz -q "$t" --fuzzer Libfuzzer --total $DUR --segment $SEG) 2>&1 | tee "$RLOG" >/dev/null || true
  read fc2 lc2 < <(extract_cov "$RLOG") || true
  gpm2=0; if [ "$fc2" != "" ] && [ "$lc2" != "" ]; then gpm2=$(python3 - <<PY
fc=$fc2; lc=$lc2; dur=$DUR
print((lc-fc)/max(dur/60.0,1.0))
PY
); fi
  echo "$t,rl,$fc2,$lc2,$gpm2,$RLOG"
done

echo "Done. Logs under $LOG_DIR"
