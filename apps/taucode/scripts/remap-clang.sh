#!/bin/bash
set -euo pipefail
REPO_ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
COMPILER=/usr/bin/clang
if [[ "$0" == *++* ]]; then COMPILER=/usr/bin/clang++; fi
ARGS=()
for arg in "$@"; do
  case "$arg" in
    # Vendored OpenSSL builds no shared modules. Its generated Makefile otherwise
    # embeds the build's install prefix as these unused runtime search directories.
    -DENGINESDIR=*) ARGS+=('-DENGINESDIR="/usr/lib/openssl/engines-3"') ;;
    -DMODULESDIR=*) ARGS+=('-DMODULESDIR="/usr/lib/openssl/modules"') ;;
    *) ARGS+=("$arg") ;;
  esac
done
exec "$COMPILER" "${ARGS[@]}" "-ffile-prefix-map=$REPO_ROOT=/taucode"
