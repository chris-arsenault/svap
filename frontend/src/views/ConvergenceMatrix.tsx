import React, { useState } from "react";
import { usePipeline } from "../data/usePipelineData";
import { formatDollars, scoreColor } from "../utils";
import type { Case, Policy, Quality, ViewProps } from "../types";

function MatrixCell({ present, color }: { present: boolean; color: string }) {
  if (!present) {
    return <span className="matrix-cell-absent">{"\u00B7"}</span>;
  }
  return (
    <span
      className="matrix-cell-present"
      // eslint-disable-next-line local/no-inline-styles
      style={{ "--cell-color": color } as React.CSSProperties}
    >
      {"\u2713"}
    </span>
  );
}

function MatrixRow({
  item,
  qualities,
  showPolicies,
  threshold,
}: {
  item: Case | Policy;
  qualities: Quality[];
  showPolicies: boolean;
  threshold: number;
}) {
  const name = "case_name" in item ? item.case_name : item.name;
  const id = "case_id" in item ? item.case_id : item.policy_id;
  const qs = item.qualities || [];
  const score = "convergence_score" in item ? item.convergence_score : qs.length;

  return (
    <tr key={id}>
      <td className="matrix-sticky-name">
        {name}
      </td>
      {!showPolicies && (
        <td className="td-mono">
          {formatDollars("scale_dollars" in item ? item.scale_dollars : undefined)}
        </td>
      )}
      {qualities.map((q) => (
        <td key={q.quality_id} className="matrix-quality-cell">
          <MatrixCell present={qs.includes(q.quality_id)} color={q.color} />
        </td>
      ))}
      <td
        className="matrix-score-cell"
        // eslint-disable-next-line local/no-inline-styles
        style={{ color: scoreColor(score, threshold) }}
      >
        {score}
      </td>
    </tr>
  );
}

function MatrixFooter({
  qualities,
  sorted,
  showPolicies,
}: {
  qualities: Quality[];
  sorted: (Case | Policy)[];
  showPolicies: boolean;
}) {
  return (
    <tfoot>
      <tr>
        <td className="matrix-sticky-footer">
          Frequency
        </td>
        {!showPolicies && <td></td>}
        {qualities.map((q) => {
          const count = sorted.filter((item) => (item.qualities || []).includes(q.quality_id)).length;
          return (
            <td
              key={q.quality_id}
              className="matrix-footer-count"
              // eslint-disable-next-line local/no-inline-styles
              style={{
                color: count > sorted.length * 0.5 ? q.color : "var(--text-muted)",
              }}
            >
              {count}
            </td>
          );
        })}
        <td></td>
      </tr>
    </tfoot>
  );
}

export default function ConvergenceMatrix({ onNavigate: _onNavigate }: ViewProps) {
  const { cases, taxonomy, policies, threshold } = usePipeline();
  const [showPolicies, setShowPolicies] = useState(false);
  const data: (Case | Policy)[] = showPolicies ? policies : cases;
  const qualities = taxonomy;

  const sorted = [...data].sort((a, b) => {
    const aScore = "convergence_score" in a ? a.convergence_score : a.qualities.length;
    const bScore = "convergence_score" in b ? b.convergence_score : b.qualities.length;
    return bScore - aScore;
  });

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Convergence Matrix</h2>
        <div className="view-desc">
          {showPolicies ? "Policies" : "Cases"} scored against {qualities.length} vulnerability qualities — threshold:
          {"\u2265"}
          {threshold}
        </div>
      </div>

      <div className="filter-bar filter-bar-mb stagger-in">
        <button className={`btn ${!showPolicies ? "btn-accent" : ""}`} onClick={() => setShowPolicies(false)}>
          Cases ({cases.length})
        </button>
        <button className={`btn ${showPolicies ? "btn-accent" : ""}`} onClick={() => setShowPolicies(true)}>
          Policies ({policies.length})
        </button>
      </div>

      <div className="panel stagger-in">
        <div className="panel-body dense panel-body-scroll-x">
          <table className="data-table matrix-table">
            <thead>
              <tr>
                <th className="matrix-sticky-header">
                  {showPolicies ? "Policy" : "Case"}
                </th>
                {!showPolicies && <th>Scale</th>}
                {qualities.map((q) => (
                  <th key={q.quality_id} className="matrix-quality-header">
                    {/* eslint-disable-next-line local/no-inline-styles */}
                    <span style={{ color: q.color }}>{q.quality_id}</span>
                  </th>
                ))}
                <th className="text-center">{"\u03A3"}</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((item) => {
                const id = "case_id" in item ? item.case_id : item.policy_id;
                return (
                  <MatrixRow
                    key={id}
                    item={item}
                    qualities={qualities}
                    showPolicies={showPolicies}
                    threshold={threshold}
                  />
                );
              })}
            </tbody>
            <MatrixFooter qualities={qualities} sorted={sorted} showPolicies={showPolicies} />
          </table>
        </div>
      </div>
    </div>
  );
}
