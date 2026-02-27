import React, { useCallback, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { usePipelineStore } from "../data/pipelineStore";
import { QualityTags, ScoreBar, Badge } from "../components/SharedUI";
import type { Prediction } from "../types";

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
        className="panel-header clickable"
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
        <div className="prediction-header-left">
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <div>
            <h3 className="prediction-policy-name">{pred.policy_name}</h3>
            <div className="prediction-lifecycle-stage">
              {pred.lifecycle_stage}
            </div>
          </div>
        </div>
        <div className="prediction-header-right">
          <ScoreBar score={pred.convergence_score} threshold={3} />
          <Badge level={difficultyLevel(pred.detection_difficulty)}>
            {pred.detection_difficulty?.split("\u2014")[0]?.trim() || "Unknown"}
          </Badge>
        </div>
      </div>

      {isExpanded && (
        <div className="panel-body panel-body-bordered">
          <div className="detail-grid-wide">
            <div>
              <div className="detail-label">Predicted Exploitation Mechanics</div>
              <div className="prediction-detail-text">{pred.mechanics}</div>
            </div>
            <div>
              <div className="detail-label">Enabling Qualities</div>
              <div className="mb-3">
                <QualityTags ids={pred.enabling_qualities} />
              </div>
              <div className="detail-label">Actor Profile</div>
              <div className="prediction-detail-text">{pred.actor_profile}</div>
              <div className="detail-label">Detection Difficulty</div>
              <div className="prediction-detail-text">
                {pred.detection_difficulty}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default function PredictionView() {
  const predictions = usePipelineStore((s) => s.predictions);
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
