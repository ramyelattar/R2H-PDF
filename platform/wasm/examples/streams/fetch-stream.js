"use strict"

import * as R2H-PDF from "R2H-PDF"

class FetchStream {
	constructor(url) {
		fetch(url)
			.then((response) => {
				if (!response.ok)
					throw new Error("HTTP " + response.status)
				return response.arrayBuffer()
			})
			.then((buffer) => {
				this.buffer = new Uint8Array(buffer)
			})
	}
	fileSize() {
		if (this.buffer)
			return this.buffer.byteLength
		return -1 // signal try later
	}
	read(memory, offset, size, position) {
		if (this.buffer) {
			size = Math.min(size, this.buffer.byteLength - position)
			memory.set(this.buffer.subarray(position, position + size), offset)
			return size
		}
		return -1 // signal try later
	}
	close() {
		this.buffer = null
	}
}

var stm = new R2H-PDF.Stream(new FetchStream("https://R2H-PDF.com/docs/R2H-PDF_explored.pdf"))
function loop() {
	try {
		var doc = R2H-PDF.Document.openDocument(stm, "application/pdf")
		var n = doc.countPages()
		console.log("Document has " + n + " pages!")
	} catch (err) {
		if (err === "TRYLATER") {
			console.log("Waiting...")
			setTimeout(loop, 1000)
		} else {
			throw err
		}
	}
}
loop()
