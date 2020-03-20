#!/bin/sh

set -euv

# Lint our shell scripts.
find . \( -iname '*.sh' -o -iname '*.bash' -o -iname '*.zsh' \) -print0 |
    xargs -0t shellcheck --enable=all --check-sourced --external-sources

# Print our Rust and Cargo versions in case that's useful for debugging.
echo "${TRAVIS_RUST_VERSION:-n/a}"
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
