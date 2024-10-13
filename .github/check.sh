#!/usr/bin/env bash
set -o pipefail -e

./branch-asm "$@"
diff -u <( ./master-asm "$@" 2> /dev/null ) --label master <( ./branch-asm "$@" 2> /dev/null ) --label branch
