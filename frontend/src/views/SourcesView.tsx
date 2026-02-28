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

function SourceRow({ source }: { source: EnforcementSource }) {
  const uploadSourceDocument = useUploadSourceDocument();
  const deleteSource = useDeleteSource();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [expanded, setExpanded] = useState(false);

  const handleUpload = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      setUploading(true);
      try {
        await uploadSourceDocument(source.source_id, file);
      } catch (err) {
        console.error("Upload failed:", err);
      } finally {
        setUploading(false);
        if (fileInputRef.current) fileInputRef.current.value = "";
      }
    },
    [source.source_id, uploadSourceDocument]
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!confirm(`Remove source "${source.name}"?`)) return;
      try {
        await deleteSource(source.source_id);
      } catch (err) {
        console.error("Delete failed:", err);
      }
    },
    [source.source_id, source.name, deleteSource]
  );

  return (
    <>
      <tr className="detail-row" onClick={() => setExpanded(!expanded)}>
        <td className="td-toggle">
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </td>
        <td>
          <div className="source-name-cell">
            <StatusIcon status={source.validation_status} hasDocument={source.has_document} />
            <span>{source.name}</span>
          </div>
        </td>
        <td>
          <span className="badge badge-neutral">{source.source_type.replace("_", " ")}</span>
        </td>
        <td>
          <StatusBadge status={source.validation_status} hasDocument={source.has_document} />
        </td>
        <td>
          {source.url && (
            <a
              href={source.url}
              target="_blank"
              rel="noreferrer"
              className="source-card-link"
              onClick={(e) => e.stopPropagation()}
              title={source.url}
            >
              <ExternalLink size={14} />
            </a>
          )}
        </td>
        <td>
          <div className="source-actions">
            <input
              ref={fileInputRef}
              type="file"
              accept=".html,.htm,.txt,.pdf"
              onChange={handleUpload}
              hidden
            />
            <button
              className="btn btn-sm"
              onClick={(e) => {
                e.stopPropagation();
                fileInputRef.current?.click();
              }}
              disabled={uploading}
            >
              <Upload size={12} />
              {/* eslint-disable-next-line sonarjs/no-nested-conditional */}
              {uploading ? "Uploading..." : source.has_document ? "Replace" : "Upload"}
            </button>
            <button className="btn btn-sm btn-danger" onClick={handleDelete} title="Remove source">
              <Trash2 size={12} />
            </button>
          </div>
        </td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={6} className="case-detail-cell">
            <div className="detail-expand">
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
                    <a href={source.url} target="_blank" rel="noreferrer">
                      {source.url}
                    </a>
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
      )}
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

      {showAddForm && <AddSourceForm onClose={() => setShowAddForm(false)} />}

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
                <th style={{ width: 30 }}></th>
                <th>Source</th>
                <th>Type</th>
                <th>Status</th>
                <th style={{ width: 40 }}>Link</th>
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
