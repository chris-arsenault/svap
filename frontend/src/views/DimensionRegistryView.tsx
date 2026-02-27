import { useEffect, useState, useCallback } from "react";
import { useShallow } from "zustand/shallow";
import { usePipelineStore } from "../data/pipelineStore";
import { Badge, QualityTags } from "../components/SharedUI";
import { ChevronDown, ChevronRight } from "lucide-react";

function originBadge(origin: string) {
  const level =
    origin === "seed" ? "low" : origin === "case_derived" ? "medium" : origin === "manual" ? "high" : "medium";
  return <Badge level={level}>{origin}</Badge>;
}

export default function DimensionRegistryView() {
  const { dimensions, fetchDimensions } = usePipelineStore(
    useShallow((s) => ({
      dimensions: s.dimensions,
      fetchDimensions: s.fetchDimensions,
    })),
  );

  const [expandedId, setExpandedId] = useState<string | null>(null);

  useEffect(() => {
    fetchDimensions();
  }, [fetchDimensions]);

  const toggleId = useCallback(
    (id: string) => setExpandedId((prev) => (prev === id ? null : id)),
    [],
  );

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Dimension Registry</h2>
        <div className="view-desc">
          Structural properties that bridge cases and policies. Each dimension describes a mechanical
          aspect of how a policy operates.
        </div>
      </div>

      <div className="metrics-row">
        <div className="metric-card stagger-in">
          <div className="metric-label">Dimensions</div>
          <div className="metric-value">{dimensions.length}</div>
          <div className="metric-sub">registered</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Seed</div>
          <div className="metric-value">{dimensions.filter((d) => d.origin === "seed").length}</div>
          <div className="metric-sub">built-in</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Derived</div>
          <div className="metric-value">
            {dimensions.filter((d) => d.origin === "case_derived" || d.origin === "policy_derived").length}
          </div>
          <div className="metric-sub">from analysis</div>
        </div>
      </div>

      <div className="quality-grid">
        {dimensions.length === 0 ? (
          <div className="empty-state stagger-in">
            No dimensions registered. Run the pipeline to seed initial dimensions.
          </div>
        ) : (
          dimensions.map((dim) => (
            <div key={dim.dimension_id} className="quality-card stagger-in">
              <div
                className="quality-card-header clickable"
                onClick={() => toggleId(dim.dimension_id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggleId(dim.dimension_id);
                  }
                }}
                role="button"
                tabIndex={0}
                style={{ cursor: "pointer" }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                  {expandedId === dim.dimension_id ? (
                    <ChevronDown size={16} />
                  ) : (
                    <ChevronRight size={16} />
                  )}
                  <span className="quality-card-name">{dim.name}</span>
                </div>
                <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
                  {originBadge(dim.origin)}
                  <span style={{ fontSize: "0.8em", color: "var(--text-secondary)" }}>
                    {dim.dimension_id}
                  </span>
                </div>
              </div>

              <div className="quality-card-def">{dim.definition}</div>

              {expandedId === dim.dimension_id && (
                <div style={{ padding: "0.75rem", borderTop: "1px solid var(--border)" }}>
                  <div className="detail-grid">
                    <div>
                      <div className="detail-label">Probing Questions</div>
                      <div className="detail-text">
                        {dim.probing_questions && dim.probing_questions.length > 0 ? (
                          <ul style={{ margin: 0, paddingLeft: "1.2rem" }}>
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
          ))
        )}
      </div>
    </div>
  );
}
