import { useState, useEffect, useCallback } from "react";
import {
  useTriageResults, useResearchSessions, usePolicies,
  useFetchTriageResults, useFetchResearchSessions,
  useRunTriage, useRunDeepResearch, useFetchFindings, useFetchAssessments,
} from "../data/usePipelineSelectors";
import { useAsyncAction } from "../hooks";
import { ErrorBanner } from "../components/SharedUI";
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
  const triage_results = useTriageResults();
  const research_sessions = useResearchSessions();
  const policies = usePolicies();
  const fetchTriageResults = useFetchTriageResults();
  const fetchResearchSessions = useFetchResearchSessions();
  const runTriage = useRunTriage();
  const runDeepResearch = useRunDeepResearch();
  const fetchFindings = useFetchFindings();
  const fetchAssessments = useFetchAssessments();

  const { busy, error, run, clearError } = useAsyncAction();
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

  const handleRunTriage = useCallback(
    () => run("triage", runTriage),
    [run, runTriage],
  );

  const handleRunResearch = useCallback(
    () => run("research", runDeepResearch),
    [run, runDeepResearch],
  );

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
      <ErrorBanner error={error} onDismiss={clearError} />
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
        <button className="btn btn-accent" onClick={handleRunTriage} disabled={!!busy}>
          {busy === "triage" ? "Running Triage..." : "Run Triage"}
        </button>
        <button className="btn btn-accent" onClick={handleRunResearch} disabled={!!busy}>
          {busy === "research" ? "Researching..." : "Run Deep Research"}
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
              <div key={session.session_id} className="panel mb-2">
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
                  <span className="font-semibold" style={{ marginLeft: "0.5rem" }}>
                    {policyName(session.policy_id)}
                  </span>
                  <span className="ml-auto">
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
                    <h4 className="mb-2">
                      Structural Findings ({findings.length})
                    </h4>
                    {findings.length === 0 ? (
                      <div className="empty-state">No findings yet.</div>
                    ) : (
                      <table className="data-table mb-3">
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
                              <td className="text-sm">{f.source_citation || f.source_type}</td>
                              <td>{confidenceBadge(f.confidence)}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}

                    {/* Assessments */}
                    <h4 className="mb-2">
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
