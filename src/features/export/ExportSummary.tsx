import type { ExportResult } from "./types";

interface ExportSummaryProps {
  result: ExportResult;
}

function Row({ label, value, hide }: { label: string; value: number | undefined; hide?: boolean }) {
  if (hide && (value ?? 0) === 0) return null;
  return (
    <>
      <span>{label}:</span>
      <strong>{value ?? 0}</strong>
    </>
  );
}

export const ExportSummary = ({ result }: ExportSummaryProps) => {
  const fileSize =
    typeof result.output_file_size_bytes === "number"
      ? formatBytes(result.output_file_size_bytes)
      : "Not reported";

  return (
    <div className="export-summary" data-testid="export-summary">
      <h5>Export Complete</h5>
      <div className="export-summary__grid">
        <span>Output path:</span><strong className="path-mono" title={result.output_path}>{result.output_path}</strong>
        <span>File size:</span><strong>{fileSize}</strong>
        <span>Output SHA-256:</span><strong className="path-mono">{result.output_sha256}</strong>
        <span>Pages:</span><strong>{result.page_count}</strong>
        <span>Independent reopen:</span><strong>{result.independently_validated ? "Validated" : "Not validated"}</strong>
        <span>Objects received:</span><strong>{result.overlay_objects_received_count}</strong>
        <span>Annotations committed:</span><strong>{result.annotations_committed_count}</strong>
        <span>Redactions applied:</span><strong>{result.redactions_applied_count}</strong>
        <span>Redaction mode:</span><strong>{result.redaction_mode === "export_copy" ? "Export copy" : "Applied to document"}</strong>
        <span>Source mutated:</span><strong>{result.source_document_mutated ? "Yes" : "No"}</strong>
      </div>

      {/* Phase 26A/F: loud warning when signatures could not be embedded. */}
      {(result.signatures_failed_count ?? 0) > 0 && (
        <div
          className="export-summary__signature-failed"
          data-testid="export-summary-signature-failed"
          style={{
            border: "1px solid var(--danger, #c33)",
            background: "var(--danger-bg, rgba(204,51,51,0.08))",
            padding: 6,
            borderRadius: 4,
            marginTop: 8,
          }}
        >
          <strong style={{ color: "var(--danger, #c33)" }}>
            {result.signatures_failed_count} signature image(s) could NOT be embedded
          </strong>
          <p style={{ fontSize: 12, marginTop: 2 }}>
            The affected signatures appear only as Stamp labels (text), not as the actual bitmap,
            in the exported PDF. Open externally to verify and re-export after fixing the
            source image.
          </p>
          {result.signatures_failed_items && result.signatures_failed_items.length > 0 && (
            <ul style={{ fontSize: 11, marginTop: 4 }}>
              {result.signatures_failed_items.map((s, i) => <li key={i}>{s}</li>)}
            </ul>
          )}
        </div>
      )}

      {/* Phase 25F + 26F: per-kind subtotals shown only when non-zero. */}
      <details
        className="export-summary__breakdown"
        data-testid="export-summary-breakdown"
        open
      >
        <summary>Objects breakdown</summary>
        <div className="export-summary__grid">
          <Row label="Text boxes" value={result.text_boxes_count} hide />
          <Row label="Comments" value={result.comments_count} hide />
          <Row label="Highlights" value={result.highlights_count} hide />
          <Row label="Shapes / rectangles" value={result.shapes_count} hide />
          <Row label="Stamps" value={result.stamps_count} hide />
          <Row label="Signature images (overlays)" value={result.signatures_count} hide />
          <Row label="Signature images embedded" value={result.signatures_embedded_count} hide />
          <Row label="Signature images aspect preserved" value={result.signatures_aspect_preserved_count} hide />
          <Row label="Signature images failed" value={result.signatures_failed_count} hide />
          <Row label="Strikethroughs" value={result.strikethroughs_count} hide />
          <Row label="Underlines" value={result.underlines_count} hide />
          <Row label="Images" value={result.images_count} hide />
          <Row label="OCR overlays" value={result.ocr_overlays_count} hide />
          <Row label="OCR text layers (searchable)" value={result.ocr_text_layers_count} hide />
          <Row label="Path covers" value={result.path_covers_count} hide />
          {/* Phase 28G — text edit counters. */}
          <Row label="Native text edits" value={result.native_text_edits_count} hide />
          <Row label="Native block edits" value={result.native_block_edits_count} hide />
          <Row label="Visual text replacements" value={result.visual_text_replacements_count} hide />
          <Row label="Paragraph reflows" value={result.paragraph_reflows_count} hide />
          <Row label="Find/replace edits" value={result.find_replace_count} hide />
          <Row label="Read-only text rejected" value={result.read_only_text_count} hide />
          <Row label="Font preservation (text edits)" value={result.font_preservation_count} hide />
          <Row label="Text edits fallback to visual" value={result.text_fallback_count} hide />
          <Row label="Image moves/resizes" value={result.image_edits_count} hide />
          <Row label="Image replacements" value={result.image_replacements_count} hide />
          <Row label="Image crops" value={result.image_crops_count} hide />
          <Row label="Image rotations" value={result.image_rotations_count} hide />
          <Row label="Image deletes/covers" value={result.image_deletes_count} hide />
          <Row label="Visual covers" value={result.visual_covers_count} hide />
          <Row label="Sampled background covers" value={result.sampled_background_covers_count} hide />
          <Row label="White fallback covers" value={result.white_fallback_covers_count} hide />
          <Row label="Compare comments" value={result.compare_annotations_count} hide />
          <Row label="Compare visual rects" value={result.compare_visual_annotations_count} hide />
          <Row label="AI Review comments" value={result.ai_review_annotations_count} hide />
          <Row label="Skipped (not available)" value={result.skipped_unsupported_count} hide />
        </div>
      </details>

      {!result.true_flattening_supported && result.annotations_committed_count > 0 && (
        <p className="export-summary__note">
          Annotations were committed with appearance streams. They display correctly in all PDF viewers but remain editable annotation objects.
        </p>
      )}
      {/* Phase 27G — z-order honesty: if the export pipeline cannot
          preserve editor z-order in the output PDF, say so loudly. */}
      {result.zorder_export_limited && (
        <p
          className="export-summary__note"
          data-testid="export-summary-zorder-warning"
          style={{ color: "var(--warn, #c80)" }}
        >
          Object stacking order in the exported PDF follows annotation creation order, which may
          not match your editor z-order. Visual rendering inside the editor honours z-order; the
          exported file's annotation array does not.
        </p>
      )}
      {result.skipped_items && result.skipped_items.length > 0 && (
        <details className="export-summary__skipped">
          <summary>{result.skipped_items.length} skipped item(s)</summary>
          <ul>{result.skipped_items.map((s, i) => <li key={i}>{s}</li>)}</ul>
        </details>
      )}
      {result.warnings.length > 0 && (
        <details className="export-summary__warnings">
          <summary>{result.warnings.length} warning(s)</summary>
          <ul>{result.warnings.map((w, i) => <li key={i}>{w}</li>)}</ul>
        </details>
      )}
    </div>
  );
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}
