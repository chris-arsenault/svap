import React, { useState, useRef, useCallback } from "react";
import { ExternalLink, Upload, Check, AlertTriangle, Clock, XCircle, Plus, Trash2, ChevronDown, ChevronRight } from "lucide-react";
import { useEnforcementSources, useUploadSourceDocument, useDeleteSource, useCreateSource } from "../data/usePipelineSelectors";
import { useAsyncAction } from "../hooks";
import { ErrorBanner } from "../components/SharedUI";
import type { EnforcementSource, ValidationStatus } from "../types";

function StatusBadge({ status, hasDocument }: { status: ValidationStatus; hasDocument: boolean }) {
  if (!hasDocument) {
    return <span className="badge badge-neutral">No document</span>;
  }
  const map: Record<ValidationStatus, { cls: string; label: string }> = {
    pending: { cls: "badge-neutral", label: "Pending" },
    valid: { cls: "badge-low", label: "Valid" },
    invalid: { cls: "badge-high", label: "Invalid" },
    error: { cls: "badge-critical", label: "Error" },
  };
  const s = map[status] || map.pending;
  return <span className={`badge ${s.cls}`}>{s.label}</span>;
}

function StatusIcon({ status, hasDocument }: { status: ValidationStatus; hasDocument: boolean }) {
  if (!hasDocument) return <Clock size={14} className="text-muted" />;
  if (status === "valid") return <Check size={14} className="text-low" />;
  if (status === "invalid") return <AlertTriangle size={14} className="text-high" />;
  if (status === "error") return <XCircle size={14} className="text-critical" />;
  return <Clock size={14} className="text-muted" />;
}

function SourceExpandedDetail({ source, error, onDismiss }: {
  source: EnforcementSource;
  error: string | null;
  onDismiss: () => void;
}) {
  return (
    <tr>
      <td colSpan={6} className="case-detail-cell">
        <div className="detail-expand">
          <ErrorBanner error={error} onDismiss={onDismiss} />
          {source.summary && (
            <>
              <div className="detail-label">Summary</div>
              <div className="detail-text">{source.summary}</div>
            </>
          )}
          <div className="detail-label">Description</div>
          <div className="detail-text">{source.description || "No description"}</div>
          {source.url && (
            <>
              <div className="detail-label">URL</div>
              <div className="detail-text">
                <a href={source.url} target="_blank" rel="noreferrer">{source.url}</a>
              </div>
            </>
          )}
          {source.s3_key && (
            <>
              <div className="detail-label">S3 Key</div>
              <div className="detail-text mono">{source.s3_key}</div>
            </>
          )}
        </div>
      </td>
    </tr>
  );
}

function uploadButtonLabel(busy: string | null, hasDocument: boolean): string {
  if (busy === "upload") return "Uploading...";
  return hasDocument ? "Replace" : "Upload";
}

function SourceRowActions({ source, busy, onUploadClick, onDelete, fileInputRef, onUpload }: {
  source: EnforcementSource;
  busy: string | null;
  onUploadClick: (e: React.MouseEvent) => void;
  onDelete: (e: React.MouseEvent) => void;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  onUpload: (e: React.ChangeEvent<HTMLInputElement>) => void;
}) {
  return (
    <td>
      <div className="source-actions">
        <input ref={fileInputRef} type="file" accept=".html,.htm,.txt,.pdf" onChange={onUpload} hidden />
        <button className="btn btn-sm" onClick={onUploadClick} disabled={!!busy}>
          <Upload size={12} /> {uploadButtonLabel(busy, source.has_document)}
        </button>
        <button className="btn btn-sm btn-danger" onClick={onDelete} title="Remove source">
          <Trash2 size={12} />
        </button>
      </div>
    </td>
  );
}

