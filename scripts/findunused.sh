#!/bin/bash
make build=debug -j4
rm -f build/debug/mutool build/debug/R2H-PDF-gl
make build=debug XLIBS=-Wl,--print-gc-sections build/debug/mutool 2>&1 | grep 'libR2H-PDF\.' | sort > build/debug/mutool.gc
make build=debug XLIBS=-Wl,--print-gc-sections build/debug/R2H-PDF-gl 2>&1 | grep 'libR2H-PDF\.' | sort >build/debug/R2H-PDF-gl.gc
comm -12 build/debug/mutool.gc build/debug/R2H-PDF-gl.gc
