#!/usr/bin/env node
// Generate the R2H-PDF license issuer Ed25519 keypair.
//
// Trust model: the application embeds ONLY the 32-byte public verification
// key (src-tauri/src/license.rs). The private signing key stays with the
// license issuer and is NEVER committed. This script writes:
//   security/license-issuer/private/issuer-private-key.pem  (gitignored)
//   security/license-issuer/issuer-public-key.hex           (tracked, convenience copy)
//
// Usage: node scripts/license/generate-license-keys.mjs [--force]
// Existing keys are refused unless --force is passed (rotation is a deliberate act).

import { generateKeyPairSync } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const privateDir = join(repoRoot, "security", "license-issuer", "private");
const privatePemPath = join(privateDir, "issuer-private-key.pem");
const publicHexPath = join(repoRoot, "security", "license-issuer", "issuer-public-key.hex");

if (!process.argv.includes("--force") && existsSync(privatePemPath)) {
  console.error(`Refusing to overwrite existing issuer key: ${privatePemPath}`);
  console.error("Key rotation is deliberate; pass --force only after securely archiving the old key.");
  process.exit(1);
}

const { publicKey, privateKey } = generateKeyPairSync("ed25519");

mkdirSync(privateDir, { recursive: true });
writeFileSync(privatePemPath, privateKey.export({ type: "pkcs8", format: "pem" }), { mode: 0o600 });

const jwk = publicKey.export({ format: "jwk" });
const rawPublic = Buffer.from(jwk.x, "base64url");
if (rawPublic.length !== 32) {
  throw new Error(`unexpected raw public key length ${rawPublic.length}`);
}
const publicHex = rawPublic.toString("hex");

mkdirSync(dirname(publicHexPath), { recursive: true });
writeFileSync(publicHexPath, publicHex + "\n");

console.log("Issuer keypair generated.");
console.log(`Private key (NEVER commit): ${privatePemPath}`);
console.log(`Public key copy:            ${publicHexPath}`);
console.log("");
console.log("Embed this in src-tauri/src/license.rs as ISSUER_PUBLIC_KEY:");
console.log(`const ISSUER_PUBLIC_KEY_HEX: &str = "${publicHex}";`);
