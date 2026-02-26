import React from 'react';
import { SEED_CASES } from '../data/seedCases';
import { SEED_TAXONOMY } from '../data/seedTaxonomy';
import { SEED_POLICIES, CONVERGENCE_THRESHOLD } from '../data/seedPolicies';
import { SEED_PREDICTIONS, SEED_DETECTION_PATTERNS } from '../data/predictions';
import { ScoreBar, QualityTags, formatDollars, RiskBadge } from '../components/SharedUI';

export default function Dashboard({ onNavigate }) {
  const criticalPolicies = SEED_POLICIES.filter(p => p.risk_level === 'critical');
  const totalFraudDollars = SEED_CASES.reduce((sum, c) => sum + (c.scale_dollars || 0), 0);
  const criticalPatterns = SEED_DETECTION_PATTERNS.filter(p => p.priority === 'critical');

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Structural Vulnerability Analysis</h2>
        <div className="view-desc">
          HHS OIG fraud detection pipeline — convergence threshold: ≥{CONVERGENCE_THRESHOLD} qualities = high exploitation risk
        </div>
      </div>

      {/* Key metrics */}
      <div className="metrics-row">
        <div className="metric-card stagger-in">
          <div className="metric-label">Enforcement Cases</div>
          <div className="metric-value">{SEED_CASES.length}</div>
          <div className="metric-sub">{formatDollars(totalFraudDollars)} total intended losses</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Vulnerability Qualities</div>
          <div className="metric-value">{SEED_TAXONOMY.length}</div>
          <div className="metric-sub">Threshold: ≥{CONVERGENCE_THRESHOLD} = high risk</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Policies Scanned</div>
          <div className="metric-value">{SEED_POLICIES.length}</div>
          <div className="metric-sub" style={{ color: 'var(--critical)' }}>
            {criticalPolicies.length} critical risk
          </div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Detection Patterns</div>
          <div className="metric-value">{SEED_DETECTION_PATTERNS.length}</div>
          <div className="metric-sub" style={{ color: 'var(--critical)' }}>
            {criticalPatterns.length} critical priority
          </div>
        </div>
      </div>

      <div className="split-view">
        {/* Highest-risk policies */}
        <div className="panel stagger-in">
          <div className="panel-header">
            <h3>Highest-Risk Policies</h3>
            <button className="btn" onClick={() => onNavigate('predictions')}>View predictions →</button>
          </div>
          <div className="panel-body dense">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Policy</th>
                  <th>Score</th>
                  <th>Qualities</th>
                  <th>Risk</th>
                </tr>
              </thead>
              <tbody>
                {SEED_POLICIES.filter(p => p.convergence_score >= CONVERGENCE_THRESHOLD)
                  .sort((a, b) => b.convergence_score - a.convergence_score)
                  .map(p => (
                    <tr key={p.policy_id}>
                      <td style={{ fontWeight: 500 }}>{p.name}</td>
                      <td><ScoreBar score={p.convergence_score} threshold={CONVERGENCE_THRESHOLD} /></td>
                      <td><QualityTags ids={p.qualities} /></td>
                      <td><RiskBadge level={p.risk_level} /></td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        </div>

        {/* Top cases by scale */}
        <div className="panel stagger-in">
          <div className="panel-header">
            <h3>Largest Enforcement Cases</h3>
            <button className="btn" onClick={() => onNavigate('cases')}>All cases →</button>
          </div>
          <div className="panel-body dense">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Case</th>
                  <th>Scale</th>
                  <th>Qualities</th>
                </tr>
              </thead>
              <tbody>
                {[...SEED_CASES]
                  .sort((a, b) => (b.scale_dollars || 0) - (a.scale_dollars || 0))
                  .slice(0, 5)
                  .map(c => (
                    <tr key={c.case_id}>
                      <td style={{ fontWeight: 500, maxWidth: 240 }}>{c.case_name}</td>
                      <td style={{ fontFamily: 'var(--font-mono)', whiteSpace: 'nowrap' }}>
                        {formatDollars(c.scale_dollars)}
                      </td>
                      <td><QualityTags ids={c.qualities} /></td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      {/* Key insight */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Key Calibration Finding</h3>
        </div>
        <div className="panel-body" style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.7 }}>
          <strong style={{ color: 'var(--text-primary)' }}>Every enforcement case exceeding $500M in intended losses scored ≥3 vulnerability qualities.</strong>{' '}
          The most common qualities in large-scale schemes are V1 (Payment Precedes Verification) and V6 (Expansion Outpaces Oversight), 
          appearing in 75%+ of major cases. The V1+V2 combination (pay-before-verify + self-attesting payment basis) appeared 
          together in every $1B+ scheme. Three policies currently score ≥5: HCBS, PACE, and Hospital-at-Home — all share 
          the characteristic of services delivered outside institutional settings with minimal verification infrastructure.
        </div>
      </div>
    </div>
  );
}
