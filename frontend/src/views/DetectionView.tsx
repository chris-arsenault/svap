import React, { useCallback, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { usePipelineStore } from "../data/pipelineStore";
import { Badge } from "../components/SharedUI";
import type { DetectionPattern, RiskLevel } from "../types";

const PRIORITY_ORDER: RiskLevel[] = ["critical", "high", "medium", "low"];

function PatternDetail({ pat }: { pat: DetectionPattern }) {
  return (
    <div className="panel-body panel-body-bordered">
      <div className="detail-grid">
        <div>
          <div className="detail-label">Data Source</div>
          <div className="pattern-data-source">
            {pat.data_source}
          </div>
          <div className="detail-label">Baseline</div>
          <div className="pattern-baseline">{pat.baseline}</div>
        </div>
        <div>
          <div className="detail-label">False Positive Risk</div>
          <div className="pattern-false-positive">{pat.false_positive_risk}</div>
          <div className="detail-label">Detection Latency</div>
          <div className="pattern-latency">{pat.detection_latency}</div>
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
    <div className="panel stagger-in pattern-card">
      <div
        className="panel-header clickable"
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
        <div className="pattern-header-left">
          <div className="pattern-chevron">
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </div>
          <div className="pattern-header-content">
            <div className="pattern-badge-row">
              <Badge level={pat.priority}>{pat.priority}</Badge>
              <span className="pattern-policy-name">
                {pat.policy_name}
              </span>
              {pat.step_title && (
                <span className="pattern-step-title">
                  {"\u2014"} {pat.step_title}
                </span>
              )}
            </div>
            <div className="pattern-anomaly-signal">
              {pat.anomaly_signal}
            </div>
          </div>
        </div>
        <div className="pattern-detection-latency">
          {pat.detection_latency}
        </div>
      </div>

      {isExpanded && <PatternDetail pat={pat} />}
    </div>
  );
}

export default function DetectionView() {
  const detection_patterns = usePipelineStore((s) => s.detection_patterns);
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
          {detection_patterns.length} actionable anomaly signals derived from exploitation steps
        </div>
      </div>

      <div className="filter-bar filter-bar-mb stagger-in">
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
