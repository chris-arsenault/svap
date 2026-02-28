import React, { useCallback, useState } from "react";
import { ExternalLink, ChevronDown, ChevronRight } from "lucide-react";
import { useCases, useEnforcementSources } from "../data/usePipelineSelectors";
import { QualityTags } from "../components/SharedUI";
import { formatDollars } from "../utils";
import type { Case, EnforcementSource } from "../types";

function SourceRegistry({ sources }: { sources: EnforcementSource[] }) {
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Enforcement Sources</h3>
        <span className="panel-count">{sources.length} sources</span>
      </div>
      <div className="panel-body">
        <div className="source-grid">
          {sources.map((src) => (
            <div key={src.source_id} className="source-card">
              <div className="source-card-header">
                <div className="source-card-name">{src.name}</div>
                {src.url && (
                  <a href={src.url} target="_blank" rel="noreferrer" className="source-card-link">
                    <ExternalLink size={14} />
                  </a>
                )}
              </div>
              <div className="source-card-desc">{src.description}</div>
              <div className="source-card-badges">
                <span className="badge badge-neutral">{src.source_type.replace("_", " ")}</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function CaseRow({
  caseData,
  isExpanded,
  onToggleId,
}: {
  caseData: Case;
  isExpanded: boolean;
  onToggleId: (id: string) => void;
}) {
  return (
    <React.Fragment>
      <tr className="detail-row" onClick={() => onToggleId(caseData.case_id)}>
        <td className="case-row-toggle">
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </td>
        <td className="td-name">{caseData.case_name}</td>
        <td className="td-mono">
          {formatDollars(caseData.scale_dollars)}
        </td>
        <td className="hide-on-mobile case-row-detection">{caseData.detection_method}</td>
        <td>
          <QualityTags ids={caseData.qualities} />
        </td>
      </tr>
      {isExpanded && (
        <tr>
          <td colSpan={5} className="case-detail-cell">
            <div className="detail-expand">
              <div className="detail-label">Scheme Mechanics</div>
              <div>{caseData.scheme_mechanics}</div>
              <div className="detail-label">Exploited Policy</div>
              <div>{caseData.exploited_policy}</div>
              <div className="detail-label">Enabling Condition</div>
              <div className="case-detail-condition">{caseData.enabling_condition}</div>
            </div>
          </td>
        </tr>
      )}
    </React.Fragment>
  );
}

export default function CaseSourcing() {
  const cases = useCases();
  const enforcement_sources = useEnforcementSources();
  const [expandedCase, setExpandedCase] = useState<string | null>(null);
  const toggleCase = useCallback((id: string) => setExpandedCase((prev) => (prev === id ? null : id)), []);

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Case Sourcing</h2>
        <div className="view-desc">Enforcement cases in the corpus and sources for discovering new cases</div>
      </div>

      <SourceRegistry sources={enforcement_sources} />

      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Case Corpus</h3>
          <span className="panel-count">{cases.length} cases loaded</span>
        </div>
        <div className="panel-body dense">
          <table className="data-table">
            <thead>
              <tr>
                <th className="th-toggle"></th>
                <th>Case</th>
                <th>Scale</th>
                <th className="hide-on-mobile">Detection</th>
                <th>Qualities</th>
              </tr>
            </thead>
            <tbody>
              {cases.map((c) => (
                <CaseRow
                  key={c.case_id}
                  caseData={c}
                  isExpanded={expandedCase === c.case_id}
                  onToggleId={toggleCase}
                />
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
