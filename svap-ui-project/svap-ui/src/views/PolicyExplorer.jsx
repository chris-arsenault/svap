import React, { useState } from 'react';
import { POLICY_CATALOG, SCANNED_PROGRAMS } from '../data/policyCatalog';
import { SEED_POLICIES, CONVERGENCE_THRESHOLD } from '../data/seedPolicies';
import { HHS_DATA_SOURCES } from '../data/sources';
import { ScoreBar, QualityTags, RiskBadge } from '../components/SharedUI';

function TreeNode({ nodeKey, node, depth = 0 }) {
  const [expanded, setExpanded] = useState(depth < 2);
  const hasChildren = node.children;
  const hasPrograms = node.programs;

  return (
    <div className={`tree-node ${depth === 0 ? 'root' : ''}`}>
      <div className="tree-label" onClick={() => setExpanded(!expanded)}>
        <span className="tree-icon">{hasChildren ? (expanded ? '▾' : '▸') : hasPrograms ? (expanded ? '▾' : '▸') : '·'}</span>
        <span style={{ fontWeight: depth < 2 ? 600 : 400, color: depth === 0 ? 'var(--text-primary)' : 'var(--text-secondary)' }}>
          {node.label}
        </span>
        {hasPrograms && (
          <span style={{ fontSize: 10, color: 'var(--text-muted)', marginLeft: 'auto' }}>
            {node.programs.length} programs
          </span>
        )}
      </div>
      {expanded && hasChildren && Object.entries(node.children).map(([k, child]) => (
        <TreeNode key={k} nodeKey={k} node={child} depth={depth + 1} />
      ))}
      {expanded && hasPrograms && node.programs.map(prog => {
        const isScanned = SCANNED_PROGRAMS.includes(prog);
        return (
          <div key={prog} className={`tree-leaf ${isScanned ? 'scanned' : ''}`}>
            {isScanned && <span style={{ marginRight: 4 }}>◆</span>}
            {prog}
          </div>
        );
      })}
    </div>
  );
}

export default function PolicyExplorer() {
  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Policy Explorer</h2>
        <div className="view-desc">
          HHS policy catalog — <span style={{ color: 'var(--accent-bright)' }}>◆ scanned policies</span> have been evaluated against the vulnerability taxonomy
        </div>
      </div>

      <div className="split-view">
        {/* Policy tree */}
        <div className="panel stagger-in">
          <div className="panel-header">
            <h3>HHS Policy Catalog</h3>
            <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>
              {SCANNED_PROGRAMS.length} scanned
            </span>
          </div>
          <div className="panel-body" style={{ maxHeight: 600, overflowY: 'auto' }}>
            {Object.entries(POLICY_CATALOG).map(([k, node]) => (
              <TreeNode key={k} nodeKey={k} node={node} depth={0} />
            ))}
          </div>
        </div>

        {/* Scan results summary */}
        <div>
          <div className="panel stagger-in">
            <div className="panel-header">
              <h3>Scan Results</h3>
            </div>
            <div className="panel-body dense">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Policy</th>
                    <th>Score</th>
                    <th>Risk</th>
                    <th>Qualities</th>
                  </tr>
                </thead>
                <tbody>
                  {[...SEED_POLICIES]
                    .sort((a, b) => b.convergence_score - a.convergence_score)
                    .map(p => (
                      <tr key={p.policy_id}>
                        <td style={{ fontWeight: 500 }}>{p.name}</td>
                        <td><ScoreBar score={p.convergence_score} threshold={CONVERGENCE_THRESHOLD} /></td>
                        <td><RiskBadge level={p.risk_level} /></td>
                        <td><QualityTags ids={p.qualities} /></td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Data sources */}
          <div className="panel stagger-in">
            <div className="panel-header"><h3>Available Data Sources</h3></div>
            <div className="panel-body">
              {Object.entries(HHS_DATA_SOURCES).map(([catKey, cat]) => (
                <div key={catKey} style={{ marginBottom: 16 }}>
                  <div style={{ fontFamily: 'var(--font-display)', fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-muted)', marginBottom: 6 }}>
                    {cat.label}
                  </div>
                  {cat.sources.map(s => (
                    <div key={s.id} style={{ fontSize: 12, padding: '4px 0', color: 'var(--text-secondary)' }}>
                      <span style={{ color: 'var(--accent-bright)', fontFamily: 'var(--font-mono)', fontSize: 11 }}>{s.id}</span>
                      {' — '}{s.name}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
