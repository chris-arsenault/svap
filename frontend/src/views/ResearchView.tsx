import { useState, useEffect, useCallback } from "react";
import { useShallow } from "zustand/shallow";
import { usePipelineStore } from "../data/pipelineStore";
import { Badge, QualityTag, QualityTags } from "../components/SharedUI";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { StructuralFinding, QualityAssessment } from "../types";

function confidenceBadge(confidence: string) {
  const level = confidence === "high" ? "low" : confidence === "medium" ? "medium" : "high";
  return <Badge level={level}>{confidence}</Badge>;
}

function presentBadge(present: string) {
  const level = present === "yes" ? "critical" : present === "no" ? "low" : "medium";
  return <Badge level={level}>{present}</Badge>;
}

export default function ResearchView() {
  const {
    triage_results,
    research_sessions,
    policies,
    fetchTriageResults,
    fetchResearchSessions,
    runTriage,
    runDeepResearch,
    fetchFindings,
    fetchAssessments,
  } = usePipelineStore(
    useShallow((s) => ({
      triage_results: s.triage_results,
      research_sessions: s.research_sessions,
      policies: s.policies,
      fetchTriageResults: s.fetchTriageResults,
      fetchResearchSessions: s.fetchResearchSessions,
      runTriage: s.runTriage,
      runDeepResearch: s.runDeepResearch,
      fetchFindings: s.fetchFindings,
      fetchAssessments: s.fetchAssessments,
    })),
  );

  const [running, setRunning] = useState<string | null>(null);
  const [expandedPolicy, setExpandedPolicy] = useState<string | null>(null);
  const [findings, setFindings] = useState<StructuralFinding[]>([]);
  const [assessments, setAssessments] = useState<QualityAssessment[]>([]);

  useEffect(() => {
    fetchTriageResults();
    fetchResearchSessions();
  }, [fetchTriageResults, fetchResearchSessions]);

  const policyName = useCallback(
    (policyId: string) => {
      const p = policies.find((p) => p.policy_id === policyId);
      return p?.name || policyId;
    },
    [policies],
  );

  const handleRunTriage = useCallback(async () => {
    setRunning("triage");
    try {
      await runTriage();
    } finally {
      setRunning(null);
    }
  }, [runTriage]);

  const handleRunResearch = useCallback(async () => {
    setRunning("research");
    try {
      await runDeepResearch();
    } finally {
      setRunning(null);
    }
  }, [runDeepResearch]);

  const handleExpand = useCallback(
    async (policyId: string) => {
      if (expandedPolicy === policyId) {
        setExpandedPolicy(null);
        return;
      }
      setExpandedPolicy(policyId);
      const [f, a] = await Promise.all([fetchFindings(policyId), fetchAssessments(policyId)]);
      setFindings(f);
      setAssessments(a);
    },
    [expandedPolicy, fetchFindings, fetchAssessments],
  );

  const completedSessions = research_sessions.filter(
    (s) => s.status === "findings_complete" || s.status === "assessment_complete",
  );

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Policy Research</h2>
        <div className="view-desc">
          Three-pass structural vulnerability analysis: triage, deep regulatory research, and quality assessment.
        </div>
      </div>

      <div className="metrics-row">
        <div className="metric-card stagger-in">
          <div className="metric-label">Triaged</div>
          <div className="metric-value">{triage_results.length}</div>
          <div className="metric-sub">policies ranked</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Researched</div>
          <div className="metric-value">{completedSessions.length}</div>
          <div className="metric-sub">deep research</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">In Progress</div>
          <div className="metric-value">
            {research_sessions.filter((s) => s.status === "researching").length}
          </div>
          <div className="metric-sub">sessions</div>
        </div>
      </div>

      {/* Actions bar */}
      <div className="filter-bar filter-bar-mb stagger-in">
        <button className="btn btn-accent" onClick={handleRunTriage} disabled={running !== null}>
          {running === "triage" ? "Running Triage..." : "Run Triage"}
        </button>
        <button className="btn btn-accent" onClick={handleRunResearch} disabled={running !== null}>
          {running === "research" ? "Researching..." : "Run Deep Research"}
        </button>
      </div>

      {/* Triage rankings */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Triage Rankings</h3>
        </div>
        <div className="panel-body dense">
          {triage_results.length === 0 ? (
            <div className="empty-state">No triage results yet. Run triage to rank policies by vulnerability.</div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>Policy</th>
                  <th>Score</th>
                  <th className="hide-on-mobile">Rationale</th>
                  <th>Uncertainty</th>
                </tr>
              </thead>
              <tbody>
                {triage_results.map((t) => (
                  <tr key={t.policy_id}>
                    <td>{t.priority_rank}</td>
                    <td className="td-name">{policyName(t.policy_id)}</td>
                    <td>
                      <span
                        style={{
                          color: t.triage_score >= 0.7 ? "var(--critical)" : t.triage_score >= 0.4 ? "var(--high)" : undefined,
                          fontWeight: 600,
                        }}
                      >
                        {t.triage_score.toFixed(2)}
                      </span>
                    </td>
                    <td className="hide-on-mobile">{t.rationale.slice(0, 120)}{t.rationale.length > 120 ? "..." : ""}</td>
                    <td>{t.uncertainty || "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>

      {/* Research sessions — expandable */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Research Sessions</h3>
        </div>
        <div className="panel-body">
          {research_sessions.length === 0 ? (
            <div className="empty-state">No research sessions. Run deep research after triage.</div>
          ) : (
            research_sessions.map((session) => (
              <div key={session.session_id} className="panel" style={{ marginBottom: "0.5rem" }}>
                <div
                  className="panel-header clickable"
                  onClick={() => handleExpand(session.policy_id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      handleExpand(session.policy_id);
                    }
                  }}
                  role="button"
                  tabIndex={0}
                >
                  {expandedPolicy === session.policy_id ? (
                    <ChevronDown size={16} />
                  ) : (
                    <ChevronRight size={16} />
                  )}
                  <span style={{ marginLeft: "0.5rem", fontWeight: 600 }}>
                    {policyName(session.policy_id)}
                  </span>
                  <span style={{ marginLeft: "auto" }}>
                    <Badge
                      level={
                        session.status === "assessment_complete"
                          ? "low"
                          : session.status === "failed"
                            ? "critical"
                            : "medium"
                      }
                    >
                      {session.status}
                    </Badge>
                  </span>
                </div>

                {expandedPolicy === session.policy_id && (
                  <div className="panel-body panel-body-bordered">
                    {/* Findings */}
                    <h4 style={{ marginBottom: "0.5rem" }}>
                      Structural Findings ({findings.length})
                    </h4>
                    {findings.length === 0 ? (
                      <div className="empty-state">No findings yet.</div>
                    ) : (
                      <table className="data-table" style={{ marginBottom: "1rem" }}>
                        <thead>
                          <tr>
                            <th>Dimension</th>
                            <th>Observation</th>
                            <th>Source</th>
                            <th>Confidence</th>
                          </tr>
                        </thead>
                        <tbody>
                          {findings.map((f) => (
                            <tr key={f.finding_id}>
                              <td>{f.dimension_id}</td>
                              <td>{f.observation.slice(0, 150)}{f.observation.length > 150 ? "..." : ""}</td>
                              <td style={{ fontSize: "0.85em" }}>{f.source_citation || f.source_type}</td>
                              <td>{confidenceBadge(f.confidence)}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}

                    {/* Assessments */}
                    <h4 style={{ marginBottom: "0.5rem" }}>
                      Quality Assessments ({assessments.length})
                    </h4>
                    {assessments.length === 0 ? (
                      <div className="empty-state">No assessments yet.</div>
                    ) : (
                      <table className="data-table">
                        <thead>
                          <tr>
                            <th>Quality</th>
                            <th>Present</th>
                            <th>Confidence</th>
                            <th className="hide-on-mobile">Rationale</th>
                            <th>Evidence</th>
                          </tr>
                        </thead>
                        <tbody>
                          {assessments.map((a) => (
                            <tr key={a.assessment_id}>
                              <td><QualityTag id={a.quality_id} /></td>
                              <td>{presentBadge(a.present)}</td>
                              <td>{confidenceBadge(a.confidence)}</td>
                              <td className="hide-on-mobile">
                                {a.rationale.slice(0, 100)}{a.rationale.length > 100 ? "..." : ""}
                              </td>
                              <td>
                                <QualityTags ids={a.evidence_finding_ids} />
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                  </div>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
