#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
g++ -std=c++20 -O1 -ffp-contract=off -o "${TMPDIR:-/tmp}/bc7gen" gen.cpp
"${TMPDIR:-/tmp}/bc7gen"
