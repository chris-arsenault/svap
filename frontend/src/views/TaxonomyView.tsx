import React, { useCallback, useState } from "react";
import { usePipeline } from "../data/usePipelineData";
import type { Case, Quality, ViewProps } from "../types";

function QualityCard({
  quality,
  isSelected,
  onSelectId,
}: {
  quality: Quality;
  isSelected: boolean;
  onSelectId: (id: string) => void;
}) {
  return (
    <div
      className="stagger-in"
      role="button"
      tabIndex={0}
      onClick={() => onSelectId(quality.quality_id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelectId(quality.quality_id);
        }
      }}
      style={{
        background: isSelected ? "var(--bg-elevated)" : "var(--bg-card)",
        border: `1px solid ${isSelected ? quality.color : "var(--border-subtle)"}`,
        borderRadius: "var(--radius-lg)",
        padding: "var(--sp-5)",
        cursor: "pointer",
        transition: "all 0.15s",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            fontWeight: 700,
            color: quality.color,
            background: `color-mix(in srgb, ${quality.color} 15%, transparent)`,
            padding: "2px 8px",
            borderRadius: 3,
          }}
        >
          {quality.quality_id}
        </span>
        <span
          style={{
            fontSize: 11,
            color: "var(--text-muted)",
            marginLeft: "auto",
            fontFamily: "var(--font-mono)",
          }}
        >
          {quality.case_count} cases
        </span>
      </div>
      <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 6 }}>{quality.name}</div>
      <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>{quality.definition}</div>
    </div>
  );
}

function QualityDetail({ quality, matchingCases }: { quality: Quality; matchingCases: Case[] }) {
  return (
    <div className="panel" style={{ borderColor: quality.color }}>
      <div className="panel-header" style={{ borderBottomColor: quality.color }}>
        <h3 style={{ color: quality.color }}>
          {quality.quality_id} — {quality.name}
        </h3>
      </div>
      <div className="panel-body">
        <div className="detail-grid">
          <div>
            <div className="detail-label">Recognition Test</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.7 }}>
              {quality.recognition_test}
            </div>
            <div className="detail-label">Exploitation Logic</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.7 }}>
              {quality.exploitation_logic}
            </div>
          </div>
          <div>
            <div className="detail-label">Cases Exhibiting This Quality</div>
            {matchingCases.map((c) => (
              <div
                key={c.case_id}
                style={{
                  padding: "8px 12px",
                  marginBottom: 4,
                  background: "var(--bg-elevated)",
                  borderRadius: "var(--radius-sm)",
                  fontSize: 12,
                }}
              >
                <div style={{ fontWeight: 500, color: "var(--text-primary)" }}>{c.case_name}</div>
                <div style={{ color: "var(--text-muted)", marginTop: 2 }}>{c.enabling_condition}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default function TaxonomyView({ onNavigate: _onNavigate }: ViewProps) {
  const { taxonomy, cases } = usePipeline();
  const [selectedQuality, setSelectedQuality] = useState<string | null>(null);
  const toggleQuality = useCallback((id: string) => setSelectedQuality((prev) => (prev === id ? null : id)), []);

  const selectedData = selectedQuality ? taxonomy.find((q) => q.quality_id === selectedQuality) : null;
  const matchingCases = selectedQuality ? cases.filter((c) => c.qualities.includes(selectedQuality)) : [];

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Vulnerability Taxonomy</h2>
        <div className="view-desc">
          {taxonomy.length} structural qualities extracted from enforcement cases — click any quality for details
        </div>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
          gap: "var(--sp-4)",
          marginBottom: "var(--sp-6)",
        }}
      >
        {taxonomy.map((q) => (
          <QualityCard
            key={q.quality_id}
            quality={q}
            isSelected={selectedQuality === q.quality_id}
            onSelectId={toggleQuality}
          />
        ))}
      </div>

      {selectedData && <QualityDetail quality={selectedData} matchingCases={matchingCases} />}
    </div>
  );
}
