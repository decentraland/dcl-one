#!/usr/bin/env bash
# Prints the BINDGEN_EXTRA_CLANG_ARGS that point bindgen at clang's builtin
# headers: libclang loaded by bindgen does not always find its resource
# directory on Windows (mm_malloc.h not found from mingw's malloc.h).
# The AMX headers are skipped by predefining their guards: winnt.h includes
# immintrin.h, and bindgen cannot express the AMX tile struct's layout
# (conflicting packed and align hints). Same list as infra/packages/uuav.nix.

set -euo pipefail

amx_header_guards='__AMXINTRIN_H __AMX_FP16INTRIN_H __AMX_COMPLEXINTRIN_H __AMXFP8INTRIN_H
__AMX_TRANSPOSEINTRIN_H __AMXMOVRSINTRIN_H __AMX_MOVRS_TRANSPOSEINTRIN_H __AMX_AVX512INTRIN_H
__AMX_TF32INTRIN_H __AMX_TF32TRANSPOSEINTRIN_H __AMX_BF16TRANSPOSEINTRIN_H
__AMX_FP16TRANSPOSEINTRIN_H __AMX_COMPLEXTRANSPOSEINTRIN_H'

for clang in clang "/c/Program Files/LLVM/bin/clang.exe"; do
    if dir="$("$clang" -print-resource-dir 2>/dev/null)"; then
        printf -- '-isystem "%s/include"' "${dir%$'\r'}"
        for guard in $amx_header_guards; do
            printf -- ' -D%s' "$guard"
        done
        printf '\n'
        exit 0
    fi
done
