# License Issuer Key Custody

This directory is the root of trust for R2H-PDF license entitlement.

```text
private/issuer-private-key.pem   Ed25519 signing key (PKCS#8 PEM).
                                 GITIGNORED. Never commit, never copy into
                                 the repository tree, never ship it.
issuer-public-key.hex            Raw 32-byte verification key (hex).
                                 Tracked for convenience; the authoritative
                                 copy is embedded in src-tauri/src/license.rs.
```

Rules:

1. The private key lives ONLY here (gitignored) and in the engineering
   secret-store backup. If this file is lost, all licenses must be reissued
   with a newly generated keypair.
2. Rotation is deliberate: `node scripts/license/generate-license-keys.mjs
   --force`, archive the old key securely, update `ISSUER_PUBLIC_KEY_HEX`
   in `src-tauri/src/license.rs`, cut a new release. Licenses signed by the
   retired key stop verifying.
3. Issue licenses only with `scripts/license/issue-license.mjs`.
4. Verify any customer license with:
   `cargo run --example verify_license_file -- <license.json>` from
   `src-tauri/`.

See `docs/license/TRUST_MODEL.md` for the full design and evidence.
