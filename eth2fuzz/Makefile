# Main Makefile designed for multi-platform support for sh and Windows pwsh
# Helps provide docker commands

ifeq ($(OS),Windows_NT)
	# NOTE: for this to work, docker-related commands need to have a pwsh equivalent alias
	SHELL := pwsh.exe
	.SHELLFLAGS := -Command
	BUILD_ENV :=
else
	SHELL := /bin/sh
	# Enable BuildKit only if buildx is available; otherwise leave empty to avoid errors.
	BUILD_ENV ?= $(shell if command -v docker >/dev/null 2>&1; then docker buildx version >/dev/null 2>&1 && printf "DOCKER_BUILDKIT=1"; fi)
endif

.PHONY: check-fmt help docker corpora fmt clippy clean clean-all submodules prepare-cache prefetch-lighthouse crashes-lighthouse

.DEFAULT_GOAL=docker-all

# NOTE: this works for windows too!
WORKSPACE_DIR := $(realpath workspace)
# Colors for pretty output
RED := \033[31m
NC  := \033[0m
# Optional run-time flags for docker run (volumes/env/limits). Example:
#   export DOCKER_RUN_FLAGS='-v $(HOME)/.cargo-docker/registry:/usr/local/rust/registry \
#     -v $(HOME)/.cargo-docker/git:/usr/local/rust/git \
#     -v $(WORKSPACE_DIR)/hfuzz/target:/eth2fuzz/workspace/hfuzz/target \
#     -e CARGO_TARGET_DIR=/eth2fuzz/workspace/hfuzz/target'
DOCKER_RUN_FLAGS ?=

# Detect host CPU count for default fuzz threads
CPUS := $(shell nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1)
# Default beaconstate path inside container
BEACONSTATE_ENV := -e ETH2FUZZ_BEACONSTATE=/eth2fuzz/workspace/corpora/beaconstate

clients := lighthouse nimbus prysm teku lodestar
test_clients := $(addprefix test-,$(clients))
clean_clients := $(addprefix clean-,$(clients))


help:
	@echo 'Management commands for eth2fuzz'
	@echo ''
	@echo 'Available clients:'
	@echo '    lighthouse'
	@echo '    lodestar'
	@echo '    nimbus'
	@echo '    prysm'
	@echo '    teku'
	@echo ''
	@echo 'Basic Usage:'
	@echo ''
	@echo '    make fuzz-all                                    Build and fuzz all targets for all clients, 1 hr each.'
	@echo '    make fuzz-[CLIENT]                               Fuzz all targets for CLIENT, 1 hr each.'
	@echo '                                                     e.g. `make fuzz-lighthouse`'
	@echo '    make fuzz-[CLIENT]-rl                            RL-enhanced fuzzing for CLIENT (bandit scheduler).'
	@echo '                                                     e.g. `make fuzz-lighthouse-rl`'
	@echo '    make or make docker-all                          Build all docker images.'
	@echo '    make CLIENT                                      Build all docker image for CLIENT.'
	@echo '                                                     e.g. `make prysm`'
	@echo ''
	@echo '    make clean                                       Clean all (eth2fuzz && compiled fuzz target harnesses).'
	@echo '    make clean-[CLIENT]                              Clean build artifacts for a single client CLIENT.'
	@echo ''
	@echo 'Dev helpers:'
	@echo '    make test-all                                    Fuzz all targets for all clients, 1 s each.'
	@echo '    make test-[CLIENT]                               Fuzz all targets for CLIENT, 1 s each.'
	@echo ''
	@echo '    make corpora                                     Create initial corpora from Eth2 Spec tests.'
	@echo ''
	@echo '    make fmt                                         Run Rust fmt.'
	@echo ''
	@echo 'Quick coverage compare (single target):'
	@echo '    make cov-attestation                              Baseline vs PPO on lighthouse_attestation.'
	@echo 'Crash helpers:'
	@echo '    make crashes-lighthouse                           List honggfuzz crash files for lighthouse.'
	@echo ''
	@echo ''


# Create an initial corpora
corpora:
	sh workspace/corpora/extract_corpora_from_spectests.sh


#
# ALL CLIENTS
#

## Build all docker
docker-all: lighthouse lodestar nimbus prysm teku

## Fuzz all eth2 clients
fuzz-all: fuzz-lighthouse fuzz-lodestar fuzz-nimbus fuzz-prysm fuzz-teku

# Clean all (eth2fuzz && compiled fuzz target harnesses).
clean: $(clean_clients)
	rm -rf workspace/debug/target
	# clean js - not needed
	# clean java libfuzzer - TODO

#
# Lighthouse
# https://github.com/sigp/lighthouse
#

## Fuzzing of lighthouse using docker.
#
# Note: we define the command outside of the recipe to avoid passing the \ to the shell (not handled nicely in pwsh)
# See https://www.gnu.org/software/make/manual/html_node/Splitting-Recipe-Lines.html#Splitting-Recipe-Lines

# Allow passing extra docker build flags like memory/shm-size via DOCKER_FLAGS, e.g.:
#   make lighthouse DOCKER_FLAGS="--memory 8g --memory-swap -1 --shm-size 4g"

lighthouse: build_cmd = $(BUILD_ENV) docker build $(DOCKER_FLAGS) \
	--file docker/lighthouse.Dockerfile \
	-t eth2fuzz_lighthouse \
	. \
	$(CACHE)
lighthouse: submodules
	$(build_cmd)
	@echo 'USAGE: docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_lighthouse list'

## Fuzz all lighthouse targets for 1 hour each
fuzz-lighthouse: prefetch-lighthouse
	@mkdir -p $(WORKSPACE_DIR)/hfuzz/target >/dev/null 2>&1 || true
	@mkdir -p $(WORKSPACE_DIR)/hfuzz/logs >/dev/null 2>&1 || true
	RUN_DOCKER_FLAGS='$(DOCKER_RUN_FLAGS)' BEACONSTATE_ENV='$(BEACONSTATE_ENV)' ETH2FUZZ_TAG='$(or $(tag),default)' \
	  /bin/sh workspace/tools/run_targets.sh eth2fuzz_lighthouse $(or $(t),3600) $(or $(n),$(CPUS)) "$(WORKSPACE_DIR)"

## RL-enhanced fuzzing of lighthouse using docker (bandit over targets)
fuzz-lighthouse-rl: prefetch-lighthouse
	RUN_ID_FLAG="$(if $(run_id),--run-id $(run_id),)"; \
	docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace \
	  $(DOCKER_RUN_FLAGS) eth2fuzz_lighthouse rl-fuzz -q lighthouse --total $(or $(T),3600) --segment $(or $(S),60) -n $(or $(n),$(CPUS)) --config configs/rl_enabled.json --tag $(or $(tag),default) $$RUN_ID_FLAG

## Prefetch dependencies into mounted caches to avoid network during fuzz runs
prefetch-lighthouse: lighthouse prepare-cache
	# purge stale git cache entries (commit ids change when vendoring lighthouse inside image)
	docker run --rm $(DOCKER_RUN_FLAGS) -v $(WORKSPACE_DIR):/eth2fuzz/workspace \
	  --entrypoint bash eth2fuzz_lighthouse -lc 'set -e; \
	  rm -rf /usr/local/rust/git/db/*; \
	  rm -f /eth2fuzz/workspace/hfuzz/Cargo.lock /eth2fuzz/workspace/targets/rust/Cargo.lock; \
	  cd /eth2fuzz/workspace/targets/rust && cargo update && cargo fetch; \
	  cd /eth2fuzz/workspace/hfuzz && cargo update && cargo fetch'

## clean compiled fuzzing targets
clean-lighthouse:
	# clean lighthouse afl
	rm -rf workspace/afl/target
	# clean lighthouse hfuzz
	rm -rf workspace/hfuzz/hfuzz_target
	# clean lighthouse libfuzzer
	rm -rf workspace/libfuzzer/fuzz/target

## Find lighthouse honggfuzz crashes saved under workspace/hfuzz/hfuzz_workspace
crashes-lighthouse:
	@printf "%b[lighthouse/hfuzz] crashes%b\n" "$(RED)" "$(NC)"
	@find $(WORKSPACE_DIR)/hfuzz/hfuzz_workspace -type f -name '*.fuzz' -print 2>/dev/null | sed -e 's#^#  #'

#
# Lodestar
# https://github.com/ChainSafe/lodestar
#

## Fuzzing of lodestar using docker.
lodestar: build_cmd = $(BUILD_ENV) docker build $(DOCKER_FLAGS) \
	--file docker/lodestar.Dockerfile \
	-t eth2fuzz_lodestar \
	. \
	$(CACHE)
lodestar:
	$(build_cmd)
	@echo 'USAGE: docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_lodestar list'

## Fuzz all lodestar targets for 1 hour each
fuzz-lodestar: lodestar
	docker run -e ETH2FUZZ_TAG=$(or $(tag),default) -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_lodestar continuously -q lodestar -t 3600

## RL-enhanced fuzzing of lodestar
fuzz-lodestar-rl: lodestar
	RUN_ID_FLAG="$(if $(run_id),--run-id $(run_id),)"; \
	docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace \
	  $(DOCKER_RUN_FLAGS) eth2fuzz_lodestar rl-fuzz -q lodestar --total $(or $(T),3600) --segment $(or $(S),60) -n $(or $(n),1) --config configs/rl_enabled.json --tag $(or $(tag),default) $$RUN_ID_FLAG

## Nothing to clean with lodestar since there
## is not compiled fuzzing targets
clean-lodestar:

#
# Nimbus
# https://github.com/status-im/nimbus-eth2
#

## Fuzzing of nimbus-eth2 using docker.
nimbus: build_cmd = $(BUILD_ENV) docker build $(DOCKER_FLAGS) \
	--file docker/nimbus.Dockerfile \
	-t eth2fuzz_nimbus \
	. \
	$(CACHE)
nimbus:
	$(build_cmd)
	@echo 'USAGE: docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_nimbus list'

## Fuzz all nimbus targets for 1 hour each
fuzz-nimbus: nimbus
	docker run -e ETH2FUZZ_TAG=$(or $(tag),default) -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_nimbus continuously -q nimbus -t 3600

## RL-enhanced fuzzing of nimbus
fuzz-nimbus-rl: nimbus
	RUN_ID_FLAG="$(if $(run_id),--run-id $(run_id),)"; \
	docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace \
	  $(DOCKER_RUN_FLAGS) eth2fuzz_nimbus rl-fuzz -q nimbus --total $(or $(T),3600) --segment $(or $(S),60) -n $(or $(n),1) --config configs/rl_enabled.json --tag $(or $(tag),default) $$RUN_ID_FLAG

## clean compiled fuzzing targets
clean-nimbus:
	for i in $(shell docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_nimbus list); do \
		rm -rf workspace/nimlibfuzzer/$$i; \
	done

#
# Prysm
# https://github.com/prysmaticlabs/prysm
#

## Fuzzing of prysm using docker.
prysm: build_cmd = $(BUILD_ENV) docker build $(DOCKER_FLAGS) \
	--file docker/prysm.Dockerfile \
	-t eth2fuzz_prysm \
	. \
	$(CACHE)
prysm:
	$(build_cmd)
	@echo 'USAGE: docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_prysm list'

## Fuzz all prysm targets for 1 hour each
fuzz-prysm: prysm
	docker run -e ETH2FUZZ_TAG=$(or $(tag),default) -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_prysm continuously -q prysm -t 3600

## RL-enhanced fuzzing of prysm
fuzz-prysm-rl: prysm
	RUN_ID_FLAG="$(if $(run_id),--run-id $(run_id),)"; \
	docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace \
	  $(DOCKER_RUN_FLAGS) eth2fuzz_prysm rl-fuzz -q prysm --total $(or $(T),3600) --segment $(or $(S),60) -n $(or $(n),1) --config configs/rl_enabled.json --tag $(or $(tag),default) $$RUN_ID_FLAG

## clean compiled fuzzing targets
clean-prysm:
	# clean gofuzz libfuzzer
	rm -rf workspace/gofuzz/*.libfuzzer

#
# Teku
# https://github.com/PegaSysEng/teku
#

## Fuzzing of teku using docker.
teku: build_cmd = $(BUILD_ENV) docker build $(DOCKER_FLAGS) \
	--file docker/teku.Dockerfile \
	-t eth2fuzz_teku \
	. \
	$(CACHE)
teku:
	$(build_cmd)
	@echo 'USAGE: docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_teku list'

## Fuzz all teku targets for 1 hour each
fuzz-teku: teku
	docker run -e ETH2FUZZ_TAG=$(or $(tag),default) -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_teku continuously -q teku -t 3600

## RL-enhanced fuzzing of teku
fuzz-teku-rl: teku
	RUN_ID_FLAG="$(if $(run_id),--run-id $(run_id),)"; \
	docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace \
	  $(DOCKER_RUN_FLAGS) eth2fuzz_teku rl-fuzz -q teku --total $(or $(T),3600) --segment $(or $(S),60) -n $(or $(n),1) --config configs/rl_enabled.json --tag $(or $(tag),default) $$RUN_ID_FLAG

## clean compiled fuzzing targets
clean-teku:
	# clean javafuzz
	rm -rf workspace/javafuzz/T*.class

test-all: $(test_clients)

test-lighthouse: exit_success=0
test-prysm: exit_success=0
test-nimbus: exit_success=0
test-teku: exit_success=1
test-lodestar: exit_success=1

# TODO depend on the build client? Wanna avoid rebuilding it unnecessarily
ifeq ($(OS),Windows_NT)
## Try to run each target for 1 sec
# Note: we keep this outside of the recipe, so we can split up lines with \ without sending
# the backslash to pwsh
# TODO break if test fails?
$(test_clients): test_cmd = "$(shell docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_$* list)".Split() | \
	ForEach-Object -Process { docker run -v $(WORKSPACE_DIR):/eth2fuzz/workspace eth2fuzz_$* target $$_ -t 2; \
	if ($$LastexitCode -neq $(exit_success)) { exit 1 } }
$(test_clients): test-%:
	@echo "Windows tests"
	$(test_cmd)
else
## Try to run each target for a minimal time
# (prob wanna run it until at least INITed?
# TODO break if test fails?

$(test_clients): test-%:
	@echo "Expecting exit status of $(exit_success)"
	@mkdir -p $(WORKSPACE_DIR)/hfuzz/target >/dev/null 2>&1 || true
	@mkdir -p $(WORKSPACE_DIR)/hfuzz/logs >/dev/null 2>&1 || true
	RUN_DOCKER_FLAGS='$(DOCKER_RUN_FLAGS)' BEACONSTATE_ENV='$(BEACONSTATE_ENV)' \
	  /bin/sh workspace/tools/run_targets.sh eth2fuzz_$* $(or $(t),2) 1 "$(WORKSPACE_DIR)"
endif

#
# Developer utils
#

# Run Rust fmt to make code cleaner
fmt:
	cargo fmt --all

# Run Rust clippy to make code cleaner
clippy:
	cargo clippy --all
submodules:
	@git submodule update --init --recursive

# Ensure host cache directories exist (used by DOCKER_RUN_FLAGS defaults)
prepare-cache:
	@mkdir -p $(HOME)/.cargo-docker/home >/dev/null 2>&1 || true
	@mkdir -p $(WORKSPACE_DIR)/hfuzz/target >/dev/null 2>&1 || true

## Quick coverage compare on single target (lighthouse_attestation)
cov-attestation: lighthouse
	/bin/sh workspace/tools/quick_cov_compare.sh eth2fuzz_lighthouse lighthouse_attestation $(or $(T),60) $(or $(RT),60) $(or $(S),10) $(or $(n),2) $(or $(tag),covtest)
