#!/usr/bin/env node
// Issue a signed R2H-PDF license (schema 2, Ed25519).
//
// The payload bytes signed here must match the Rust canonical serialization
// in src-tauri/src/license.rs exactly: compact JSON with the field order
// license_id, subject, issued_unix, expires_unix, features. Keep ASCII in
// string fields to avoid any encoding divergence.
//
// Usage:
//   node scripts/license/issue-license.mjs --subject "ACME Corp" --days 365 \
//        --features export,ocr,rag,compare [--out license.json] [--id <uuid>]

import { createPrivateKey, sign, randomUUID } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const defaultKeyPath = join(repoRoot, "security", "license-issuer", "private", "issuer-private-key.pem");

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    if (!flag.startsWith("--")) throw new Error(`unexpected argument: ${flag}`);
    out[flag.slice(2)] = argv[i + 1];
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));
if (!args.subject || !args.days || !args.features) {
  console.error("Required: --subject <string> --days <n> --features <comma-separated>");
  console.error('Example: node scripts/license/issue-license.mjs --subject "ACME Corp" --days 365 --features export,ocr,rag,compare');
  process.exit(1);
}

const keyPath = args.key ? resolve(args.key) : defaultKeyPath;
if (!existsSync(keyPath)) {
  console.error(`Issuer private key not found at ${keyPath}`);
  console.error("Generate it with: node scripts/license/generate-license-keys.mjs");
  process.exit(1);
}

const privateKey = createPrivateKey(readFileSync(keyPath));

const issuedUnix = Math.floor(Date.now() / 1000);
const expiresUnix = issuedUnix + Number(args.days) * 24 * 60 * 60;
// Field order matters: it must mirror the serde struct declaration in license.rs.
const payload = {
  license_id: args.id ?? randomUUID(),
  subject: args.subject,
  issued_unix: issuedUnix,
  expires_unix: expiresUnix,
  features: String(args.features).split(",").map((f) => f.trim()).filter(Boolean),
};

const payloadJson = JSON.stringify(payload);
const signature = sign(null, Buffer.from(payloadJson, "utf8"), privateKey).toString("base64");

const license = { schema: 2, payload: JSON.parse(payloadJson), signature };
const outPath = args.out ? resolve(args.out) : join(process.cwd(), "license.json");
writeFileSync(outPath, JSON.stringify(license, null, 2) + "\n");

console.log(`License written: ${outPath}`);
console.log(`  license_id:    ${payload.license_id}`);
console.log(`  subject:       ${payload.subject}`);
console.log(`  features:      ${payload.features.join(", ")}`);
console.log(`  expires_unix:  ${payload.expires_unix} (${new Date(payload.expires_unix * 1000).toISOString()})`);
