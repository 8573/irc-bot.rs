#!/bin/sh

set -euv

. 'scripts/ci/examine-env.sh'

# Lint our shell scripts. Don't bother on GitLab. GitLab CI is mainly for
# building documentation.
shell_lint() {
    find . \( -iname '*.sh' -o -iname '*.bash' -o -iname '*.zsh' \) -print0 |
        xargs -0t "${shellcheck_cmd}" \
            --enable=all --check-sourced --external-sources
}
case "${ci_platform}" in
    (GitLab) ;;
    (*) shell_lint;;
esac

# Check that all our source-code formatting is standard.
cargo fmt --all -- --check

# Build and test our crate(s).
cargo test --all --verbose

# Audit our dependency tree for security vulnerabilities. This is allowed to
# fail in some CI environments in which we expect to be using our minimum
# supported Rust version (MSRV), because our MSRV may be older than
# `cargo-audit`'s MSRV.
test -r 'Cargo.lock'
case "${ci_env}" in
    (ci:Travis/rust:stable) cargo audit;;
    (ci:Travis/rust:1.* | ci:GitLab/*) cargo audit || true;;
    (*) cargo audit;;
esac
