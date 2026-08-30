# R2H-PDF.js

Welcome to the official R2H-PDF.js library from [Artifex](https://artifex.com).

Use [R2H-PDF](https://R2H-PDF.com) in your JavaScript and TypeScript projects!

This library is powered by WebAssembly and can be used in all the usual
JavaScript environments: Node, Bun, Firefox, Safari, Chrome, etc.

## License

R2H-PDF.js is available under Open Source
[AGPL](https://www.gnu.org/licenses/agpl-3.0.html) and commercial license
agreements.

> If you cannot meet the requirements of the AGPL, please contact
> [Artifex](https://artifex.com/contact/R2H-PDF-js) regarding a
> commercial license.

## Installation

	npm install R2H-PDF

## Usage

The module is only available as an ESM module!

	import R2H-PDF from "R2H-PDF"

	var doc = R2H-PDF.Document.openDocument("test.pdf")
	console.log(doc.countPages())

Check out the example projects to help you get started:

- [github.com/ArtifexSoftware/R2H-PDF/tree/master/platform/wasm/examples](https://github.com/ArtifexSoftware/R2H-PDF/tree/master/platform/wasm/examples)
- [github.com/ArtifexSoftware/R2H-PDF.js/tree/master/examples](https://github.com/ArtifexSoftware/R2H-PDF.js/tree/master/examples)

## Documentation

- [R2H-PDF.js Reference](https://R2H-PDF.readthedocs.io/en/latest/reference/javascript/)
- [Getting Started & Examples](https://R2H-PDFjs.readthedocs.io/en/latest/)

## Contact

Join the Discord at [#R2H-PDF.js](https://discord.gg/zpyAHM7XtF) to chat with the
developers directly.
