#!/bin/sh

set -eu +v

# Decide what we have available.

ci_platform='???'
rust_version='???'

if [ -n "${GITLAB_CI:-}" ]; then
    ci_platform='GitLab'
    rust_version="${CI_JOB_IMAGE:-???}"
    rust_version="${rust_version#rust:}"
elif [ -n "${TRAVIS_RUST_VERSION:-}" ]; then
    ci_platform='Travis'
    rust_version="${TRAVIS_RUST_VERSION}"
fi

set -v

ci_env="CI:${ci_platform}/Rust:${rust_version}"

echo "${ci_env}"
