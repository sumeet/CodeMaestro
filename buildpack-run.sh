#!/bin/bash
set -e

# grab exports from the rust buildpack. see
# https://github.com/emk/heroku-buildpack-rust/blob/master/bin/compile#L79-L95
BP_DIR=$(cd $(dirname ${0:-}); cd ..; pwd)
. $BP_DIR/export

./scripts/run-on-server-release
