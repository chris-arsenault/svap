import React, { useState, useEffect, useCallback } from "react";
import {
  useTriageResults, useResearchSessions, usePolicies,
  useFetchTriageResults, useFetchResearchSessions,
  useRunTriage, useRunDeepResearch, useFetchFindings, useFetchAssessments,
} from "../data/usePipelineSelectors";
import { useAsyncAction, useExpandSingle, expandableProps } from "../hooks";
import { ErrorBanner, Badge, QualityTag, QualityTags, ViewHeader, MetricCard } from "../components/SharedUI";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { RiskLevel, TriageResult, ResearchSession, StructuralFinding, QualityAssessment } from "../types";

const CONFIDENCE_LEVELS: Record<string, RiskLevel> = {
  high: "low",
  medium: "medium",
};

function confidenceBadge(confidence: string) {
  const level: RiskLevel = CONFIDENCE_LEVELS[confidence] ?? "high";
  return <Badge level={level}>{confidence}</Badge>;
}

const PRESENT_LEVELS: Record<string, RiskLevel> = {
  yes: "critical",
  no: "low",
};

function presentBadge(present: string) {
  const level: RiskLevel = PRESENT_LEVELS[present] ?? "medium";
  return <Badge level={level}>{present}</Badge>;
}

function triageScoreColor(score: number): string | undefined {
  if (score >= 0.7) return "var(--critical)";
  if (score >= 0.4) return "var(--high)";
  return undefined;
}

const SESSION_STATUS_LEVELS: Record<string, RiskLevel> = {
  assessment_complete: "low",
  failed: "critical",
};

function sessionStatusLevel(status: string): RiskLevel {
  return SESSION_STATUS_LEVELS[status] ?? "medium";
}

function TriageTable({
  results,
  policyName,
}: {
  results: TriageResult[];
  policyName: (id: string) => string;
}) {
  if (results.length === 0) {
    return <div className="empty-state">No triage results yet. Run triage to rank policies by vulnerability.</div>;
  }
  return (
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
        {results.map((t) => (
          <tr key={t.policy_id}>
            <td>{t.priority_rank}</td>
            <td className="td-name">{policyName(t.policy_id)}</td>
            <td>
              <span className="score-highlight" style={{ '--score-color': triageScoreColor(t.triage_score) } as React.CSSProperties}>
                {t.triage_score.toFixed(2)}
              </span>
            </td>
            <td className="hide-on-mobile">{t.rationale.slice(0, 120)}{t.rationale.length > 120 ? "..." : ""}</td>
            <td>{t.uncertainty || "—"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SessionDetail({
  findings,
  assessments,
}: {
  findings: StructuralFinding[];
  assessments: QualityAssessment[];
}) {
  return (
    <div className="panel-body panel-body-bordered">
      <h4 className="mb-2">Structural Findings ({findings.length})</h4>
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

      <h4 className="mb-2">Quality Assessments ({assessments.length})</h4>
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
  );
}

function ResearchMetrics({ triageCount, completedCount, inProgressCount }: {
  triageCount: number; completedCount: number; inProgressCount: number;
}) {
  return (
    <div className="metrics-row">
      <MetricCard label="Triaged" value={triageCount} sub="policies ranked" />
      <MetricCard label="Researched" value={completedCount} sub="deep research" />
      <MetricCard label="In Progress" value={inProgressCount} sub="sessions" />
    </div>
  );
}

function SessionCard({ session, isExpanded, policyName, findings, assessments, onExpand }: {
  session: ResearchSession;
  isExpanded: boolean;
  policyName: string;
  findings: StructuralFinding[];
  assessments: QualityAssessment[];
  onExpand: (policyId: string) => void;
}) {
  return (
    <div className="panel mb-2">
      <div
        className="panel-header clickable"
        {...expandableProps(() => onExpand(session.policy_id))}
      >
        {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        <span className="font-semibold ml-2">{policyName}</span>
        <span className="ml-auto">
          <Badge level={sessionStatusLevel(session.status)}>{session.status}</Badge>
        </span>
      </div>
      {isExpanded && <SessionDetail findings={findings} assessments={assessments} />}
    </div>
  );
}

function SessionsPanel({ sessions, policyName }: {
  sessions: ResearchSession[];
  policyName: (id: string) => string;
}) {
  const fetchFindings = useFetchFindings();
  const fetchAssessments = useFetchAssessments();
  const { expandedId: expandedPolicy, toggle: togglePolicy } = useExpandSingle();
  const [findings, setFindings] = useState<StructuralFinding[]>([]);
  const [assessments, setAssessments] = useState<QualityAssessment[]>([]);

  const handleExpand = useCallback(
    async (policyId: string) => {
      if (expandedPolicy === policyId) {
        togglePolicy(policyId);
        return;
      }
      togglePolicy(policyId);
      const [f, a] = await Promise.all([fetchFindings(policyId), fetchAssessments(policyId)]);
      setFindings(f);
      setAssessments(a);
    },
    [expandedPolicy, togglePolicy, fetchFindings, fetchAssessments],
  );

  return (
    <div className="panel stagger-in">
      <div className="panel-header"><h3>Research Sessions</h3></div>
      <div className="panel-body">
        {sessions.length === 0 ? (
          <div className="empty-state">No research sessions. Run deep research after triage.</div>
        ) : (
          sessions.map((session) => (
            <SessionCard
              key={session.session_id}
              session={session}
              isExpanded={expandedPolicy === session.policy_id}
              policyName={policyName(session.policy_id)}
              findings={findings}
              assessments={assessments}
              onExpand={handleExpand}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default function ResearchView() {
  const triage_results = useTriageResults();
  const research_sessions = useResearchSessions();
  const policies = usePolicies();
  const fetchTriageResults = useFetchTriageResults();
  const fetchResearchSessions = useFetchResearchSessions();
  const runTriage = useRunTriage();
  const runDeepResearch = useRunDeepResearch();

  const { busy, error, run, clearError } = useAsyncAction();

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

  const handleRunTriage = useCallback(() => run("triage", runTriage), [run, runTriage]);
  const handleRunResearch = useCallback(() => run("research", runDeepResearch), [run, runDeepResearch]);

  const completedCount = research_sessions.filter(
    (s) => s.status === "findings_complete" || s.status === "assessment_complete",
  ).length;
  const inProgressCount = research_sessions.filter((s) => s.status === "researching").length;

  return (
    <div>
      <ErrorBanner error={error} onDismiss={clearError} />
      <ViewHeader title="Policy Research" description="Three-pass structural vulnerability analysis: triage, deep regulatory research, and quality assessment." />
      <ResearchMetrics triageCount={triage_results.length} completedCount={completedCount} inProgressCount={inProgressCount} />
      <div className="filter-bar filter-bar-mb stagger-in">
        <button className="btn btn-accent" onClick={handleRunTriage} disabled={!!busy}>
          {busy === "triage" ? "Running Triage..." : "Run Triage"}
        </button>
        <button className="btn btn-accent" onClick={handleRunResearch} disabled={!!busy}>
          {busy === "research" ? "Researching..." : "Run Deep Research"}
        </button>
      </div>
      <div className="panel stagger-in">
        <div className="panel-header"><h3>Triage Rankings</h3></div>
        <div className="panel-body dense">
          <TriageTable results={triage_results} policyName={policyName} />
        </div>
      </div>
      <SessionsPanel sessions={research_sessions} policyName={policyName} />
    </div>
  );
}
