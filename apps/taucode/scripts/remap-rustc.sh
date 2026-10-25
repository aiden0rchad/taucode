#!/bin/sh
# Keep checkout paths out of release panic locations and compiler metadata.
set -eu
REPO_ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
exec "$@" "--remap-path-prefix=$REPO_ROOT=/taucode"
