#!/bin/sh

set -euv

. 'scripts/ci/examine-env.sh'

# Print our Rust and Cargo versions in case that's useful for debugging.
echo "${TRAVIS_RUST_VERSION:-n/a}"
rustc --version
cargo --version

# Install Rust-related tools.
rustup component add rustfmt
cargo install --force cargo-audit || true
if [ "${rust_version}" = stable ]; then cargo install --force cargo-tree; fi

# List our dependencies in case that's useful for debugging.
if [ "${rust_version}" = stable ]; then cargo tree; fi
