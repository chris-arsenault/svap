import React from 'react';
import { SEED_TAXONOMY } from '../data/seedTaxonomy';

const QUALITY_MAP = {};
SEED_TAXONOMY.forEach(q => { QUALITY_MAP[q.quality_id] = q; });

export function Badge({ level, children }) {
  const cls = `badge badge-${level || 'neutral'}`;
  return <span className={cls}>{children}</span>;
}

export function ScoreBar({ score, max = 8, threshold = 3 }) {
  const level = score >= threshold + 2 ? 'critical' : score >= threshold ? 'high' : score >= threshold - 1 ? 'medium' : '';
  return (
    <div className="score-bar-container">
      <div className="score-bar">
        {Array.from({ length: max }, (_, i) => (
          <div key={i} className={`score-pip ${i < score ? `filled ${level}` : ''}`} />
        ))}
      </div>
      <span className="score-number" style={{ color: score >= threshold ? 'var(--critical)' : score >= threshold - 1 ? 'var(--high)' : 'var(--text-secondary)' }}>
        {score}
      </span>
    </div>
  );
}

export function QualityTag({ id }) {
  const q = QUALITY_MAP[id];
  if (!q) return <span className="quality-tag" style={{ borderColor: 'var(--border-default)', color: 'var(--text-muted)' }}>{id}</span>;
  return (
    <span
      className="quality-tag"
      style={{ borderColor: q.color, color: q.color, background: `color-mix(in srgb, ${q.color} 10%, transparent)` }}
      title={`${q.name}: ${q.definition}`}
    >
      {id}
    </span>
  );
}

export function QualityTags({ ids }) {
  return <div style={{ display: 'flex', flexWrap: 'wrap', gap: 2 }}>{ids.map(id => <QualityTag key={id} id={id} />)}</div>;
}

export function formatDollars(n) {
  if (n == null) return '—';
  if (n >= 1e9) return `$${(n / 1e9).toFixed(1)}B`;
  if (n >= 1e6) return `$${(n / 1e6).toFixed(0)}M`;
  if (n >= 1e3) return `$${(n / 1e3).toFixed(0)}K`;
  return `$${n}`;
}

export function RiskBadge({ level }) {
  const labels = { critical: 'CRITICAL', high: 'HIGH', medium: 'MEDIUM', low: 'LOW' };
  return <Badge level={level}>{labels[level] || level}</Badge>;
}

export function StageDot({ status }) {
  return <span className={`stage-dot ${status}`} />;
}
