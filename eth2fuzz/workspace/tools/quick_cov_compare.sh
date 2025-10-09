#!/usr/bin/env bash
set -euo pipefail

# Quick A/B coverage compare for a single target (baseline vs PPO RL)
# Usage: quick_cov_compare.sh [image] [target] [base_secs] [rl_secs] [rl_segment] [threads] [tag]

IMAGE=${1:-eth2fuzz_lighthouse}
TARGET=${2:-lighthouse_attestation}
BASE_SECS=${3:-60}
RL_SECS=${4:-60}
RL_SEG=${5:-10}
THREADS=${6:-2}
TAG=${7:-covtest}

WORKSPACE_DIR="$(cd "$(dirname "$0")/.." && pwd)"

log() { echo "[quick-cov] $*"; }

count_inputs() {
  local d="$WORKSPACE_DIR/hfuzz/hfuzz_workspace/$TARGET/input"
  [ -d "$d" ] || { echo 0; return; }
  find "$d" -type f 2>/dev/null | wc -l | awk '{print $1}'
}

list_inputs() {
  local d="$WORKSPACE_DIR/hfuzz/hfuzz_workspace/$TARGET/input"
  [ -d "$d" ] || { return; }
  find "$d" -type f -maxdepth 1 2>/dev/null | xargs -I{} basename {} | sort -u
}

hash_file() {
  local f="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" 2>/dev/null | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" 2>/dev/null | awk '{print $1}'
  else
    cksum "$f" 2>/dev/null | awk '{print $1":"$2}'
  fi
}

compute_mutated_new() {
  local newlist_file="$1"  # file with new basenames
  local seeds_dir="$2"     # corpora seeds dir
  local input_dir="$WORKSPACE_DIR/hfuzz/hfuzz_workspace/$TARGET/input"
  local seeds_hashes
  seeds_hashes="$(mktemp)"
  if [ -d "$seeds_dir" ]; then
    for s in "$seeds_dir"/*; do [ -f "$s" ] || continue; hash_file "$s"; done | sort -u > "$seeds_hashes"
  else
    echo -n > "$seeds_hashes"
  fi
  local cnt=0
  while read -r name; do
    [ -f "$input_dir/$name" ] || continue
    h=$(hash_file "$input_dir/$name")
    if ! grep -q "$h" "$seeds_hashes"; then cnt=$((cnt+1)); fi
  done < "$newlist_file"
  rm -f "$seeds_hashes"
  echo "$cnt"
}

extract_cov() {
  local f="$WORKSPACE_DIR/logs/${TAG}/$1/hfuzz/logs/${TARGET}.log"
  [ -f "$f" ] || { echo "NA"; return; }
  # search from bottom for branch_coverage_percent: N
  grep "branch_coverage_percent" "$f" | tail -n1 | sed -E 's/.*branch_coverage_percent:([0-9]+).*/\1/' || echo "NA"
}

log "Workspace: $WORKSPACE_DIR"

# Remove stale lock files that may pin old vendored Lighthouse commits
rm -f "$WORKSPACE_DIR/libfuzzer/fuzz/Cargo.lock" \
      "$WORKSPACE_DIR/hfuzz/Cargo.lock" \
      "$WORKSPACE_DIR/targets/rust/Cargo.lock" 2>/dev/null || true

# Prefetch dependencies to avoid spending run time building and refresh locks
make -C "$(dirname "$WORKSPACE_DIR")" prefetch-lighthouse >/dev/null 2>&1 || true

