const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const inputDir = path.join(root, "demo", "input");
const outputDir = path.join(root, "demo", "output");
const smokeDir = path.join(root, "release", "smoke-results");
const logsDir = path.join(root, "release", "logs");

for (const dir of [inputDir, outputDir, smokeDir, logsDir]) {
  fs.mkdirSync(dir, { recursive: true });
}

function esc(text) {
  return String(text)
    .replace(/\\/g, "\\\\")
    .replace(/\(/g, "\\(")
    .replace(/\)/g, "\\)");
}

function textOps(lines) {
  const ops = ["BT", "/F1 24 Tf", "72 720 Td"];
  lines.forEach((line, index) => {
    if (index > 0) ops.push("0 -34 Td");
    ops.push(`(${esc(line)}) Tj`);
  });
  ops.push("ET");
  return ops.join("\n");
}

function buildPdf({ lines, includeImage = false, imageLabel = false }) {
  const content = [
    textOps(lines),
    includeImage ? "q\n80 0 0 40 72 610 cm\n/Im1 Do\nQ" : "",
    imageLabel ? "BT\n/F1 14 Tf\n72 590 Td\n(Synthetic embedded image XObject) Tj\nET" : "",
  ].filter(Boolean).join("\n");

  const imageBytes = Buffer.from([
    0x12, 0x72, 0xc9, 0xe8, 0x44, 0x2e,
    0xff, 0xf1, 0x52, 0x1c, 0x1c, 0x1c,
  ]);

  const objects = [];
  objects.push(Buffer.from("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n", "binary"));
  objects.push(Buffer.from("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n", "binary"));
  const resources = includeImage
    ? "<< /Font << /F1 4 0 R >> /XObject << /Im1 6 0 R >> >>"
    : "<< /Font << /F1 4 0 R >> >>";
  objects.push(Buffer.from(`3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources ${resources} /Contents 5 0 R >>\nendobj\n`, "binary"));
  objects.push(Buffer.from("4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n", "binary"));
  const contentBytes = Buffer.from(content, "binary");
  objects.push(Buffer.from(`5 0 obj\n<< /Length ${contentBytes.length} >>\nstream\n${content}\nendstream\nendobj\n`, "binary"));
  if (includeImage) {
    objects.push(Buffer.concat([
      Buffer.from(`6 0 obj\n<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length ${imageBytes.length} >>\nstream\n`, "binary"),
      imageBytes,
      Buffer.from("\nendstream\nendobj\n", "binary"),
    ]));
  }

  const chunks = [Buffer.from("%PDF-1.4\n%\xE2\xE3\xCF\xD3\n", "binary")];
  const offsets = [0];
  let offset = chunks[0].length;
  for (const obj of objects) {
    offsets.push(offset);
    chunks.push(obj);
    offset += obj.length;
  }

  const xrefOffset = offset;
  const count = objects.length + 1;
  let xref = `xref\n0 ${count}\n0000000000 65535 f \n`;
  for (let i = 1; i < count; i++) {
    xref += `${String(offsets[i]).padStart(10, "0")} 00000 n \n`;
  }
  xref += `trailer\n<< /Size ${count} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  chunks.push(Buffer.from(xref, "binary"));
  return Buffer.concat(chunks);
}

const files = [
  ["render-sample.pdf", ["R2H PDF Render Smoke Test"]],
  ["text-edit-sample.pdf", ["OLD TEXT VALUE"]],
  ["image-edit-sample.pdf", ["IMAGE EDIT SAMPLE"], true, true],
  ["scanned-ocr-sample.pdf", ["R2H OCR TEST 123"], true, true],
  ["ask-pdf-sample.pdf", ["Main distribution board MDB-01 is rated 400A.", "Feeder cable is 4C x 240 mm2 Cu XLPE."]],
  ["compare-a.pdf", ["Revision A: Cable size is 4C x 185 mm2."]],
  ["compare-b.pdf", ["Revision B: Cable size is 4C x 240 mm2."]],
];

for (const [name, lines, includeImage, imageLabel] of files) {
  fs.writeFileSync(
    path.join(inputDir, name),
    buildPdf({ lines, includeImage: Boolean(includeImage), imageLabel: Boolean(imageLabel) }),
  );
}

console.log(`Demo PDFs written to ${inputDir}`);
