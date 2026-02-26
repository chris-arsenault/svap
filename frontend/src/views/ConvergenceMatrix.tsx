import React, { useState } from "react";
import { usePipeline } from "../data/usePipelineData";
import { formatDollars, scoreColor } from "../components/SharedUI";
import type { Case, Policy, Quality, ViewProps } from "../types";

function MatrixCell({ present, color }: { present: boolean; color: string }) {
  if (!present) {
    return <span style={{ color: "var(--border-default)", fontSize: 12 }}>{"\u00B7"}</span>;
  }
  return (
    <span
      style={{
        display: "inline-block",
        width: 20,
        height: 20,
        borderRadius: 3,
        background: `color-mix(in srgb, ${color} 20%, transparent)`,
        border: `1px solid ${color}`,
        lineHeight: "20px",
        fontSize: 10,
        fontWeight: 700,
        color,
      }}
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
      <td
        style={{
          fontWeight: 500,
          fontSize: 12,
          position: "sticky",
          left: 0,
          background: "var(--bg-card)",
          zIndex: 1,
          maxWidth: 200,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {name}
      </td>
      {!showPolicies && (
        <td style={{ fontFamily: "var(--font-mono)", fontSize: 11, whiteSpace: "nowrap" }}>
          {formatDollars("scale_dollars" in item ? item.scale_dollars : undefined)}
        </td>
      )}
      {qualities.map((q) => (
        <td key={q.quality_id} style={{ textAlign: "center", padding: "8px 4px" }}>
          <MatrixCell present={qs.includes(q.quality_id)} color={q.color} />
        </td>
      ))}
      <td
        style={{
          textAlign: "center",
          fontFamily: "var(--font-mono)",
          fontWeight: 700,
          fontSize: 14,
          color: scoreColor(score, threshold),
        }}
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
        <td
          style={{
            fontFamily: "var(--font-display)",
            fontSize: 10,
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: "0.06em",
            color: "var(--text-muted)",
            position: "sticky",
            left: 0,
            background: "var(--bg-card)",
            zIndex: 1,
          }}
        >
          Frequency
        </td>
        {!showPolicies && <td></td>}
        {qualities.map((q) => {
          const count = sorted.filter((item) => (item.qualities || []).includes(q.quality_id)).length;
          return (
            <td
              key={q.quality_id}
              style={{
                textAlign: "center",
                fontFamily: "var(--font-mono)",
                fontSize: 11,
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

      <div className="filter-bar stagger-in" style={{ marginBottom: "var(--sp-4)" }}>
        <button className={`btn ${!showPolicies ? "btn-accent" : ""}`} onClick={() => setShowPolicies(false)}>
          Cases ({cases.length})
        </button>
        <button className={`btn ${showPolicies ? "btn-accent" : ""}`} onClick={() => setShowPolicies(true)}>
          Policies ({policies.length})
        </button>
      </div>

      <div className="panel stagger-in">
        <div className="panel-body dense" style={{ overflowX: "auto" }}>
          <table className="data-table matrix-table">
            <thead>
              <tr>
                <th
                  style={{
                    minWidth: 200,
                    position: "sticky",
                    left: 0,
                    background: "var(--bg-card)",
                    zIndex: 2,
                  }}
                >
                  {showPolicies ? "Policy" : "Case"}
                </th>
                {!showPolicies && <th>Scale</th>}
                {qualities.map((q) => (
                  <th key={q.quality_id} style={{ textAlign: "center", minWidth: 44 }}>
                    <span style={{ color: q.color }}>{q.quality_id}</span>
                  </th>
                ))}
                <th style={{ textAlign: "center" }}>{"\u03A3"}</th>
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
