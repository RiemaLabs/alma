#!/usr/bin/env bash
set -euo pipefail

# Rewrite dependency entries to use git = file:///eth2fuzz/lighthouse inside Docker
# so that builds are independent from host paths.

ROOT="/eth2fuzz"
GIT_URI="file://${ROOT}/lighthouse"

rewrite_file() {
  local file="$1"
  [ -f "$file" ] || return 0

  # state_processing
  sed -i -E \
    "s#^state_processing = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#state_processing = { git = \"${GIT_URI}\", package = \"state_processing\" }#" \
    "$file" || true

  # types
  sed -i -E \
    "s#^types = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#types = { git = \"${GIT_URI}\", package = \"types\" }#" \
    "$file" || true

  # bls
  sed -i -E \
    "s#^bls = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#bls = { git = \"${GIT_URI}\", package = \"bls\" }#" \
    "$file" || true

  # lighthouse_network
  sed -i -E \
    "s#^lighthouse_network =\s*\{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#lighthouse_network = { git = \"${GIT_URI}\", package = \"lighthouse_network\" }#" \
    "$file" || true

  # If any were already git deps pointing to another path, normalize to our GIT_URI
  sed -i -E \
    "s#(state_processing\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
  sed -i -E \
    "s#(types\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
  sed -i -E \
    "s#(bls\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
  sed -i -E \
    "s#(lighthouse_network\s*=\s*\{[^}]*git\s*=\s*\")file:[^\"]*(/lighthouse)(\")#\1${GIT_URI}\3#" "$file" || true
}

FILES=(
  "workspace/hfuzz/Cargo.toml"
  "workspace/targets/rust/Cargo.toml"
  "workspace/libfuzzer/fuzz/Cargo.toml"
  "fuzzers/rust-honggfuzz/Cargo.toml"
)

for f in "${FILES[@]}"; do
  echo "[docker] Rewriting deps in $f"
  rewrite_file "$f"
done

echo "[docker] Dependency rewrite complete."

