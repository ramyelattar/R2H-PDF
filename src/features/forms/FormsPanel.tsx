import { useCallback, useEffect, useState } from "react";
import {
  docListFormFields,
  editApplyTransaction,
  pdfCreateFormField,
  pdfDeleteFormField,
  type CreateFieldType,
  type EditTransaction,
  type FormField,
} from "../../lib/ipc";

interface FormsPanelProps {
  sessionId: string;
  pageIndex?: number;
  onNavigateToPage?: (pageIndex: number) => void;
  onContentEdited?: () => void;
}

interface NewFieldDraft {
  type: CreateFieldType;
  name: string;
  value: string;
  options: string;
  rect: { x: number; y: number; w: number; h: number };
  required: boolean;
  readOnly: boolean;
  fontSize: number;
}

const defaultDraft = (pageIndex: number): NewFieldDraft => ({
  type: "text",
  name: `Field_${pageIndex + 1}_${Math.floor(Math.random() * 1000)}`,
  value: "",
  options: "",
  rect: { x: 72, y: 72, w: 200, h: 28 },
  required: false,
  readOnly: false,
  fontSize: 12,
});

export const FormsPanel = ({ sessionId, pageIndex, onNavigateToPage, onContentEdited }: FormsPanelProps) => {
  const [fields, setFields] = useState<FormField[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editingField, setEditingField] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [showCreator, setShowCreator] = useState(false);
  const [draft, setDraft] = useState<NewFieldDraft>(defaultDraft(pageIndex ?? 0));
  const [busy, setBusy] = useState(false);

  const loadFields = useCallback(async () => {
    setLoading(true);
    setError(null);
    const result = await docListFormFields(sessionId);
    setLoading(false);
    if (result.ok) {
      setFields(result.data);
    } else {
      setError(result.error.message);
    }
  }, [sessionId]);

  useEffect(() => { void loadFields(); }, [loadFields]);

  const handleEditField = (field: FormField) => {
    setEditingField(field.name);
    setEditValue(field.value);
  };

  const handleApplyFieldEdit = async (field: FormField) => {
    const fieldName = field.name;
    const txId = `form-edit-${Date.now()}`;
    const tx: EditTransaction = {
      transaction_id: txId,
      session_id: sessionId,
      operations: [{
        id: `op-${txId}`,
        op_type: "FillFormField",
        session_id: sessionId,
        page_index: field.page_index,
        object_ref: null,
        payload_json: JSON.stringify({ field_name: fieldName, value: editValue }),
      }],
      description: `Fill form field: ${fieldName}`,
    };

    const result = await editApplyTransaction(tx);
    if (result.ok && result.data.success) {
      setEditingField(null);
      void loadFields();
      onContentEdited?.();
    } else {
      setError(result.ok ? (result.data.error ?? "Failed") : result.error.message);
    }
  };

  const handleToggleCheckbox = async (field: FormField) => {
    const newValue = field.value === "Yes" ? "Off" : "Yes";
    const txId = `form-toggle-${Date.now()}`;
    const tx: EditTransaction = {
      transaction_id: txId,
      session_id: sessionId,
      operations: [{
        id: `op-${txId}`,
        op_type: "FillFormField",
        session_id: sessionId,
        page_index: field.page_index,
        object_ref: null,
        payload_json: JSON.stringify({ field_name: field.name, value: newValue }),
      }],
      description: `Toggle checkbox: ${field.name}`,
    };

    const result = await editApplyTransaction(tx);
    if (result.ok && result.data.success) {
      void loadFields();
      onContentEdited?.();
    } else {
      setError(result.ok ? (result.data.error ?? "Failed") : result.error.message);
    }
  };

  const handleCreate = async () => {
    setError(null);
    setNotice(null);
    if (!draft.name.trim()) {
      setError("Field name is required");
      return;
    }
    if (draft.rect.w <= 0 || draft.rect.h <= 0) {
      setError("Field size must be positive");
      return;
    }
    const optionsArr = draft.options
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    setBusy(true);
    const result = await pdfCreateFormField({
      session_id: sessionId,
      page_index: pageIndex ?? 0,
      field_type: draft.type,
      name: draft.name.trim(),
      value: draft.value || null,
      default_value: null,
      options: optionsArr.length > 0 ? optionsArr : null,
      rect: [draft.rect.x, draft.rect.y, draft.rect.x + draft.rect.w, draft.rect.y + draft.rect.h],
      required: draft.required,
      read_only: draft.readOnly,
      font_size: draft.fontSize,
      border_color: null,
      fill_color: null,
    });
    setBusy(false);
    if (!result.ok) {
      setError(result.error.message);
      return;
    }
    if (!result.data.success) {
      setError("Create failed");
      return;
    }
    const created = result.data.field_name;
    const warns = result.data.warnings.join("; ");
    setNotice(`Created field "${created}"${warns ? ` — ${warns}` : ""}`);
    setShowCreator(false);
    setDraft(defaultDraft(pageIndex ?? 0));
    await loadFields();
    onContentEdited?.();
  };

  const handleDelete = async (fieldName: string) => {
    setError(null);
    setNotice(null);
    setBusy(true);
    const result = await pdfDeleteFormField({ session_id: sessionId, field_name: fieldName });
    setBusy(false);
    if (!result.ok) {
      setError(result.error.message);
      return;
    }
    if (!result.data.success) {
      setError(result.data.warnings.join("; ") || `Delete failed for field "${fieldName}".`);
      return;
    }
    setNotice(`Deleted field "${fieldName}"`);
    await loadFields();
    onContentEdited?.();
  };

  return (
    <div className="forms-panel" data-testid="forms-panel">
      <div className="section-header">
        <h4 className="section-header__title">Forms</h4>
        <span className="section-header__hint">{fields.length} field{fields.length !== 1 ? "s" : ""}</span>
      </div>

      {loading && <div className="loading-bar" />}
      {error && <div className="callout callout--danger"><span className="callout__icon" aria-hidden="true">!</span><div className="callout__body">{error}</div></div>}
      {notice && <div className="callout callout--success"><span className="callout__icon" aria-hidden="true">i</span><div className="callout__body">{notice}</div></div>}
      <p className="empty-text">Field required/read-only flags are accepted when creating a field, but this runtime does not expose them in the authoritative field-list response.</p>

      <div className="forms-actions" style={{ display: "flex", gap: 6, margin: "6px 0" }}>
        <button
          className="btn btn--secondary btn--sm"
          disabled={busy}
          onClick={() => { setDraft({ ...defaultDraft(pageIndex ?? 0), type: "text" }); setShowCreator(true); setError(null); setNotice(null); }}
          title="Create a new form field on the current page"
          data-testid="forms-create-text"
        >
          Create text field
        </button>
        <button
          className="btn btn--secondary btn--sm"
          disabled={busy}
          onClick={() => { setDraft({ ...defaultDraft(pageIndex ?? 0), type: "checkbox", value: "Off" }); setShowCreator(true); setError(null); setNotice(null); }}
          data-testid="forms-create-checkbox"
        >
          Create checkbox
        </button>
        {showCreator && (
          <button className="btn btn--ghost btn--sm" onClick={() => setShowCreator(false)}>Cancel</button>
        )}
        <button className="btn btn--ghost btn--sm" onClick={() => void loadFields()} disabled={loading || busy}>
          Refresh
        </button>
      </div>

      {showCreator && (
        <div className="forms-creator inspector-card" data-testid="forms-create-card">
          <p className="inspector-card__title">Create field</p>
          <p className="empty-text" style={{ marginTop: 0 }}>
            New field on page {(pageIndex ?? 0) + 1}. Coordinates are in PDF points
            (1pt = 1/72&quot;, origin at bottom-left).
          </p>
          <label style={{ display: "block", marginBottom: 4 }}>
            Type
            <select
              value={draft.type}
              onChange={(e) => setDraft({ ...draft, type: e.target.value as CreateFieldType })}
              style={{ marginLeft: 6 }}
              data-testid="create-field-type"
            >
              <option value="text">Text</option>
              <option value="checkbox">Checkbox</option>
              <option value="combo">Combo</option>
              <option value="list">List</option>
              <option value="signature">Signature</option>
            </select>
          </label>
          <label style={{ display: "block", marginBottom: 4 }}>
            Name
            <input
              type="text"
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              className="compact-input"
              style={{ marginLeft: 6, width: 180 }}
              data-testid="create-field-name"
            />
          </label>
          {(draft.type === "text" || draft.type === "combo" || draft.type === "list") && (
            <label style={{ display: "block", marginBottom: 4 }}>
              Value
              <input
                type="text"
                value={draft.value}
                onChange={(e) => setDraft({ ...draft, value: e.target.value })}
                className="compact-input"
                style={{ marginLeft: 6, width: 180 }}
                data-testid="create-field-value"
              />
            </label>
          )}
          {(draft.type === "combo" || draft.type === "list") && (
            <label style={{ display: "block", marginBottom: 4 }}>
              Options (comma-sep)
              <input
                type="text"
                value={draft.options}
                onChange={(e) => setDraft({ ...draft, options: e.target.value })}
                className="compact-input"
                style={{ marginLeft: 6, width: 220 }}
                placeholder="Yes, No, Maybe"
              />
            </label>
          )}
          <div style={{ display: "grid", gridTemplateColumns: "auto auto auto auto", gap: 4, marginBottom: 4 }}>
            <label>x<input type="number" value={draft.rect.x} onChange={(e) => setDraft({ ...draft, rect: { ...draft.rect, x: parseFloat(e.target.value) || 0 } })} style={{ width: 60, marginLeft: 4 }} /></label>
            <label>y<input type="number" value={draft.rect.y} onChange={(e) => setDraft({ ...draft, rect: { ...draft.rect, y: parseFloat(e.target.value) || 0 } })} style={{ width: 60, marginLeft: 4 }} /></label>
            <label>w<input type="number" value={draft.rect.w} onChange={(e) => setDraft({ ...draft, rect: { ...draft.rect, w: parseFloat(e.target.value) || 0 } })} style={{ width: 60, marginLeft: 4 }} /></label>
            <label>h<input type="number" value={draft.rect.h} onChange={(e) => setDraft({ ...draft, rect: { ...draft.rect, h: parseFloat(e.target.value) || 0 } })} style={{ width: 60, marginLeft: 4 }} /></label>
          </div>
          <label style={{ display: "inline-block", marginRight: 8 }}>
            <input type="checkbox" checked={draft.required} onChange={(e) => setDraft({ ...draft, required: e.target.checked })} /> Required
          </label>
          <label style={{ display: "inline-block", marginRight: 8 }}>
            <input type="checkbox" checked={draft.readOnly} onChange={(e) => setDraft({ ...draft, readOnly: e.target.checked })} /> Read-only
          </label>
          {(draft.type === "text" || draft.type === "combo" || draft.type === "list") && (
            <label style={{ display: "inline-block" }}>
              Font size
              <input
                type="number"
                value={draft.fontSize}
                onChange={(e) => setDraft({ ...draft, fontSize: parseFloat(e.target.value) || 12 })}
                style={{ width: 60, marginLeft: 4 }}
                min={1} max={72}
              />
            </label>
          )}
          <div style={{ marginTop: 6 }}>
            <button className="btn btn--primary btn--sm" onClick={() => void handleCreate()} disabled={busy} data-testid="create-field-apply">
              {busy ? "Creating..." : `Create ${draft.type} field`}
            </button>
          </div>
          {draft.type === "signature" && (
            <p className="text-warn" style={{ marginTop: 6, fontSize: 11 }}>
              Signature field — not a certificate-based digital signature.
            </p>
          )}
        </div>
      )}

      {!loading && fields.length === 0 && !showCreator && (
        <p className="empty-text">No form fields detected in this document.</p>
      )}

      {fields.length > 0 && (
        <ul className="forms-field-list">
          {fields.map((field) => (
            <li key={field.name} className="forms-field-item">
              <div className="forms-field-header">
                <span className="forms-field-name">{field.name}</span>
                <span className="forms-field-type badge">{formatFieldType(field.field_type)}</span>
                <button
                  className="forms-field-nav ghost-btn"
                  onClick={() => onNavigateToPage?.(field.page_index)}
                  title={`Go to page ${field.page_index + 1}`}
                >
                  p.{field.page_index + 1}
                </button>
                <button
                  className="ghost-btn"
                  onClick={() => void handleDelete(field.name)}
                  disabled={busy}
                  title={`Delete field ${field.name}. This permanently removes the form field.`}
                  style={{ color: "var(--danger, #c33)" }}
                  data-testid="forms-delete-field"
                >
                  Delete
                </button>
              </div>
              <div className="forms-field-row-meta">
                <span className="badge">Page {field.page_index + 1}</span>
                <span className="badge">Value: {field.value || "(empty)"}</span>
                <span className="badge">Flags: backend-listed on create</span>
              </div>

              {field.field_type === "text" && (
                <div className="forms-field-edit">
                  {editingField === field.name ? (
                    <>
                      <input
                        type="text"
                        value={editValue}
                        onChange={(e) => setEditValue(e.target.value)}
                        className="compact-input"
                      />
                      <button className="ghost-btn" onClick={() => void handleApplyFieldEdit(field)}>
                        Save
                      </button>
                      <button className="ghost-btn" onClick={() => setEditingField(null)}>
                        Cancel
                      </button>
                    </>
                  ) : (
                    <>
                      <span className="forms-field-value">{field.value || "(empty)"}</span>
                      <button className="ghost-btn" onClick={() => handleEditField(field)}>
                        Edit
                      </button>
                    </>
                  )}
                </div>
              )}

              {field.field_type === "checkbox" && (
                <div className="forms-field-edit">
                  <label>
                  <input
                      type="checkbox"
                      checked={field.value === "Yes"}
                      onChange={() => void handleToggleCheckbox(field)}
                      title="Toggle checkbox value"
                    />
                    {field.value === "Yes" ? "Checked" : "Unchecked"}
                  </label>
                </div>
              )}

              {field.field_type === "radio" && (
                <div className="forms-field-edit">
                  <span className="forms-field-value">Value: {field.value || "(none)"}</span>
                </div>
              )}

              {field.field_type === "signature" && (
                <div className="forms-field-edit">
                  <span className="forms-field-value text-warn">
                    Signature field (visual placement only — not cryptographic)
                  </span>
                </div>
              )}

            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

function formatFieldType(type: string): string {
  switch (type) {
    case "text": return "Text";
    case "checkbox": return "Checkbox";
    case "combo": return "Combo";
    case "list": return "List";
    case "radio": return "Radio";
    case "signature": return "Signature";
    default: return type.replace(/_/g, " ");
  }
}
