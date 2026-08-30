#!/bin/bash

echo BROTLI
test -f dist/R2H-PDF.js.br || brotli dist/R2H-PDF.js
test -f dist/R2H-PDF-wasm.js.br || brotli dist/R2H-PDF-wasm.js
test -f dist/R2H-PDF-wasm.wasm.br || brotli dist/R2H-PDF-wasm.wasm

echo LICENSE
test -f ../../COPYING && cp -f ../../COPYING LICENSE
test -f ../../LICENSE && cp -f ../../LICENSE LICENSE
true
