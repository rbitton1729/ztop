#!/bin/sh
#
# Build script that runs ON bsd-1 (the FreeBSD CI host) inside a fresh SSH
# session. The GitLab CI job pipes this file to the remote shell:
#
#     ssh gitlab-ci@bsd-1 "VERSION='${VERSION}' sh -s" < scripts/bsd-build.sh
#
# Expects $VERSION to be set in the environment (e.g. "0.2.0", no "v" prefix).
# Builds zfstop in release mode and leaves the binary at:
#
#     ~gitlab-ci/zfstop/target/release/zfstop
#
# The CI job then SCPs that file back into its workspace and continues.
#
# Idempotent — safe to re-run with the same or a different version.

set -eu
: "${VERSION:?VERSION not set; pipe with ssh \"VERSION=x.y.z sh -s\" < scripts/bsd-build.sh}"

cd "$HOME/zfstop"

# Fetch tags so we can check out the release tag the CI is building.
git fetch --tags --force

# Check out the tag. Use a clean reset to wipe any prior CI run's
# version-rewrite of Cargo.toml.
git checkout --force "v${VERSION}"
git reset --hard "v${VERSION}"

# Stamp the version into Cargo.toml the same way the Linux jobs do.
# Note: BSD sed requires '' as the empty backup extension argument.
sed -i '' "s/^version = .*/version = \"${VERSION}\"/" Cargo.toml

# Build. Cargo lives at $HOME/.cargo/bin/cargo because rustup installs
# per-user (see scripts/setup-bsd-ci.sh).
"$HOME/.cargo/bin/cargo" build --release

# Strip is optional on FreeBSD but matches what the Linux jobs do.
# /usr/bin/strip is in the FreeBSD base system.
strip target/release/zfstop

ls -la target/release/zfstop
file target/release/zfstop
