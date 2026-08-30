# AGENTS.md

R2H Engineering family repository.

Authority order:
1. Ramy
2. `E:\Projects\R2H-Desktop\R2H_ENGINEERING_MASTER_CHARTER.md`
3. this repository AGENTS.md
4. assigned task
5. fresh verification evidence

Preserve user work, stay inside task scope, and never commit/push/release/deploy without explicit authorization.
Use PASS / PARTIAL / FAIL / NOT_VERIFIED / BLOCKED precisely.

## Repository role — R2H-PDF
Source of reusable DocumentEngine capabilities.

Priority capabilities:
- PDF rendering;
- text/vector extraction;
- OCR only where justified;
- annotations;
- compare;
- page operations;
- document analysis/extraction.

Prefer a stable reusable engine/interface boundary over duplicating UI inside R2H.AI-ELE.

Processing order:
vector/text extraction → deterministic processing → OCR fallback → local AI only for unresolved ambiguity.

No cloud-required production path.
