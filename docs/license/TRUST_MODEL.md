# R2H-PDF License Trust Model

Status: **VERIFIED** (implementation date: 2026-08-30)

## Model

R2H-PDF licensing uses **Ed25519 asymmetric signatures** (RFC 8032) via the
`ed25519-dalek` crate (2.2.0).

```text
License issuer (R2H):
  holds the private Ed25519 key, offline, outside the repository
  signs canonical license payload bytes with scripts/license/issue-license.mjs

R2H-PDF application:
  embeds ONLY the 32-byte public verification key
  verifies signatures offline; contains no signing capability
```

The application can never mint a valid license. Compromising a distributed
binary does not yield any signing capability beyond what verification already
permits.

## License document (schema 2)

```json
{
  "schema": 2,
  "payload": {
    "license_id": "<uuid>",
    "subject": "<licensee>",
    "issued_unix": 1756500000,
    "expires_unix": 1788036000,
    "features": ["export", "ocr", "rag", "compare"]
  },
  "signature": "<base64 of 64-byte Ed25519 signature>"
}
```

The signature covers the **canonical serde serialization** of the payload
(compact JSON, field order `license_id, subject, issued_unix, expires_unix,
features`). The issuer reproduces exactly these bytes with `JSON.stringify`
using the same field order. Keep subject/license_id ASCII to avoid any
encoding divergence.

Verification steps (`src-tauri/src/license.rs::verify_license_with_key`):

1. JSON parse; missing `schema` field → explicit legacy rejection.
2. `schema != 2` → "unsupported license schema".
3. base64-decode signature; must be exactly 64 bytes.
4. Ed25519 `verify_strict` of the canonical payload bytes against the
   embedded public key.
5. Expiry check: valid through the expiration second
   (`now == expires_unix` is still valid, `now > expires_unix` is expired).

Feature gating (`assert_feature_allowed`): in licensed mode the requested
feature must appear in `payload.features` (`"*"` grants everything). Trial
mode permits all gated features (export / OCR / RAG / compare).

## Key custody

- Private key: `security/license-issuer/private/issuer-private-key.pem`
  (PKCS#8 PEM). **Gitignored — never committed, never shipped.** Back it up
  in the engineering secret store; losing it means re-issuing all licenses
  with a new keypair (rotation: `scripts/license/generate-license-keys.mjs
  --force`, then update `ISSUER_PUBLIC_KEY_HEX` in `license.rs` and release).
- Public key: embedded as `ISSUER_PUBLIC_KEY_HEX` in
  `src-tauri/src/license.rs`; a convenience copy lives at
  `security/license-issuer/issuer-public-key.hex`.

## Issuing a license

```bash
node scripts/license/issue-license.mjs \
  --subject "ACME Corp" --days 365 \
  --features export,ocr,rag,compare --out license.json
```

The customer places `license.json` in `%LOCALAPPDATA%\R2H-PDF\license\`
(or the directory shown by the in-app license status). Verification is fully
offline.

## Migration and rejection behavior

- **Legacy schema-1 licenses** (HMAC-style signature produced with the
  embedded shared secret that the application shipped before 2026-08-30) are
  detected by the missing `schema` field and rejected with the explicit
  message *"legacy license format (schema 1) is no longer accepted; request a
  reissued schema-2 license"*. No compatibility path preserves the old
  shared-secret design.
- **Legacy trial-state files** (no `schema` field) are migrated in place on
  first evaluation: counters and the original trial start are preserved and
  the state is re-sealed with the current scheme. Migration grants no
  additional trial time; it is equivalent in strength to the old scheme
  because the legacy secret shipped inside the binary and protected nothing.
- **Trial-state integrity** is now a domain-separated SHA-256 digest
  (`R2H-PDF/trial-state/v2` + body). This is a casual-tamper deterrent only:
  a local state file can always be deleted for a fresh trial, and no embedded
  secret can change that. Authoritative entitlement comes exclusively from a
  signed license file. A schema-2 state with a mismatched digest is locked.

## Known limitation

The trial launch counter (`MAX_LAUNCHES = 50`) is evaluated but no production
code increments it; the operative trial limit is the 14-day window from
`first_seen_unix`. Documented here rather than silently claimed.

## Verification evidence

- 18 unit tests in `src-tauri/src/license.rs` cover: valid signed license,
  invalid signature, modified payload, wrong public key, expired license,
  malformed JSON, unsupported schema, legacy rejection, missing payload
  fields, feature scoping, fresh trial, launch-count expiry, date rollback,
  integrity mismatch (locked), legacy trial migration (start preserved and
  no state mutation on rejected use), boundary inclusivity, digest
  stability.
- Issuer/verifier interop proven live: a license issued by
  `scripts/license/issue-license.mjs` verified `VALID` through
  `cargo run --example verify_license_file`, and a payload-tampered copy of
  the same file returned `INVALID: license signature mismatch`.
- Runtime smokes (`--license-smoke offline|trial-expiry|tamper`) all report
  `PASS` with every probe passing.
