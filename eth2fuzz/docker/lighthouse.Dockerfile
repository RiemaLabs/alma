FROM ubuntu:18.04 AS build

ARG RUST_TOOLCHAIN="nightly"
ARG GIT_BRANCH="stable"

ENV CARGO_HOME=/usr/local/rust
ENV RUSTUP_HOME=/usr/local/rust
ENV PATH="$PATH:$CARGO_HOME/bin"

# Update ubuntu
# Install dependencies
RUN apt-get update && \
	apt-get install -y --no-install-recommends \
		build-essential \
		pkg-config \
		libssl-dev \
		ca-certificates \
		curl \
		git \
		build-essential \
		libtool-bin \
		python3-dev \
		automake \
		flex \
		bison \
		libglib2.0-dev \
		libpixman-1-dev \
		clang \
		python3-setuptools \
		llvm \
		binutils-dev \
		libunwind-dev \
		libblocksruntime-dev \
		cmake

# Install Rust and Cargo
RUN curl --proto '=https' \
	--tlsv1.2 \
	-sSf https://sh.rustup.rs | sh -s -- -y \
	--default-toolchain "$RUST_TOOLCHAIN"

# Note: Building the full Lighthouse workspace inside the image consumes large memory
# due to lto fat and codegen-units=1 in its maxperf profile.
# We don't need Lighthouse binaries for fuzzing; Cargo will fetch Lighthouse crates on-demand
# when compiling fuzz targets. Skip building Lighthouse here to keep memory usage low.

#####################################
############ FUZZERS ################

# Install Rust fuzzer
RUN cargo install honggfuzz
RUN cargo install cargo-fuzz
# Install Rust AFL CLI (cargo-afl)
RUN cargo install cargo-afl

#####################################
############ eth2fuzz ################

WORKDIR /eth2fuzz

# Copy eth2fuzz code (including workspace and configs)
COPY . .

# Ensure the vendored lighthouse is a standalone git repo so Cargo can use it as a local git source.
RUN rm -rf lighthouse/.git \
 && cd lighthouse \
 && git init -q \
 && git config user.email "local@eth2fuzz" \
 && git config user.name "eth2fuzz" \
 && git add . \
 && GIT_AUTHOR_DATE="2000-01-01T00:00:00Z" GIT_COMMITTER_DATE="2000-01-01T00:00:00Z" git commit -q -m "vendor lighthouse"

# Build the CLI tool
RUN make -f eth2fuzz.mk build

# Set env for eth2fuzz target listing
ENV CURRENT_CLIENT="LIGHTHOUSE"

ENTRYPOINT ["/eth2fuzz/eth2fuzz"]
