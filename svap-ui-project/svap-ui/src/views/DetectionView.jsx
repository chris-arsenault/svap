import React, { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { SEED_DETECTION_PATTERNS } from '../data/predictions';
import { Badge } from '../components/SharedUI';

const PRIORITY_ORDER = ['critical', 'high', 'medium', 'low'];

export default function DetectionView() {
  const [expandedPattern, setExpandedPattern] = useState(null);
  const [filterPriority, setFilterPriority] = useState(null);

  const filtered = filterPriority
    ? SEED_DETECTION_PATTERNS.filter(p => p.priority === filterPriority)
    : SEED_DETECTION_PATTERNS;

  const sorted = [...filtered].sort((a, b) => {
    return PRIORITY_ORDER.indexOf(a.priority) - PRIORITY_ORDER.indexOf(b.priority);
  });

  const counts = {};
  SEED_DETECTION_PATTERNS.forEach(p => { counts[p.priority] = (counts[p.priority] || 0) + 1; });

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Detection Patterns</h2>
        <div className="view-desc">
          {SEED_DETECTION_PATTERNS.length} actionable anomaly signals derived from exploitation predictions
        </div>
      </div>

      {/* Priority filters */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 'var(--sp-5)' }} className="stagger-in">
        <button
          className={`btn ${!filterPriority ? 'btn-accent' : ''}`}
          onClick={() => setFilterPriority(null)}
        >
          All ({SEED_DETECTION_PATTERNS.length})
        </button>
        {PRIORITY_ORDER.map(p => counts[p] ? (
          <button
            key={p}
            className={`btn ${filterPriority === p ? 'btn-accent' : ''}`}
            onClick={() => setFilterPriority(filterPriority === p ? null : p)}
          >
            {p} ({counts[p]})
          </button>
        ) : null)}
      </div>

      {/* Pattern cards */}
      {sorted.map(pat => {
        const isExpanded = expandedPattern === pat.id;

        return (
          <div key={pat.id} className="panel stagger-in" style={{ marginBottom: 'var(--sp-4)' }}>
            <div
              className="panel-header"
              style={{ cursor: 'pointer' }}
              onClick={() => setExpandedPattern(isExpanded ? null : pat.id)}
            >
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12, flex: 1 }}>
                <div style={{ paddingTop: 2, color: 'var(--text-muted)' }}>
                  {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </div>
                <div style={{ flex: 1 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                    <Badge level={pat.priority}>{pat.priority}</Badge>
                    <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--accent-bright)' }}>
                      {pat.policy_name}
                    </span>
                  </div>
                  <div style={{
                    fontFamily: 'var(--font-body)', fontSize: 13,
                    textTransform: 'none', letterSpacing: 0, fontWeight: 400,
                    color: 'var(--text-primary)', lineHeight: 1.4,
                  }}>
                    {pat.anomaly_signal}
                  </div>
                </div>
              </div>
              <div style={{
                fontSize: 11, color: 'var(--text-muted)', whiteSpace: 'nowrap', flexShrink: 0,
                fontFamily: 'var(--font-mono)',
              }}>
                {pat.detection_latency}
              </div>
            </div>

            {isExpanded && (
              <div className="panel-body" style={{ borderTop: '1px solid var(--border-subtle)' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 'var(--sp-6)' }}>
                  <div>
                    <div className="detail-label">Data Source</div>
                    <div style={{ fontSize: 13, color: 'var(--accent-bright)', fontFamily: 'var(--font-mono)' }}>
                      {pat.data_source}
                    </div>

                    <div className="detail-label">Baseline</div>
                    <div style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.6 }}>
                      {pat.baseline}
                    </div>
                  </div>
                  <div>
                    <div className="detail-label">False Positive Risk</div>
                    <div style={{ fontSize: 13, color: 'var(--high)', lineHeight: 1.6 }}>
                      {pat.false_positive_risk}
                    </div>

                    <div className="detail-label">Detection Latency</div>
                    <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
                      {pat.detection_latency}
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