function SourceRow({ source }: { source: EnforcementSource }) {
  const uploadSourceDocument = useUploadSourceDocument();
  const deleteSource = useDeleteSource();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { busy, error, run, clearError } = useAsyncAction();
  const [expanded, setExpanded] = useState(false);
  const toggleExpanded = useCallback(() => setExpanded((prev) => !prev), []);

  const handleUpload = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    await run("upload", () => uploadSourceDocument(source.source_id, file));
    if (fileInputRef.current) fileInputRef.current.value = "";
  }, [source.source_id, uploadSourceDocument, run]);

  const handleDelete = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm(`Remove source "${source.name}"?`)) return;
    await run("delete", () => deleteSource(source.source_id));
  }, [source.source_id, source.name, deleteSource, run]);

  const handleUploadClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    fileInputRef.current?.click();
  }, []);

  return (
    <>
      <tr
        className="detail-row"
        onClick={toggleExpanded}
        tabIndex={0}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleExpanded(); } }}
      >
        <td className="td-toggle">{expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}</td>
        <td>
          <div className="source-name-cell">
            <StatusIcon status={source.validation_status} hasDocument={source.has_document} />
            <span>{source.name}</span>
          </div>
        </td>
        <td><span className="badge badge-neutral">{source.source_type.replace("_", " ")}</span></td>
        <td><StatusBadge status={source.validation_status} hasDocument={source.has_document} /></td>
        <td>
          {source.url && (
            <a href={source.url} target="_blank" rel="noreferrer" className="source-card-link"
              onClick={(e) => e.stopPropagation()} title={source.url}>
              <ExternalLink size={14} />
            </a>
          )}
        </td>
        <SourceRowActions source={source} busy={busy} onUploadClick={handleUploadClick}
          onDelete={handleDelete} fileInputRef={fileInputRef} onUpload={handleUpload} />
      </tr>
      {!expanded && error && (
        <tr><td colSpan={6} className="case-detail-cell"><ErrorBanner error={error} onDismiss={clearError} /></td></tr>
      )}
      {expanded && <SourceExpandedDetail source={source} error={error} onDismiss={clearError} />}
    </>
  );
}

function AddSourceForm({ onClose }: { onClose: () => void }) {
  const createSource = useCreateSource();
  const { busy, error, run, clearError } = useAsyncAction();

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const fd = new FormData(e.currentTarget);
    const name = String(fd.get("name") ?? "").trim();
    if (!name) return;
    await run("add", async () => {
      await createSource({
        name,
        url: String(fd.get("url") ?? "").trim() || undefined,
        description: String(fd.get("description") ?? "").trim() || undefined,
      });
      onClose();
    });
  };

  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Add Source</h3>
      </div>
      <div className="panel-body">
        <form className="add-source-form" onSubmit={handleSubmit}>
          <input name="name" placeholder="Source name (required)" required />
          <input name="url" placeholder="URL (optional)" type="url" />
          <input name="description" placeholder="Description (optional)" />
          <ErrorBanner error={error} onDismiss={clearError} />
          <div className="add-source-actions">
            <button type="submit" className="btn btn-accent" disabled={!!busy}>
              {busy ? "Adding..." : "Add Source"}
            </button>
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export default function SourcesView() {
  const enforcement_sources = useEnforcementSources();
  const [showAddForm, setShowAddForm] = useState(false);
  const hideAddForm = useCallback(() => setShowAddForm(false), []);

  const withDoc = enforcement_sources.filter((s) => s.has_document).length;
  const validated = enforcement_sources.filter((s) => s.validation_status === "valid").length;

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Enforcement Sources</h2>
        <div className="view-desc">
          Manage enforcement source documents. Upload files manually or let the pipeline fetch from
          URLs. Documents are validated during Stage 0.
        </div>
      </div>

      <div className="metrics-row">
        <div className="metric-card stagger-in">
          <div className="metric-label">Total Sources</div>
          <div className="metric-value">{enforcement_sources.length}</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">With Documents</div>
          <div className="metric-value">{withDoc}</div>
          <div className="metric-sub">{enforcement_sources.length - withDoc} pending fetch</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Validated</div>
          <div className="metric-value">{validated}</div>
          <div className="metric-sub">confirmed by LLM</div>
        </div>
      </div>

      {showAddForm && <AddSourceForm onClose={hideAddForm} />}

      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Source Registry</h3>
          <div className="pipeline-header-actions">
            <button className="btn btn-accent" onClick={() => setShowAddForm(!showAddForm)}>
              <Plus size={14} />
              Add Source
            </button>
          </div>
        </div>
        <div className="panel-body dense">
          <table className="data-table">
            <thead>
              <tr>
                <th className="th-toggle"></th>
                <th>Source</th>
                <th>Type</th>
                <th>Status</th>
                <th className="th-icon">Link</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {enforcement_sources.map((src) => (
                <SourceRow key={src.source_id} source={src} />
              ))}
              {enforcement_sources.length === 0 && (
                <tr>
                  <td colSpan={6} className="empty-state">
                    No enforcement sources configured. Add a source to get started.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
