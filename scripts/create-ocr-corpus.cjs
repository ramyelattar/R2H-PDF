const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const corpusDir = path.join(root, "demo", "input", "ocr-corpus");
fs.mkdirSync(corpusDir, { recursive: true });

function esc(text) {
  return String(text).replace(/\\/g, "\\\\").replace(/\(/g, "\\(").replace(/\)/g, "\\)");
}

function buildPdf({ lines, fontSize = 24, rotate = false }) {
  let ops = ["BT", `/F1 ${fontSize} Tf`, rotate ? "0 1 -1 0 120 120 Tm" : "72 720 Td"];
  for (let i = 0; i < lines.length; i += 1) {
    if (i > 0) ops.push(`0 -${Math.max(fontSize + 10, 24)} Td`);
    ops.push(`(${esc(lines[i])}) Tj`);
  }
  ops.push("ET");
  const content = ops.join("\n");
  const objects = [
    Buffer.from("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n", "binary"),
    Buffer.from("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n", "binary"),
    Buffer.from("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n", "binary"),
    Buffer.from("4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n", "binary"),
    Buffer.from(`5 0 obj\n<< /Length ${Buffer.byteLength(content, "binary")} >>\nstream\n${content}\nendstream\nendobj\n`, "binary"),
  ];
  const chunks = [Buffer.from("%PDF-1.4\n%\xE2\xE3\xCF\xD3\n", "binary")];
  const offsets = [0];
  let offset = chunks[0].length;
  for (const obj of objects) {
    offsets.push(offset);
    chunks.push(obj);
    offset += obj.length;
  }
  const xrefOffset = offset;
  let xref = `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (let i = 1; i <= objects.length; i += 1) {
    xref += `${String(offsets[i]).padStart(10, "0")} 00000 n \n`;
  }
  xref += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  chunks.push(Buffer.from(xref, "binary"));
  return Buffer.concat(chunks);
}

const fixtures = [
  ["01-clean-english.pdf", ["R2H OCR CORPUS CLEAN 101"], "R2H OCR CORPUS CLEAN 101", "supported"],
  ["02-low-resolution-english.pdf", ["R2H OCR LOWRES 202"], "R2H OCR LOWRES 202", "unsupported"],
  ["03-rotated-scan.pdf", ["R2H OCR ROTATED 303"], "R2H OCR ROTATED 303", "unsupported"],
  ["04-noisy-scan.pdf", ["R2H OCR NOISY 404"], "R2H OCR NOISY 404", "supported"],
  ["05-multi-page-scan.pdf", ["R2H OCR MULTIPAGE 505", "PAGE TWO TEXT"], "R2H OCR MULTIPAGE 505", "unsupported"],
  ["06-table-like-scan.pdf", ["PANEL MDB-01 400A", "FEEDER 4C X 240 MM2"], "PANEL MDB-01 400A", "unsupported"],
  ["07-mep-title-block.pdf", ["MEP TITLE BLOCK DB-L1", "DRAWING E-101"], "MEP TITLE BLOCK DB-L1", "unsupported"],
  ["08-arabic-bilingual.pdf", ["ARABIC SAMPLE 808"], "ARABIC SAMPLE 808", "unsupported"],
  ["09-blank-no-text.pdf", [""], "", "negative"],
  ["10-image-heavy-negative.pdf", [" "], "", "negative"],
];

for (const [name, lines, expected, category] of fixtures) {
  fs.writeFileSync(path.join(corpusDir, name), buildPdf({
    lines,
    fontSize: name.includes("low") ? 14 : 24,
    rotate: name.includes("rotated"),
  }));
}

fs.writeFileSync(path.join(corpusDir, "manifest.json"), JSON.stringify(fixtures.map(([file, , expected, category]) => ({ file, expected, category })), null, 2));
console.log(`OCR corpus written to ${corpusDir}`);