# Baseline
log "Baseline: $TARGET for ${BASE_SECS}s"
BASE_LOG="$WORKSPACE_DIR/logs/${TAG}/base/hfuzz/logs/${TARGET}.log"
BASE_POS=0; [ -f "$BASE_LOG" ] && BASE_POS=$(wc -c < "$BASE_LOG")
BASE_BEFORE=$(count_inputs)
# snapshot pre list
BASE_PRE=$(mktemp)
list_inputs > "$BASE_PRE"
# prepare seed dir copy for baseline to avoid writing into workspace/corpora
LABEL=$(docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace "$IMAGE" corpora-label "$TARGET" 2>/dev/null || echo "")
SEED_DIR="$WORKSPACE_DIR/logs/${TAG}/base/hfuzz/seed/${TARGET}"
mkdir -p "$SEED_DIR"
if [ -n "$LABEL" ] && [ -z "$(ls -A "$SEED_DIR" 2>/dev/null)" ]; then
  SRC_DIR="$WORKSPACE_DIR/corpora/${LABEL}"
  [ -d "$SRC_DIR" ] && cp -f "$SRC_DIR"/* "$SEED_DIR"/ 2>/dev/null || true
fi
docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace -e ETH2FUZZ_TAG="$TAG" -e ETH2FUZZ_CORPORA_OVERRIDE="/eth2fuzz/workspace/logs/${TAG}/base/hfuzz/seed/${TARGET}" "$IMAGE" target "$TARGET" -t "$BASE_SECS" -n "$THREADS" || true
BASE_AFTER=$(count_inputs)
BASE_DELTA=$(( BASE_AFTER - BASE_BEFORE ))
BASE_COV=$(extract_cov base)
BASE_NEW=$( ( [ -f "$BASE_LOG" ] && tail -c +$((BASE_POS+1)) "$BASE_LOG" | grep -oE 'new_units_added:([0-9]+)' | awk -F: '{s+=$2} END{print s+0}' ) || echo 0 )
# compute mutated_new by hashing and excluding corpora seeds
BASE_POST=$(mktemp)
list_inputs > "$BASE_POST"
BASE_DIFF=$(mktemp)
comm -13 "$BASE_PRE" "$BASE_POST" > "$BASE_DIFF"
SEEDS_DIR="$WORKSPACE_DIR/corpora/${LABEL}"
BASE_MUT=$(compute_mutated_new "$BASE_DIFF" "$SEEDS_DIR")
log "Baseline inputs delta: $BASE_DELTA, new_units_added(sum): $BASE_NEW, mutated_new: $BASE_MUT, branch_coverage_percent: $BASE_COV"

# PPO RL
RUN_ID="cov_$(date +%s)"
RL_LOG="$WORKSPACE_DIR/logs/${TAG}/rl/hfuzz/logs/${TARGET}.log"
RL_POS=0; [ -f "$RL_LOG" ] && RL_POS=$(wc -c < "$RL_LOG")
log "PPO RL: $TARGET total=${RL_SECS}s segment=${RL_SEG}s threads=${THREADS} run_id=$RUN_ID"
RL_BEFORE=$(count_inputs)
# snapshot pre list
RL_PRE=$(mktemp)
list_inputs > "$RL_PRE"
docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace "$IMAGE" rl-fuzz -q "$TARGET" --total "$RL_SECS" --segment "$RL_SEG" -n "$THREADS" --config configs/rl_enabled.json --run-id "$RUN_ID" --tag "$TAG" || true
RL_AFTER=$(count_inputs)
RL_DELTA=$(( RL_AFTER - RL_BEFORE ))
RL_COV=$(extract_cov rl)
RL_NEW=$( ( [ -f "$RL_LOG" ] && tail -c +$((RL_POS+1)) "$RL_LOG" | grep -oE 'new_units_added:([0-9]+)' | awk -F: '{s+=$2} END{print s+0}' ) || echo 0 )
# compute mutated_new vs corpora seeds (approximate for RL)
RL_POST=$(mktemp)
list_inputs > "$RL_POST"
RL_DIFF=$(mktemp)
comm -13 "$RL_PRE" "$RL_POST" > "$RL_DIFF"
RL_MUT=$(compute_mutated_new "$RL_DIFF" "$WORKSPACE_DIR/corpora/${LABEL}")
log "RL inputs delta: $RL_DELTA, new_units_added(sum): $RL_NEW, mutated_new: $RL_MUT, branch_coverage_percent: $RL_COV"

echo "--- SUMMARY (target=$TARGET) ---"
echo "Baseline: inputs +$BASE_DELTA, new_units +$BASE_NEW, mutated_new +$BASE_MUT, cov: $BASE_COV%"
echo "RL:       inputs +$RL_DELTA, new_units +$RL_NEW, mutated_new +$RL_MUT, cov: $RL_COV%"

exit 0
