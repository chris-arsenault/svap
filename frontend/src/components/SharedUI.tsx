import React from "react";
import { useQuality } from "../data/usePipelineSelectors";
import { scoreLevel, scoreColor } from "../utils";
import type { RiskLevel, StageStatus } from "../types";

interface BadgeProps {
  level?: RiskLevel | "accent" | "neutral";
  children: React.ReactNode;
}

export function Badge({ level, children }: BadgeProps) {
  const cls = `badge badge-${level || "neutral"}`;
  return <span className={cls}>{children}</span>;
}

interface ScoreBarProps {
  score: number;
  max?: number;
  threshold?: number;
}

export function ScoreBar({ score, max = 8, threshold = 3 }: ScoreBarProps) {
  const level = scoreLevel(score, threshold);
  const pipClass = (i: number) => (i < score ? "score-pip filled " + level : "score-pip");
  return (
    <div className="score-bar-container">
      <div className="score-bar">
        {Array.from({ length: max }, (_, i) => (
          <div key={i} className={pipClass(i)} />
        ))}
      </div>
      {/* eslint-disable-next-line local/no-inline-styles */}
      <span className="score-number" style={{ color: scoreColor(score, threshold) }}>
        {score}
      </span>
    </div>
  );
}

export function QualityTag({ id }: { id: string }) {
  const q = useQuality(id);
  if (!q)
    return (
      <span className="quality-tag quality-tag-unknown">
        {id}
      </span>
    );
  return (
    <span
      className="quality-tag"
      // eslint-disable-next-line local/no-inline-styles
      style={{
        borderColor: q.color,
        color: q.color,
        background: `color-mix(in srgb, ${q.color} 10%, transparent)`,
      }}
      title={q.definition}
    >
      {q.name}
    </span>
  );
}

export function QualityTags({ ids }: { ids: string[] }) {
  return (
    <div className="quality-tags-list">
      {ids.map((id) => (
        <QualityTag key={id} id={id} />
      ))}
    </div>
  );
}

export function RiskBadge({ level }: { level: RiskLevel }) {
  const labels: Record<RiskLevel, string> = { critical: "CRITICAL", high: "HIGH", medium: "MEDIUM", low: "LOW" };
  return <Badge level={level}>{labels[level] || level}</Badge>;
}

export function StageDot({ status }: { status: StageStatus }) {
  return <span className={`stage-dot ${status}`} />;
}

export function ErrorBanner({ error, onDismiss }: { error: string | null; onDismiss?: () => void }) {
  if (!error) return null;
  return (
    <div className="error-banner panel stagger-in">
      <div className="panel-body">
        {error}
        {onDismiss && (
          <button className="btn btn-sm error-banner-dismiss" onClick={onDismiss}>
            Dismiss
          </button>
        )}
      </div>
    </div>
  );
}

export function ViewHeader({ title, description }: { title: string; description: React.ReactNode }) {
  return (
    <div className="view-header stagger-in">
      <h2>{title}</h2>
      <div className="view-desc">{description}</div>
    </div>
  );
}

export function MetricCard({ label, value, sub, className }: {
  label: string;
  value: React.ReactNode;
  sub?: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={`metric-card stagger-in${className ? ` ${className}` : ""}`}>
      <div className="metric-label">{label}</div>
      <div className="metric-value">{value}</div>
      {sub && <div className="metric-sub">{sub}</div>}
    </div>
  );
}
