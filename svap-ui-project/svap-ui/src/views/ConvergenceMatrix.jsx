import React, { useState } from 'react';
import { SEED_CASES } from '../data/seedCases';
import { SEED_TAXONOMY } from '../data/seedTaxonomy';
import { SEED_POLICIES, CONVERGENCE_THRESHOLD } from '../data/seedPolicies';
import { formatDollars } from '../components/SharedUI';

export default function ConvergenceMatrix() {
  const [showPolicies, setShowPolicies] = useState(false);
  const data = showPolicies ? SEED_POLICIES : SEED_CASES;
  const qualities = SEED_TAXONOMY;

  // Sort by convergence score descending
  const sorted = [...data].sort((a, b) => {
    const aScore = a.convergence_score ?? a.qualities.length;
    const bScore = b.convergence_score ?? b.qualities.length;
    return bScore - aScore;
  });

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Convergence Matrix</h2>
        <div className="view-desc">
          {showPolicies ? 'Policies' : 'Cases'} scored against {qualities.length} vulnerability qualities — 
          threshold: ≥{CONVERGENCE_THRESHOLD}
        </div>
      </div>

      {/* Toggle */}
      <div style={{ marginBottom: 'var(--sp-4)', display: 'flex', gap: 8 }} className="stagger-in">
        <button className={`btn ${!showPolicies ? 'btn-accent' : ''}`} onClick={() => setShowPolicies(false)}>
          Cases ({SEED_CASES.length})
        </button>
        <button className={`btn ${showPolicies ? 'btn-accent' : ''}`} onClick={() => setShowPolicies(true)}>
          Policies ({SEED_POLICIES.length})
        </button>
      </div>

      {/* Matrix */}
      <div className="panel stagger-in">
        <div className="panel-body dense" style={{ overflowX: 'auto' }}>
          <table className="data-table" style={{ minWidth: 900 }}>
            <thead>
              <tr>
                <th style={{ minWidth: 200, position: 'sticky', left: 0, background: 'var(--bg-card)', zIndex: 2 }}>
                  {showPolicies ? 'Policy' : 'Case'}
                </th>
                {!showPolicies && <th>Scale</th>}
                {qualities.map(q => (
                  <th key={q.quality_id} style={{ textAlign: 'center', minWidth: 44 }}>
                    <span style={{ color: q.color }}>{q.quality_id}</span>
                  </th>
                ))}
                <th style={{ textAlign: 'center' }}>Σ</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((item) => {
                const name = item.case_name || item.name;
                const id = item.case_id || item.policy_id;
                const qs = item.qualities || [];
                const score = item.convergence_score ?? qs.length;
                const isAbove = score >= CONVERGENCE_THRESHOLD;

                return (
                  <tr key={id}>
                    <td style={{
                      fontWeight: 500, fontSize: 12,
                      position: 'sticky', left: 0,
                      background: 'var(--bg-card)', zIndex: 1,
                      maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                    }}>
                      {name}
                    </td>
                    {!showPolicies && (
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11, whiteSpace: 'nowrap' }}>
                        {formatDollars(item.scale_dollars)}
                      </td>
                    )}
                    {qualities.map(q => {
                      const present = qs.includes(q.quality_id);
                      return (
                        <td key={q.quality_id} style={{ textAlign: 'center', padding: '8px 4px' }}>
                          {present ? (
                            <span style={{
                              display: 'inline-block', width: 20, height: 20,
                              borderRadius: 3,
                              background: `color-mix(in srgb, ${q.color} 20%, transparent)`,
                              border: `1px solid ${q.color}`,
                              lineHeight: '20px', fontSize: 10, fontWeight: 700,
                              color: q.color,
                            }}>
                              ✓
                            </span>
                          ) : (
                            <span style={{ color: 'var(--border-default)', fontSize: 12 }}>·</span>
                          )}
                        </td>
                      );
                    })}
                    <td style={{
                      textAlign: 'center',
                      fontFamily: 'var(--font-mono)',
                      fontWeight: 700,
                      fontSize: 14,
                      color: isAbove ? 'var(--critical)' : score >= CONVERGENCE_THRESHOLD - 1 ? 'var(--high)' : 'var(--text-muted)',
                    }}>
                      {score}
                    </td>
                  </tr>
                );
              })}
            </tbody>
            {/* Quality frequency footer */}
            <tfoot>
              <tr>
                <td style={{
                  fontFamily: 'var(--font-display)', fontSize: 10, fontWeight: 600,
                  textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-muted)',
                  position: 'sticky', left: 0, background: 'var(--bg-card)', zIndex: 1,
                }}>
                  Frequency
                </td>
                {!showPolicies && <td></td>}
                {qualities.map(q => {
                  const count = sorted.filter(item => (item.qualities || []).includes(q.quality_id)).length;
                  return (
                    <td key={q.quality_id} style={{
                      textAlign: 'center',
                      fontFamily: 'var(--font-mono)', fontSize: 11,
                      color: count > sorted.length * 0.5 ? q.color : 'var(--text-muted)',
                    }}>
                      {count}
                    </td>
                  );
                })}
                <td></td>
              </tr>
            </tfoot>
          </table>
        </div>
      </div>
    </div>
  );
}
