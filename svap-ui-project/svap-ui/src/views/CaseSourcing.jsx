import React, { useState } from 'react';
import { ExternalLink, ChevronDown, ChevronRight } from 'lucide-react';
import { SEED_CASES } from '../data/seedCases';
import { ENFORCEMENT_SOURCES } from '../data/sources';
import { QualityTags, formatDollars } from '../components/SharedUI';

export default function CaseSourcing() {
  const [expandedCase, setExpandedCase] = useState(null);

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Case Sourcing</h2>
        <div className="view-desc">
          Enforcement cases in the corpus and sources for discovering new cases
        </div>
      </div>

      {/* Source registry */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Enforcement Sources</h3>
          <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{ENFORCEMENT_SOURCES.length} sources</span>
        </div>
        <div className="panel-body">
          <div className="source-grid">
            {ENFORCEMENT_SOURCES.map(src => (
              <div key={src.id} className="source-card">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 8 }}>
                  <div style={{ fontWeight: 600, fontSize: 13 }}>{src.name}</div>
                  <a href={src.url} target="_blank" rel="noreferrer" style={{ color: 'var(--accent)', flexShrink: 0 }}>
                    <ExternalLink size={14} />
                  </a>
                </div>
                <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 8 }}>{src.description}</div>
                <div style={{ display: 'flex', gap: 8 }}>
                  <span className="badge badge-neutral">{src.type.replace('_', ' ')}</span>
                  <span className="badge badge-neutral">{src.frequency}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Case corpus */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Case Corpus</h3>
          <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{SEED_CASES.length} cases loaded</span>
        </div>
        <div className="panel-body dense">
          <table className="data-table">
            <thead>
              <tr>
                <th style={{ width: 24 }}></th>
                <th>Case</th>
                <th>Scale</th>
                <th>Detection</th>
                <th>Qualities</th>
              </tr>
            </thead>
            <tbody>
              {SEED_CASES.map(c => (
                <React.Fragment key={c.case_id}>
                  <tr
                    className="detail-row"
                    onClick={() => setExpandedCase(expandedCase === c.case_id ? null : c.case_id)}
                  >
                    <td style={{ color: 'var(--text-muted)' }}>
                      {expandedCase === c.case_id ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </td>
                    <td style={{ fontWeight: 500 }}>{c.case_name}</td>
                    <td style={{ fontFamily: 'var(--font-mono)', whiteSpace: 'nowrap' }}>{formatDollars(c.scale_dollars)}</td>
                    <td style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{c.detection_method}</td>
                    <td><QualityTags ids={c.qualities} /></td>
                  </tr>
                  {expandedCase === c.case_id && (
                    <tr>
                      <td colSpan={5} style={{ padding: 0 }}>
                        <div className="detail-expand">
                          <div className="detail-label">Scheme Mechanics</div>
                          <div>{c.scheme_mechanics}</div>
                          <div className="detail-label">Exploited Policy</div>
                          <div>{c.exploited_policy}</div>
                          <div className="detail-label">Enabling Condition</div>
                          <div style={{ color: 'var(--high)' }}>{c.enabling_condition}</div>
                        </div>
                      </td>
                    </tr>
                  )}
                </React.Fragment>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
