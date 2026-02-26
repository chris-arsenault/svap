import React, { useState } from 'react';
import { SEED_TAXONOMY } from '../data/seedTaxonomy';
import { SEED_CASES } from '../data/seedCases';

export default function TaxonomyView() {
  const [selectedQuality, setSelectedQuality] = useState(null);

  const selectedData = selectedQuality ? SEED_TAXONOMY.find(q => q.quality_id === selectedQuality) : null;
  const matchingCases = selectedQuality
    ? SEED_CASES.filter(c => c.qualities.includes(selectedQuality))
    : [];

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Vulnerability Taxonomy</h2>
        <div className="view-desc">
          {SEED_TAXONOMY.length} structural qualities extracted from enforcement cases — click any quality for details
        </div>
      </div>

      {/* Quality grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 'var(--sp-4)', marginBottom: 'var(--sp-6)' }}>
        {SEED_TAXONOMY.map(q => (
          <div
            key={q.quality_id}
            className="stagger-in"
            onClick={() => setSelectedQuality(selectedQuality === q.quality_id ? null : q.quality_id)}
            style={{
              background: selectedQuality === q.quality_id ? 'var(--bg-elevated)' : 'var(--bg-card)',
              border: `1px solid ${selectedQuality === q.quality_id ? q.color : 'var(--border-subtle)'}`,
              borderRadius: 'var(--radius-lg)',
              padding: 'var(--sp-5)',
              cursor: 'pointer',
              transition: 'all 0.15s',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <span style={{
                fontFamily: 'var(--font-mono)', fontSize: 12, fontWeight: 700,
                color: q.color, background: `color-mix(in srgb, ${q.color} 15%, transparent)`,
                padding: '2px 8px', borderRadius: 3,
              }}>
                {q.quality_id}
              </span>
              <span style={{ fontSize: 11, color: 'var(--text-muted)', marginLeft: 'auto', fontFamily: 'var(--font-mono)' }}>
                {q.case_count} cases
              </span>
            </div>
            <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 6 }}>{q.name}</div>
            <div style={{ fontSize: 12, color: 'var(--text-secondary)', lineHeight: 1.5 }}>{q.definition}</div>
          </div>
        ))}
      </div>

      {/* Detail panel */}
      {selectedData && (
        <div className="panel" style={{ borderColor: selectedData.color }}>
          <div className="panel-header" style={{ borderBottomColor: selectedData.color }}>
            <h3 style={{ color: selectedData.color }}>
              {selectedData.quality_id} — {selectedData.name}
            </h3>
          </div>
          <div className="panel-body">
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 'var(--sp-6)' }}>
              <div>
                <div className="detail-label">Recognition Test</div>
                <div style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.7 }}>
                  {selectedData.recognition_test}
                </div>

                <div className="detail-label">Exploitation Logic</div>
                <div style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.7 }}>
                  {selectedData.exploitation_logic}
                </div>
              </div>

              <div>
                <div className="detail-label">Cases Exhibiting This Quality</div>
                {matchingCases.map(c => (
                  <div
                    key={c.case_id}
                    style={{
                      padding: '8px 12px', marginBottom: 4,
                      background: 'var(--bg-elevated)', borderRadius: 'var(--radius-sm)',
                      fontSize: 12,
                    }}
                  >
                    <div style={{ fontWeight: 500, color: 'var(--text-primary)' }}>{c.case_name}</div>
                    <div style={{ color: 'var(--text-muted)', marginTop: 2 }}>{c.enabling_condition}</div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
