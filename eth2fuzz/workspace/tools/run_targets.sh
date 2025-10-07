#!/usr/bin/env sh
set -e

if [ "$#" -lt 4 ]; then
  echo "Usage: $0 <image> <seg_seconds> <nthreads> <workspace_dir>" >&2
  echo "Env: RUN_DOCKER_FLAGS, BEACONSTATE_ENV" >&2
  exit 1
fi

IMAGE="$1"
SEG="$2"
NTHREADS="$3"
WORKSPACE_DIR="$4"

: "${RUN_DOCKER_FLAGS:=}"
: "${BEACONSTATE_ENV:=-e ETH2FUZZ_BEACONSTATE=/eth2fuzz/workspace/corpora/beaconstate}"

targets=$(docker run -v "$WORKSPACE_DIR":/eth2fuzz/workspace "$IMAGE" list)

mkdir -p "$WORKSPACE_DIR/hfuzz/logs" >/dev/null 2>&1 || true

for i in $targets; do
  echo "[eth2fuzz] running $i ..."
  docker run ${RUN_DOCKER_FLAGS} -v "$WORKSPACE_DIR":/eth2fuzz/workspace ${BEACONSTATE_ENV} \
    -e TARGET="$i" -e SEG="$SEG" -e NTHREADS="$NTHREADS" \
    --entrypoint /bin/sh "$IMAGE" -c "sh /eth2fuzz/workspace/run_segment.sh"
done

exit 0
