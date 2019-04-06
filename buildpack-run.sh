#!/bin/bash
set -e

# grab exports from the rust buildpack. see
# https://github.com/emk/heroku-buildpack-rust/blob/master/bin/compile#L79-L95

# Our rustup installation.
export RUSTUP_HOME="$CACHE_DIR/multirust"
# Our cargo installation.  We implicitly trust Rustup and Cargo
# to do the right thing when new versions are released.
export CARGO_HOME="$CACHE_DIR/cargo"
# Include binaries installed by cargo and rustup in our path.
export PATH="$CARGO_HOME/bin:$PATH"

./scripts/run-on-server-release
