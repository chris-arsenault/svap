import { useEffect } from "react";
import { useDimensions, useFetchDimensions } from "../data/usePipelineSelectors";
import { Badge, QualityTags, ViewHeader, MetricCard } from "../components/SharedUI";
import { useExpandSingle, expandableProps } from "../hooks";
import type { Dimension, RiskLevel } from "../types";
import { ChevronDown, ChevronRight } from "lucide-react";

const ORIGIN_LEVELS: Record<string, RiskLevel> = {
  seed: "low",
  case_derived: "medium",
  manual: "high",
};

function originBadge(origin: string) {
  const level: RiskLevel = ORIGIN_LEVELS[origin] ?? "medium";
  return <Badge level={level}>{origin}</Badge>;
}

function DimensionCard({
  dim,
  isExpanded,
  onToggle,
}: {
  dim: Dimension;
  isExpanded: boolean;
  onToggle: (id: string) => void;
}) {
  return (
    <div className="quality-card stagger-in">
      <div
        className="quality-card-header clickable cursor-pointer"
        {...expandableProps(() => onToggle(dim.dimension_id))}
      >
        <div className="flex-row">
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <span className="quality-card-name">{dim.name}</span>
        </div>
        <div className="flex-row">
          {originBadge(dim.origin)}
          <span className="text-secondary">{dim.dimension_id}</span>
        </div>
      </div>

      <div className="quality-card-def">{dim.definition}</div>

      {isExpanded && (
        <div className="panel-expand-body">
          <div className="detail-grid">
            <div>
              <div className="detail-label">Probing Questions</div>
              <div className="detail-text">
                {dim.probing_questions && dim.probing_questions.length > 0 ? (
                  <ul className="list-compact">
                    {dim.probing_questions.map((q, i) => (
                      <li key={i}>{q}</li>
                    ))}
                  </ul>
                ) : (
                  "None defined"
                )}
              </div>
            </div>
            <div>
              <div className="detail-label">Related Qualities</div>
              <div className="detail-text">
                {dim.related_quality_ids && dim.related_quality_ids.length > 0 ? (
                  <QualityTags ids={dim.related_quality_ids} />
                ) : (
                  "None linked"
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default function DimensionRegistryView() {
  const dimensions = useDimensions();
  const fetchDimensions = useFetchDimensions();

  const { expandedId, toggle: toggleId } = useExpandSingle();

  useEffect(() => {
    fetchDimensions();
  }, [fetchDimensions]);

  return (
    <div>
      <ViewHeader
        title="Dimension Registry"
        description="Structural properties that bridge cases and policies. Each dimension describes a mechanical aspect of how a policy operates."
      />

      <div className="metrics-row">
        <MetricCard label="Dimensions" value={dimensions.length} sub="registered" />
        <MetricCard label="Seed" value={dimensions.filter((d) => d.origin === "seed").length} sub="built-in" />
        <MetricCard label="Derived" value={dimensions.filter((d) => d.origin === "case_derived" || d.origin === "policy_derived").length} sub="from analysis" />
      </div>

      <div className="quality-grid">
        {dimensions.length === 0 ? (
          <div className="empty-state stagger-in">
            No dimensions registered. Run the pipeline to seed initial dimensions.
          </div>
        ) : (
          dimensions.map((dim) => (
            <DimensionCard
              key={dim.dimension_id}
              dim={dim}
              isExpanded={expandedId === dim.dimension_id}
              onToggle={toggleId}
            />
          ))
        )}
      </div>
    </div>
  );
}
