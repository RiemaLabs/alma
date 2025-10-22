#!/usr/bin/env bash
set -euo pipefail

# Rewrite path deps to git=file://<ABS>/lighthouse for local builds,
# to satisfy Lighthouse's workspace dependency inheritance.

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
GIT_URI="file://${ROOT}/lighthouse"

rewrite_file() {
  local file="$1"
  [ -f "$file" ] || return 0

  sed -i '' -E \
    "s#^state_processing = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#state_processing = { git = \"${GIT_URI}\", package = \"state_processing\" }#" \
    "$file" || true

  sed -i '' -E \
    "s#^types = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#types = { git = \"${GIT_URI}\", package = \"types\" }#" \
    "$file" || true

  sed -i '' -E \
    "s#^bls = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#bls = { git = \"${GIT_URI}\", package = \"bls\" }#" \
    "$file" || true

  sed -i '' -E \
    "s#^lighthouse_network =\s*\{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#lighthouse_network = { git = \"${GIT_URI}\", package = \"lighthouse_network\" }#" \
    "$file" || true

  # Normalize any existing file://.../lighthouse URIs to ${GIT_URI}
  sed -i '' -E \
    "s#(state_processing\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
  sed -i '' -E \
    "s#(types\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
  sed -i '' -E \
    "s#(bls\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
  sed -i '' -E \
    "s#(lighthouse_network\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
}

FILES=(
  "workspace/hfuzz/Cargo.toml"
  "workspace/targets/rust/Cargo.toml"
  "workspace/libfuzzer/fuzz/Cargo.toml"
  "fuzzers/rust-honggfuzz/Cargo.toml"
)

for f in "${FILES[@]}"; do
  echo "[local] Rewriting deps in $f -> ${GIT_URI}"
  rewrite_file "$f"
done

echo "[local] Dependency rewrite complete."

