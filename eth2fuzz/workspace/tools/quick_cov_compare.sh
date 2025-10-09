#!/usr/bin/env bash
set -euo pipefail

# Quick A/B coverage compare for a single target (baseline vs PPO RL)
# Usage: quick_cov_compare.sh [image] [target] [base_secs] [rl_secs] [rl_segment] [threads] [tag] [fuzzer]

IMAGE=${1:-eth2fuzz_lighthouse}
TARGET=${2:-lighthouse_attestation}
BASE_SECS=${3:-60}
RL_SECS=${4:-60}
RL_SEG=${5:-10}
THREADS=${6:-2}
TAG=${7:-covtest}
FUZZER=${8:-honggfuzz}
# Optional overrides (baseline runs as a single segment without bins by default)
BASE_SEG=${BASE_SEG:-$BASE_SECS}
BASE_RL_CONFIG=${BASE_RL_CONFIG:-configs/rl_baseline.json}

WORKSPACE_DIR="$(cd "$(dirname "$0")/.." && pwd)"

log() { echo "[quick-cov] $*"; }

count_inputs_dir() {
  local d="$1"; local recursive="${2:-0}"
  [ -d "$d" ] || { echo 0; return 0; }
  if [ "$recursive" = "1" ]; then
    find "$d" -type f 2>/dev/null | wc -l | awk '{print $1}'
  else
    find "$d" -type f -maxdepth 1 2>/dev/null | wc -l | awk '{print $1}'
  fi
}

list_inputs_dir() {
  local d="$1"; local recursive="${2:-0}"
  [ -d "$d" ] || { return 0; }
  if [ "$recursive" = "1" ]; then
    find "$d" -type f 2>/dev/null | sed -e "s#^$d/##" | sort -u
  else
    find "$d" -type f -maxdepth 1 2>/dev/null | sed -e "s#^$d/##" | sort -u
  fi
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
  local newlist_file="$1"      # file with new basenames
  local seeds_ref="$2"          # either: a dir of seed files OR a precomputed hashes file
  local input_dir="$3"         # directory where new files were written
  local seeds_hashes_file
  if [ -f "$seeds_ref" ] && [ ! -d "$seeds_ref" ]; then
    seeds_hashes_file="$seeds_ref"
  else
    seeds_hashes_file="$(mktemp)"
    if [ -d "$seeds_ref" ]; then
      for s in "$seeds_ref"/*; do [ -f "$s" ] || continue; hash_file "$s"; done | sort -u > "$seeds_hashes_file"
    else
      echo -n > "$seeds_hashes_file"
    fi
  fi
  local cnt=0
  while read -r name; do
    [ -f "$input_dir/$name" ] || continue
    h=$(hash_file "$input_dir/$name")
    if ! grep -q "$h" "$seeds_hashes_file"; then cnt=$((cnt+1)); fi
  done < "$newlist_file"
  # Only cleanup if we created a temp file
  if [ "$(dirname "$seeds_hashes_file")" = "/tmp" ] || [[ "$seeds_hashes_file" == /tmp/* ]]; then
    rm -f "$seeds_hashes_file"
  fi
  echo "$cnt"
}

# Run libFuzzer offline merge to count coverage-contributing samples
# Args: <global_corpus_dir> <newlist_file> <input_dir> <target> <image>
merge_count_cov() {
  local global_dir="$1"
  local newlist_file="$2"
  local input_dir="$3"
  local target="$4"
  local image="$5"
  mkdir -p "$global_dir"
  # materialize just the new files into a temp dir to avoid re-merging the whole input
  local tmp_newdir="$WORKSPACE_DIR/logs/${TAG}/merge_tmp/${target}/$RANDOM"
  mkdir -p "$tmp_newdir"
  local limit="${MERGE_LIMIT:-2048}"; local i=0
  while read -r name; do
    i=$((i+1)); [ "$i" -gt "$limit" ] && break
    [ -f "$input_dir/$name" ] || continue
    cp -f "$input_dir/$name" "$tmp_newdir/" 2>/dev/null || true
  done < "$newlist_file"
  local pre
  pre=$(find "$global_dir" -type f -maxdepth 1 2>/dev/null | wc -l | awk '{print $1}')
  # perform merge using the libFuzzer harness for the target
  docker run --rm -v "$WORKSPACE_DIR":/eth2fuzz/workspace --entrypoint /bin/sh "$image" -lc \
    "set -e; cd /eth2fuzz/workspace/libfuzzer/fuzz; \
     timeout ${MERGE_TIMEOUT:-180}s cargo +nightly fuzz run $target -- -merge=1 -runs=0 \
       /eth2fuzz/workspace${global_dir#${WORKSPACE_DIR}} \
       /eth2fuzz/workspace${tmp_newdir#${WORKSPACE_DIR}}" >/dev/null 2>&1 || true
  local post
  post=$(find "$global_dir" -type f -maxdepth 1 2>/dev/null | wc -l | awk '{print $1}')
  local delta=$(( post - pre ))
  [ "$delta" -lt 0 ] && delta=0
  echo "$delta"
}

# Filter a list of basenames by excluding any that hash-match seeds in a hashes file
# Args: <newlist_file> <seeds_hashes_file> <input_dir> <out_file>
filter_newlist_by_hashes() {
  local newlist_file="$1"; local seeds_hashes_file="$2"; local input_dir="$3"; local out_file="$4"
  : > "$out_file"
  while read -r name; do
    [ -f "$input_dir/$name" ] || continue
    local h
    h=$(hash_file "$input_dir/$name")
    if ! grep -q "$h" "$seeds_hashes_file"; then echo "$name" >> "$out_file"; fi
  done < "$newlist_file"
}

# Prebuild the libFuzzer harness once to avoid long compile during merges
prebuild_harness() {
  docker run --rm -v "$WORKSPACE_DIR":/eth2fuzz/workspace --entrypoint /bin/sh "$IMAGE" -lc \
    "set -e; cd /eth2fuzz/workspace/libfuzzer/fuzz; cargo +nightly fuzz build $TARGET" >/dev/null 2>&1 || true
}

extract_cov() {
  local f="$WORKSPACE_DIR/logs/${TAG}/$1/hfuzz/logs/${TARGET}.log"
  [ -f "$f" ] || { echo "NA"; return; }
  # search from bottom for branch_coverage_percent: N
  grep "branch_coverage_percent" "$f" | tail -n1 | sed -E 's/.*branch_coverage_percent:([0-9]+).*/\1/' || echo "NA"
}

