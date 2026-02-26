import React, { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { SEED_PREDICTIONS } from '../data/predictions';
import { SEED_POLICIES } from '../data/seedPolicies';
import { QualityTags, ScoreBar, Badge } from '../components/SharedUI';

export default function PredictionView() {
  const [expandedPred, setExpandedPred] = useState(null);

  const difficultyLevel = (d) => {
    if (!d) return 'neutral';
    const lower = d.toLowerCase();
    if (lower.includes('hard')) return 'critical';
    if (lower.includes('medium')) return 'high';
    return 'medium';
  };

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Exploitation Predictions</h2>
        <div className="view-desc">
          Structurally-entailed predictions for {SEED_PREDICTIONS.length} high-risk policies — 
          every prediction cites specific enabling qualities
        </div>
      </div>

      {SEED_PREDICTIONS.map((pred) => {
        const isExpanded = expandedPred === pred.id;
        const policy = SEED_POLICIES.find(p => p.policy_id === pred.policy_id);

        return (
          <div key={pred.id} className="panel stagger-in">
            <div
              className="panel-header"
              style={{ cursor: 'pointer' }}
              onClick={() => setExpandedPred(isExpanded ? null : pred.id)}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                <div>
                  <h3 style={{ textTransform: 'none', letterSpacing: 0 }}>{pred.policy_name}</h3>
                  <div style={{ fontSize: 11, color: 'var(--text-muted)', fontFamily: 'var(--font-body)', textTransform: 'none', letterSpacing: 0, marginTop: 2 }}>
                    {pred.lifecycle_stage}
                  </div>
                </div>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <ScoreBar score={pred.convergence_score} threshold={3} />
                <Badge level={difficultyLevel(pred.detection_difficulty)}>
                  {pred.detection_difficulty?.split('—')[0]?.trim() || 'Unknown'}
                </Badge>
              </div>
            </div>

            {isExpanded && (
              <div className="panel-body" style={{ borderTop: '1px solid var(--border-subtle)' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 'var(--sp-6)' }}>
                  <div>
                    <div className="detail-label">Predicted Exploitation Mechanics</div>
                    <div style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.7 }}>
                      {pred.mechanics}
                    </div>
                  </div>
                  <div>
                    <div className="detail-label">Enabling Qualities</div>
                    <div style={{ marginBottom: 12 }}>
                      <QualityTags ids={pred.enabling_qualities} />
                    </div>

                    <div className="detail-label">Actor Profile</div>
                    <div style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.5 }}>
                      {pred.actor_profile}
                    </div>

                    <div className="detail-label">Detection Difficulty</div>
                    <div style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.5 }}>
                      {pred.detection_difficulty}
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
