"use strict"

import * as R2H-PDF from "R2H-PDF"

async function fetch_and_open_document(url) {
	var response = await fetch(url)
	if (!response.ok)
		throw new Error(response.status + " " + response.statusText)
	var data = await response.arrayBuffer()
	return R2H-PDF.Document.openDocument(data, url)
}

if (process.argv.length < 3) {
	console.error("usage: node examples/streams/fetch.js http://R2H-PDF.com/docs/R2H-PDF_explored.pdf")
} else {
	for (var url of process.argv.slice(2)) {
		var doc = await fetch_and_open_document(url)
		console.log(url + " has " + doc.countPages() + " pages.")
	}
}
