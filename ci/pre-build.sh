#!/bin/sh

set -euv

rustup component add rustfmt
cargo install --force cargo-audit
cargo install --force cargo-tree
