import React, { useCallback, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { usePipeline } from "../data/usePipelineData";
import { QualityTags, ScoreBar, Badge } from "../components/SharedUI";
import type { Prediction, ViewProps } from "../types";

const difficultyLevel = (d?: string): "critical" | "high" | "medium" | "neutral" => {
  if (!d) return "neutral";
  const lower = d.toLowerCase();
  if (lower.includes("hard")) return "critical";
  if (lower.includes("medium")) return "high";
  return "medium";
};

function PredictionCard({
  pred,
  isExpanded,
  onToggleId,
}: {
  pred: Prediction;
  isExpanded: boolean;
  onToggleId: (id: string) => void;
}) {
  return (
    <div className="panel stagger-in">
      <div
        className="panel-header"
        style={{ cursor: "pointer" }}
        role="button"
        tabIndex={0}
        onClick={() => onToggleId(pred.prediction_id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggleId(pred.prediction_id);
          }
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <div>
            <h3 style={{ textTransform: "none", letterSpacing: 0 }}>{pred.policy_name}</h3>
            <div
              style={{
                fontSize: 11,
                color: "var(--text-muted)",
                fontFamily: "var(--font-body)",
                textTransform: "none",
                letterSpacing: 0,
                marginTop: 2,
              }}
            >
              {pred.lifecycle_stage}
            </div>
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <ScoreBar score={pred.convergence_score} threshold={3} />
          <Badge level={difficultyLevel(pred.detection_difficulty)}>
            {pred.detection_difficulty?.split("\u2014")[0]?.trim() || "Unknown"}
          </Badge>
        </div>
      </div>

      {isExpanded && (
        <div className="panel-body" style={{ borderTop: "1px solid var(--border-subtle)" }}>
          <div className="detail-grid-wide">
            <div>
              <div className="detail-label">Predicted Exploitation Mechanics</div>
              <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.7 }}>{pred.mechanics}</div>
            </div>
            <div>
              <div className="detail-label">Enabling Qualities</div>
              <div style={{ marginBottom: 12 }}>
                <QualityTags ids={pred.enabling_qualities} />
              </div>
              <div className="detail-label">Actor Profile</div>
              <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>{pred.actor_profile}</div>
              <div className="detail-label">Detection Difficulty</div>
              <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>
                {pred.detection_difficulty}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default function PredictionView({ onNavigate: _onNavigate }: ViewProps) {
  const { predictions } = usePipeline();
  const [expandedPred, setExpandedPred] = useState<string | null>(null);
  const togglePred = useCallback((id: string) => setExpandedPred((prev) => (prev === id ? null : id)), []);

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Exploitation Predictions</h2>
        <div className="view-desc">
          Structurally-entailed predictions for {predictions.length} high-risk policies — every prediction cites
          specific enabling qualities
        </div>
      </div>

      {predictions.map((pred) => (
        <PredictionCard
          key={pred.prediction_id}
          pred={pred}
          isExpanded={expandedPred === pred.prediction_id}
          onToggleId={togglePred}
        />
      ))}
    </div>
  );
}
