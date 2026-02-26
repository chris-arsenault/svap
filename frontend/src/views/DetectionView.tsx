import React, { useCallback, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { usePipeline } from "../data/usePipelineData";
import { Badge } from "../components/SharedUI";
import type { DetectionPattern, RiskLevel, ViewProps } from "../types";

const PRIORITY_ORDER: RiskLevel[] = ["critical", "high", "medium", "low"];

function PatternDetail({ pat }: { pat: DetectionPattern }) {
  return (
    <div className="panel-body" style={{ borderTop: "1px solid var(--border-subtle)" }}>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--sp-6)" }}>
        <div>
          <div className="detail-label">Data Source</div>
          <div style={{ fontSize: 13, color: "var(--accent-bright)", fontFamily: "var(--font-mono)" }}>
            {pat.data_source}
          </div>
          <div className="detail-label">Baseline</div>
          <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>{pat.baseline}</div>
        </div>
        <div>
          <div className="detail-label">False Positive Risk</div>
          <div style={{ fontSize: 13, color: "var(--high)", lineHeight: 1.6 }}>{pat.false_positive_risk}</div>
          <div className="detail-label">Detection Latency</div>
          <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>{pat.detection_latency}</div>
        </div>
      </div>
    </div>
  );
}

function PatternCard({
  pat,
  isExpanded,
  onToggleId,
}: {
  pat: DetectionPattern;
  isExpanded: boolean;
  onToggleId: (id: string) => void;
}) {
  const handleToggle = () => onToggleId(pat.pattern_id);
  return (
    <div className="panel stagger-in" style={{ marginBottom: "var(--sp-4)" }}>
      <div
        className="panel-header"
        style={{ cursor: "pointer" }}
        role="button"
        tabIndex={0}
        onClick={handleToggle}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            handleToggle();
          }
        }}
      >
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12, flex: 1 }}>
          <div style={{ paddingTop: 2, color: "var(--text-muted)" }}>
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
              <Badge level={pat.priority}>{pat.priority}</Badge>
              <span style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--accent-bright)" }}>
                {pat.policy_name}
              </span>
            </div>
            <div
              style={{
                fontFamily: "var(--font-body)",
                fontSize: 13,
                textTransform: "none",
                letterSpacing: 0,
                fontWeight: 400,
                color: "var(--text-primary)",
                lineHeight: 1.4,
              }}
            >
              {pat.anomaly_signal}
            </div>
          </div>
        </div>
        <div
          style={{
            fontSize: 11,
            color: "var(--text-muted)",
            whiteSpace: "nowrap",
            flexShrink: 0,
            fontFamily: "var(--font-mono)",
          }}
        >
          {pat.detection_latency}
        </div>
      </div>

      {isExpanded && <PatternDetail pat={pat} />}
    </div>
  );
}

export default function DetectionView({ onNavigate: _onNavigate }: ViewProps) {
  const { detection_patterns } = usePipeline();
  const [expandedPattern, setExpandedPattern] = useState<string | null>(null);
  const [filterPriority, setFilterPriority] = useState<RiskLevel | null>(null);
  const togglePattern = useCallback(
    (id: string) => setExpandedPattern((prev) => (prev === id ? null : id)),
    []
  );

  const filtered = filterPriority
    ? detection_patterns.filter((p) => p.priority === filterPriority)
    : detection_patterns;

  const sorted = [...filtered].sort((a, b) => {
    return PRIORITY_ORDER.indexOf(a.priority) - PRIORITY_ORDER.indexOf(b.priority);
  });

  const counts: Partial<Record<RiskLevel, number>> = {};
  detection_patterns.forEach((p) => {
    counts[p.priority] = (counts[p.priority] || 0) + 1;
  });

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Detection Patterns</h2>
        <div className="view-desc">
          {detection_patterns.length} actionable anomaly signals derived from exploitation predictions
        </div>
      </div>

      <div style={{ display: "flex", gap: 8, marginBottom: "var(--sp-5)" }} className="stagger-in">
        <button className={`btn ${!filterPriority ? "btn-accent" : ""}`} onClick={() => setFilterPriority(null)}>
          All ({detection_patterns.length})
        </button>
        {PRIORITY_ORDER.map((p) =>
          counts[p] ? (
            <button
              key={p}
              className={`btn ${filterPriority === p ? "btn-accent" : ""}`}
              onClick={() => setFilterPriority(filterPriority === p ? null : p)}
            >
              {p} ({counts[p]})
            </button>
          ) : null
        )}
      </div>

      {sorted.map((pat) => (
        <PatternCard
          key={pat.pattern_id}
          pat={pat}
          isExpanded={expandedPattern === pat.pattern_id}
          onToggleId={togglePattern}
        />
      ))}
    </div>
  );
}
