import React, { useCallback, useState } from "react";
import { useShallow } from "zustand/shallow";
import { usePipelineStore } from "../data/pipelineStore";
import type { Case, Quality } from "../types";

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
      className={`stagger-in quality-card ${isSelected ? "selected" : ""}`}
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
      style={{ "--q-color": quality.color } as React.CSSProperties}
    >
      <div className="quality-card-header">
        <span className="quality-card-id">
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
    <div
      className="panel quality-detail-panel"
      // eslint-disable-next-line local/no-inline-styles
      style={{ "--q-color": quality.color } as React.CSSProperties}
    >
      <div className="panel-header">
        <h3>
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

export default function TaxonomyView() {
  const { taxonomy, cases } = usePipelineStore(
    useShallow((s) => ({
      taxonomy: s.taxonomy,
      cases: s.cases,
    })),
  );
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
