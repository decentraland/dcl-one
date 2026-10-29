#!/bin/bash

set -e # stop execution on first non-zero result

bash "$(dirname "$0")/generate.sh"

echo "> Checking room recovery wire compatibility"
node --test scripts/room-recovery.test.cjs