log "Workspace: $WORKSPACE_DIR (fuzzer=$FUZZER)"

# Remove stale lock files that may pin old vendored Lighthouse commits
rm -f "$WORKSPACE_DIR/libfuzzer/fuzz/Cargo.lock" \
      "$WORKSPACE_DIR/hfuzz/Cargo.lock" \
      "$WORKSPACE_DIR/targets/rust/Cargo.lock" 2>/dev/null || true

# Prefetch dependencies to avoid spending run time building and refresh locks
make -C "$(dirname "$WORKSPACE_DIR")" prefetch-lighthouse >/dev/null 2>&1 || true
# Warm build to amortize merge-time compiles
prebuild_harness

# Prepare per-run seed/input dirs depending on fuzzer
LABEL=$(docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace "$IMAGE" corpora-label "$TARGET" 2>/dev/null || echo "")
BASE_SEED_DIR="$WORKSPACE_DIR/logs/${TAG}/base/seed/${TARGET}"
RL_SEED_DIR="$WORKSPACE_DIR/logs/${TAG}/rl/seed/${TARGET}"
mkdir -p "$BASE_SEED_DIR" "$RL_SEED_DIR"

if [ -n "$LABEL" ] && [ -z "$(ls -A "$BASE_SEED_DIR" 2>/dev/null)" ]; then
  SRC_DIR="$WORKSPACE_DIR/corpora/${LABEL}"
  [ -d "$SRC_DIR" ] && cp -f "$SRC_DIR"/* "$BASE_SEED_DIR"/ 2>/dev/null || true
fi
if [ -n "$LABEL" ] && [ -z "$(ls -A "$RL_SEED_DIR" 2>/dev/null)" ]; then
  SRC_DIR="$WORKSPACE_DIR/corpora/${LABEL}"
  [ -d "$SRC_DIR" ] && cp -f "$SRC_DIR"/* "$RL_SEED_DIR"/ 2>/dev/null || true
fi

# Input dirs to watch for new files
case "$FUZZER" in
  honggfuzz)
    BASE_INPUT_DIR="$WORKSPACE_DIR/hfuzz/hfuzz_workspace/$TARGET/input"
    # RL segments copy seeds into logs/<tag>/rl/rl_input/<target>/seg_*
    RL_INPUT_DIR="$WORKSPACE_DIR/logs/${TAG}/rl/rl_input/${TARGET}"
    BASE_RECURSIVE=0; RL_RECURSIVE=1
    ;;
  libfuzzer)
    BASE_INPUT_DIR="$BASE_SEED_DIR"              # baseline writes into its seed dir
    RL_INPUT_DIR="$WORKSPACE_DIR/logs/${TAG}/rl/rl_input/${TARGET}"  # RL engine writes per-seg here
    BASE_RECURSIVE=0; RL_RECURSIVE=1
    ;;
  afl)
    BASE_INPUT_DIR="$WORKSPACE_DIR/afl/afl_workspace/queue"
    RL_INPUT_DIR="$WORKSPACE_DIR/afl/afl_workspace/queue"
    BASE_RECURSIVE=0; RL_RECURSIVE=0
    ;;
  *)
    BASE_INPUT_DIR="$WORKSPACE_DIR/hfuzz/hfuzz_workspace/$TARGET/input"
    RL_INPUT_DIR="$WORKSPACE_DIR/hfuzz/hfuzz_workspace/$TARGET/input"
    BASE_RECURSIVE=0; RL_RECURSIVE=0
    ;;
esac

# Snapshot seed hashes prior to runs
BASE_SEED_HASHES="$(mktemp)"; for s in "$BASE_SEED_DIR"/*; do [ -f "$s" ] || continue; hash_file "$s"; done | sort -u > "$BASE_SEED_HASHES"
RL_SEED_HASHES="$(mktemp)"; for s in "$RL_SEED_DIR"/*; do [ -f "$s" ] || continue; hash_file "$s"; done | sort -u > "$RL_SEED_HASHES"

# Baseline
log "Baseline: $TARGET for ${BASE_SECS}s (via rl-fuzz)"
# Run baseline via RL engine to unify metrics; use separate tag to avoid mixing with RL outputs
BASE_TAG="${TAG}_base"
BASE_RUN_ID="base_$(date +%s)"
# ensure seed dir exists and seeded
mkdir -p "$BASE_SEED_DIR"
if [ -n "$LABEL" ] && [ -z "$(ls -A "$BASE_SEED_DIR" 2>/dev/null)" ]; then
  SRC_DIR="$WORKSPACE_DIR/corpora/${LABEL}"
  [ -d "$SRC_DIR" ] && cp -f "$SRC_DIR"/* "$BASE_SEED_DIR"/ 2>/dev/null || true
fi
# run baseline RL
docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace \
  -e ETH2FUZZ_RUN_MODE="base" \
  -e ETH2FUZZ_TAG="$BASE_TAG" \
  -e ETH2FUZZ_CORPORA_OVERRIDE="/eth2fuzz/workspace/logs/${TAG}/base/seed/${TARGET}" \
  "$IMAGE" rl-fuzz -q "$TARGET" --fuzzer "$FUZZER" --total "$BASE_SECS" --segment "$BASE_SEG" -n "$THREADS" --config "$BASE_RL_CONFIG" --run-id "$BASE_RUN_ID" --tag "$BASE_TAG" || true
# parse baseline stats
BASE_STATS="$WORKSPACE_DIR/logs/${BASE_TAG}/rl/rl_runs/$BASE_RUN_ID/stats.json"
BASE_MUT_COV="0"; BASE_MUT="0"; BASE_COV="NA"; BASE_NEW="0"; BASE_DELTA="0"
if [ -f "$BASE_STATS" ]; then
  BASE_MUT_COV=$(grep -o '"mutated_new_cov"[[:space:]]*:[[:space:]]*[0-9]\+' "$BASE_STATS" 2>/dev/null | awk -F: '{s+=$2} END{print s+0}' || echo 0)
  # mutated_new aligns to coverage-contributing samples for non-libfuzzer
  BASE_MUT="$BASE_MUT_COV"
  # coverage: take last non-empty branch_cov_pct from stats or fallback to baseline RL hfuzz log
  tmpcov=$(grep -o '"branch_cov_pct"[[:space:]]*:[[:space:]]*[0-9]\+\.?[0-9]*' "$BASE_STATS" 2>/dev/null | tail -n1 | awk -F: '{print $2+0}' || echo "")
  if [ -n "$tmpcov" ]; then
    BASE_COV="$tmpcov"
  else
    BASE_HFUZZ_LOG="$WORKSPACE_DIR/logs/${BASE_TAG}/rl/hfuzz/logs/${TARGET}.log"
    if [ -f "$BASE_HFUZZ_LOG" ]; then
      tmpcov=$(grep -o 'branch_coverage_percent:[[:space:]]*[0-9]\+' "$BASE_HFUZZ_LOG" 2>/dev/null | tail -n1 | awk -F: '{print $2+0}' || echo "")
      [ -n "$tmpcov" ] && BASE_COV="$tmpcov"
    fi
  fi
  # new_units from hfuzz log if present under baseline tag
  BASE_LOG="$WORKSPACE_DIR/logs/${BASE_TAG}/rl/hfuzz/logs/${TARGET}.log"
  if [ -f "$BASE_LOG" ]; then
    BASE_NEW=$(grep -oE 'new_units_added:([0-9]+)' "$BASE_LOG" 2>/dev/null | awk -F: '{s+=$2} END{print s+0}' || echo 0)
fi
fi
log "Baseline (rl): mutated_new_cov=$BASE_MUT_COV, cov=$BASE_COV, new_units_added(sum)=$BASE_NEW"

# PPO RL
RUN_ID="cov_$(date +%s)"
RL_LOG="$WORKSPACE_DIR/logs/${TAG}/rl/hfuzz/logs/${TARGET}.log"
RL_POS=0; [ -f "$RL_LOG" ] && RL_POS=$(wc -c < "$RL_LOG")
log "PPO RL: $TARGET total=${RL_SECS}s segment=${RL_SEG}s threads=${THREADS} run_id=$RUN_ID"
# Reset RL input dir to avoid accumulation across runs
RL_INPUT_DIR_ROOT="$WORKSPACE_DIR/logs/${TAG}/rl/rl_input/${TARGET}"
rm -rf "$RL_INPUT_DIR_ROOT" 2>/dev/null || true
mkdir -p "$RL_INPUT_DIR_ROOT" || true
RL_BEFORE=$(count_inputs_dir "$RL_INPUT_DIR" "$RL_RECURSIVE")
# snapshot pre list
RL_PRE=$(mktemp)
list_inputs_dir "$RL_INPUT_DIR" "$RL_RECURSIVE" > "$RL_PRE"
docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace \
  -e ETH2FUZZ_RUN_MODE="rl" \
  -e ETH2FUZZ_CORPORA_OVERRIDE="/eth2fuzz/workspace/logs/${TAG}/rl/seed/${TARGET}" \
  "$IMAGE" rl-fuzz -q "$TARGET" --fuzzer "$FUZZER" --total "$RL_SECS" --segment "$RL_SEG" -n "$THREADS" --config configs/rl_enabled.json --run-id "$RUN_ID" --tag "$TAG" || true
RL_AFTER=$(count_inputs_dir "$RL_INPUT_DIR" "$RL_RECURSIVE")
RL_DELTA=$(( RL_AFTER - RL_BEFORE ))
RL_COV=$(extract_cov rl)
RL_NEW=$( ( [ -f "$RL_LOG" ] && tail -c +$((RL_POS+1)) "$RL_LOG" | grep -oE 'new_units_added:([0-9]+)' | awk -F: '{s+=$2} END{print s+0}' ) || echo 0 )
# compute mutated_new vs corpora seeds (approximate for RL)
RL_POST=$(mktemp)
list_inputs_dir "$RL_INPUT_DIR" "$RL_RECURSIVE" > "$RL_POST"
RL_DIFF=$(mktemp)
comm -13 "$RL_PRE" "$RL_POST" > "$RL_DIFF"
RL_MUT=$(compute_mutated_new "$RL_DIFF" "$RL_SEED_HASHES" "$RL_INPUT_DIR")

# Prefer RL engine stats for mutated_new_cov if available (any fuzzer)
RL_STATS="$WORKSPACE_DIR/logs/${TAG}/rl/rl_runs/$RUN_ID/stats.json"
RL_MUT_COV="NA"
if [ -f "$RL_STATS" ]; then
  RL_MUT_COV=$(grep -o '"mutated_new_cov"[[:space:]]*:[[:space:]]*[0-9]\+' "$RL_STATS" 2>/dev/null | awk -F: '{s+=$2} END{print s+0}' || echo 0)
fi
# Align non-libfuzzer mutated_new to coverage-contributing count
if [ "$FUZZER" != "libfuzzer" ] && [ "$RL_MUT_COV" != "NA" ]; then
  RL_MUT=$RL_MUT_COV
fi
log "RL inputs delta: $RL_DELTA, new_units_added(sum): $RL_NEW, mutated_new: $RL_MUT, mutated_new_cov: $RL_MUT_COV, branch_coverage_percent: $RL_COV"

# If needed, compute mutated_new_cov via offline merge for baseline (when stats missing)
if [ "$BASE_MUT_COV" = "0" ] || [ "$BASE_MUT_COV" = "NA" ]; then
  BASE_GLOBAL_CORP="$WORKSPACE_DIR/logs/${TAG}/base/libfuzzer_corpus/${TARGET}"
  # Build a synthetic new list from baseline seed dir changes (if any); otherwise skip
  BASE_PRE_LIST="$(mktemp)"; BASE_POST_LIST="$(mktemp)"; BASE_DIFF_LIST="$(mktemp)"
  list_inputs_dir "$BASE_SEED_DIR" 0 > "$BASE_PRE_LIST"
  # no reliable baseline pre snapshot; skip producing a diff if empty
  # Merge entire seed as a last resort (bounded by MERGE_LIMIT)
  list_inputs_dir "$BASE_SEED_DIR" 0 > "$BASE_POST_LIST"
  comm -13 "$BASE_PRE_LIST" "$BASE_POST_LIST" > "$BASE_DIFF_LIST" || true
  BASE_MUT_COV=$(merge_count_cov "$BASE_GLOBAL_CORP" "$BASE_DIFF_LIST" "$BASE_SEED_DIR" "$TARGET" "$IMAGE")
  BASE_MUT="$BASE_MUT_COV"
fi

# RL engine writes stats under logs/<tag>/rl/rl_runs/<run_id>/stats.json
if [ "$RL_MUT_COV" = "NA" ]; then
  RL_GLOBAL_CORP="$WORKSPACE_DIR/logs/${TAG}/rl/libfuzzer_corpus/${TARGET}"
  # Filter out pure seeds from the RL new-file list before merging
  RL_MUT_LIST="$(mktemp)"; filter_newlist_by_hashes "$RL_DIFF" "$RL_SEED_HASHES" "$RL_INPUT_DIR" "$RL_MUT_LIST"
  RL_MUT_COV=$(merge_count_cov "$RL_GLOBAL_CORP" "$RL_MUT_LIST" "$RL_INPUT_DIR" "$TARGET" "$IMAGE")
fi

# For non-libfuzzer engines, align mutated_new with coverage-contributing count
if [ "$FUZZER" != "libfuzzer" ]; then
  BASE_MUT=$BASE_MUT_COV
  RL_MUT=$RL_MUT_COV
fi

echo "--- SUMMARY (target=$TARGET) ---"
# Default: print only mutated_new_cov and coverage for both sides
SHOW_VERBOSE="${SHOW_VERBOSE:-0}"
if [ "$SHOW_VERBOSE" = "1" ]; then
  echo "Baseline: inputs +$BASE_DELTA, new_units +$BASE_NEW, mutated_new +$BASE_MUT, mutated_new_cov +$BASE_MUT_COV, cov: $BASE_COV%"
  echo "RL:       inputs +$RL_DELTA, new_units +$RL_NEW, mutated_new +$RL_MUT, mutated_new_cov +$RL_MUT_COV, cov: $RL_COV%"
else
  echo "Baseline: mutated_new_cov +$BASE_MUT_COV, cov: $BASE_COV%"
  echo "RL:       mutated_new_cov +$RL_MUT_COV, cov: $RL_COV%"
fi

# Cleanup ephemeral dirs unless kept
if [ "${KEEP_TMP:-0}" != "1" ]; then
  rm -rf "$WORKSPACE_DIR/logs/${TAG}/merge_tmp" 2>/dev/null || true
  rm -rf "$WORKSPACE_DIR/logs/${TAG}/rl/rl_input/${TARGET}" 2>/dev/null || true
fi
# Optionally remove baseline and RL run stats to avoid clutter
if [ "${KEEP_BASE_RUN:-0}" != "1" ]; then
  rm -rf "$WORKSPACE_DIR/logs/${BASE_TAG}/rl/rl_runs/$BASE_RUN_ID" 2>/dev/null || true
fi
if [ "${KEEP_RL_RUN:-1}" != "1" ]; then
  rm -rf "$WORKSPACE_DIR/logs/${TAG}/rl/rl_runs/$RUN_ID" 2>/dev/null || true
fi

# Remove per-run seed copies unless kept; original corpora remain under workspace/corpora
if [ "${KEEP_SEEDS:-0}" != "1" ]; then
  rm -rf "$WORKSPACE_DIR/logs/${TAG}/base/seed/${TARGET}" 2>/dev/null || true
  rm -rf "$WORKSPACE_DIR/logs/${TAG}/rl/seed/${TARGET}" 2>/dev/null || true
fi

exit 0
