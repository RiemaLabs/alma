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

extract_cov() {
  local f="$WORKSPACE_DIR/logs/${TAG}/$1/hfuzz/logs/${TARGET}.log"
  [ -f "$f" ] || { echo "NA"; return; }
  # search from bottom for branch_coverage_percent: N
  grep "branch_coverage_percent" "$f" | tail -n1 | sed -E 's/.*branch_coverage_percent:([0-9]+).*/\1/' || echo "NA"
}

log "Workspace: $WORKSPACE_DIR"

# Prefetch dependencies to avoid spending run time building
make -C "$(dirname "$WORKSPACE_DIR")" prefetch-lighthouse >/dev/null 2>&1 || true

# Baseline
log "Baseline: $TARGET for ${BASE_SECS}s"
BASE_LOG="$WORKSPACE_DIR/logs/${TAG}/base/hfuzz/logs/${TARGET}.log"
BASE_POS=0; [ -f "$BASE_LOG" ] && BASE_POS=$(wc -c < "$BASE_LOG")
BASE_BEFORE=$(count_inputs)
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
log "Baseline inputs delta: $BASE_DELTA, new_units_added(sum): $BASE_NEW, branch_coverage_percent: $BASE_COV"

# PPO RL
RUN_ID="cov_$(date +%s)"
RL_LOG="$WORKSPACE_DIR/logs/${TAG}/rl/hfuzz/logs/${TARGET}.log"
RL_POS=0; [ -f "$RL_LOG" ] && RL_POS=$(wc -c < "$RL_LOG")
log "PPO RL: $TARGET total=${RL_SECS}s segment=${RL_SEG}s threads=${THREADS} run_id=$RUN_ID"
RL_BEFORE=$(count_inputs)
docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace "$IMAGE" rl-fuzz -q "$TARGET" --total "$RL_SECS" --segment "$RL_SEG" -n "$THREADS" --config configs/rl_enabled.json --run-id "$RUN_ID" --tag "$TAG" || true
RL_AFTER=$(count_inputs)
RL_DELTA=$(( RL_AFTER - RL_BEFORE ))
RL_COV=$(extract_cov rl)
RL_NEW=$( ( [ -f "$RL_LOG" ] && tail -c +$((RL_POS+1)) "$RL_LOG" | grep -oE 'new_units_added:([0-9]+)' | awk -F: '{s+=$2} END{print s+0}' ) || echo 0 )
log "RL inputs delta: $RL_DELTA, new_units_added(sum): $RL_NEW, branch_coverage_percent: $RL_COV"

echo "--- SUMMARY (target=$TARGET) ---"
echo "Baseline: inputs +$BASE_DELTA, new_units +$BASE_NEW, cov: $BASE_COV%"
echo "RL:       inputs +$RL_DELTA, new_units +$RL_NEW, cov: $RL_COV%"

exit 0
