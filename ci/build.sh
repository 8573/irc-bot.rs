#!/bin/sh

set -euv

# Print our rustc and Cargo versions in case that's useful for debugging.
rustc --version
cargo --version

# Check that all our source-code formatting is standard.
cargo fmt --all -- --check

# List our dependencies in case that's useful for debugging.
cargo tree

# Build and test our crate(s).
cargo test --all --verbose

# Audit our dependency tree for security vulnerabilities.
test -r 'Cargo.lock'
cargo audit
