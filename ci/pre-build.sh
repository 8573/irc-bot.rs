#!/bin/sh

set -euv

# Print our Rust and Cargo versions in case that's useful for debugging.
echo "${TRAVIS_RUST_VERSION:-n/a}"
rustc --version
cargo --version

# Install Rust-related tools.
rustup component add rustfmt
cargo install --force cargo-audit || true
cargo install --force cargo-tree

# List our dependencies in case that's useful for debugging.
cargo tree
