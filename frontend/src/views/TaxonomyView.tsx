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
      className="stagger-in quality-card"
      role="button"
      tabIndex={0}
      onClick={() => onSelectId(quality.quality_id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelectId(quality.quality_id);
        }
      }}
      // eslint-disable-next-line local/no-inline-styles
      style={{
        background: isSelected ? "var(--bg-elevated)" : "var(--bg-card)",
        border: `1px solid ${isSelected ? quality.color : "var(--border-subtle)"}`,
      }}
    >
      <div className="quality-card-header">
        <span
          className="quality-card-id"
          // eslint-disable-next-line local/no-inline-styles
          style={{
            color: quality.color,
            background: `color-mix(in srgb, ${quality.color} 15%, transparent)`,
          }}
        >
          {quality.quality_id}
        </span>
        <span className="quality-card-count">
          {quality.case_count} cases
        </span>
      </div>
      <div className="quality-card-name">{quality.name}</div>
      <div className="quality-card-def">{quality.definition}</div>
    </div>
  );
}

function QualityDetail({ quality, matchingCases }: { quality: Quality; matchingCases: Case[] }) {
  return (
    // eslint-disable-next-line local/no-inline-styles
    <div className="panel" style={{ borderColor: quality.color }}>
      {/* eslint-disable-next-line local/no-inline-styles */}
      <div className="panel-header" style={{ borderBottomColor: quality.color }}>
        {/* eslint-disable-next-line local/no-inline-styles */}
        <h3 style={{ color: quality.color }}>
          {quality.quality_id} — {quality.name}
        </h3>
      </div>
      <div className="panel-body">
        <div className="detail-grid">
          <div>
            <div className="detail-label">Recognition Test</div>
            <div className="detail-text">
              {quality.recognition_test}
            </div>
            <div className="detail-label">Exploitation Logic</div>
            <div className="detail-text">
              {quality.exploitation_logic}
            </div>
          </div>
          <div>
            <div className="detail-label">Cases Exhibiting This Quality</div>
            {matchingCases.map((c) => (
              <div key={c.case_id} className="quality-case-item">
                <div className="quality-case-item-name">{c.case_name}</div>
                <div className="quality-case-item-condition">{c.enabling_condition}</div>
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

      <div className="quality-grid">
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
