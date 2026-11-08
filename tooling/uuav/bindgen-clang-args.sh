#!/usr/bin/env bash
# Prints the BINDGEN_EXTRA_CLANG_ARGS that point bindgen at clang's builtin
# headers: libclang loaded by bindgen does not always find its resource
# directory on Windows (mm_malloc.h not found from mingw's malloc.h).

set -euo pipefail

for clang in clang "/c/Program Files/LLVM/bin/clang.exe"; do
    if dir="$("$clang" -print-resource-dir 2>/dev/null)"; then
        printf -- '-isystem "%s/include"\n' "${dir%$'\r'}"
        exit 0
    fi
done
