#!/bin/bash
set -euo pipefail

# Keep a single compatibility definition. The legacy proto-compatibility-tool
# aborts on valid reserved field names, while Buf validates both reserved names
# and wire-level schema changes against this repository's main branch
# (the Makefile exports that branch's proto tree with git archive).
make buf-breaking
