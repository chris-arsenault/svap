/* eslint-disable react-refresh/only-export-components */
import React from "react";
import { usePipeline } from "../data/usePipelineData";
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

export function scoreLevel(score: number, threshold: number): string {
  if (score >= threshold + 2) return "critical";
  if (score >= threshold) return "high";
  if (score >= threshold - 1) return "medium";
  return "";
}

export function scoreColor(score: number, threshold: number): string {
  if (score >= threshold) return "var(--critical)";
  if (score >= threshold - 1) return "var(--high)";
  return "var(--text-secondary)";
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
      <span className="score-number" style={{ color: scoreColor(score, threshold) }}>
        {score}
      </span>
    </div>
  );
}

export function QualityTag({ id }: { id: string }) {
  const { qualityMap } = usePipeline();
  const q = qualityMap[id];
  if (!q)
    return (
      <span className="quality-tag" style={{ borderColor: "var(--border-default)", color: "var(--text-muted)" }}>
        {id}
      </span>
    );
  return (
    <span
      className="quality-tag"
      style={{
        borderColor: q.color,
        color: q.color,
        background: `color-mix(in srgb, ${q.color} 10%, transparent)`,
      }}
      title={`${q.name}: ${q.definition}`}
    >
      {id}
    </span>
  );
}

export function QualityTags({ ids }: { ids: string[] }) {
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 2 }}>
      {ids.map((id) => (
        <QualityTag key={id} id={id} />
      ))}
    </div>
  );
}

export function formatDollars(n?: number | null): string {
  if (n == null) return "\u2014";
  if (n >= 1e9) return `$${(n / 1e9).toFixed(1)}B`;
  if (n >= 1e6) return `$${(n / 1e6).toFixed(0)}M`;
  if (n >= 1e3) return `$${(n / 1e3).toFixed(0)}K`;
  return `$${n}`;
}

export function RiskBadge({ level }: { level: RiskLevel }) {
  const labels: Record<RiskLevel, string> = { critical: "CRITICAL", high: "HIGH", medium: "MEDIUM", low: "LOW" };
  return <Badge level={level}>{labels[level] || level}</Badge>;
}

export function StageDot({ status }: { status: StageStatus }) {
  return <span className={`stage-dot ${status}`} />;
}
