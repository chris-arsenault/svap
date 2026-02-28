import React from "react";
import { useNavigate } from "react-router-dom";
import {
  usePipelineStatus, useRunId, useApiAvailable,
  useRunPipeline, useApproveStage, useSeedPipeline, useRefresh,
  useCases, useTaxonomy, usePolicies, useDetectionPatterns, useThreshold,
} from "../data/usePipelineSelectors";
import { useAsyncAction } from "../hooks";
import { ScoreBar, QualityTags, RiskBadge, StageDot, ErrorBanner, ViewHeader, MetricCard } from "../components/SharedUI";
import { formatDollars } from "../utils";
import type { Case, Policy, StageStatus } from "../types";

function DashboardMetrics({
  cases,
  taxonomy,
  policies,
  detectionPatterns,
  threshold,
  totalFraudDollars,
  criticalPolicies,
  criticalPatterns,
}: {
  cases: number;
  taxonomy: number;
  policies: number;
  detectionPatterns: number;
  threshold: number;
  totalFraudDollars: number;
  criticalPolicies: number;
  criticalPatterns: number;
}) {
  return (
    <div className="metrics-row">
      <MetricCard label="Enforcement Cases" value={cases} sub={<>{formatDollars(totalFraudDollars)} total intended losses</>} />
      <MetricCard label="Vulnerability Qualities" value={taxonomy} sub={<>Threshold: {"\u2265"}{threshold} = high risk</>} />
      <MetricCard label="Policies Scanned" value={policies} sub={<span className="metric-sub-critical">{criticalPolicies} critical risk</span>} />
      <MetricCard label="Detection Patterns" value={detectionPatterns} sub={<span className="metric-sub-critical">{criticalPatterns} critical priority</span>} />
    </div>
  );
}

function HighRiskTable({
  policies,
  threshold,
}: {
  policies: Policy[];
  threshold: number;
}) {
  const navigate = useNavigate();
  const highRisk = policies
    .filter((p) => p.convergence_score >= threshold)
    .sort((a, b) => b.convergence_score - a.convergence_score);

  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Highest-Risk Policies</h3>
        <button className="btn" onClick={() => navigate("/predictions")}>
          View exploitation trees {"\u2192"}
        </button>
      </div>
      <div className="panel-body dense">
        <table className="data-table">
          <thead>
            <tr>
              <th>Policy</th>
              <th>Score</th>
              <th>Qualities</th>
              <th>Risk</th>
            </tr>
          </thead>
          <tbody>
            {highRisk.map((p) => (
              <tr key={p.policy_id}>
                <td className="td-name">{p.name}</td>
                <td>
                  <ScoreBar score={p.convergence_score} threshold={threshold} />
                </td>
                <td>
                  <QualityTags ids={p.qualities} />
                </td>
                <td>
                  <RiskBadge level={p.risk_level} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function TopCasesTable({ cases }: { cases: Case[] }) {
  const navigate = useNavigate();
  const topCases = [...cases].sort((a, b) => (b.scale_dollars || 0) - (a.scale_dollars || 0)).slice(0, 5);

  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Largest Enforcement Cases</h3>
        <button className="btn" onClick={() => navigate("/cases")}>
          All cases {"\u2192"}
        </button>
      </div>
      <div className="panel-body dense">
        <table className="data-table">
          <thead>
            <tr>
              <th>Case</th>
              <th>Scale</th>
              <th>Qualities</th>
            </tr>
          </thead>
          <tbody>
            {topCases.map((c) => (
              <tr key={c.case_id}>
                <td className="td-case-name">{c.case_name}</td>
                <td className="td-mono">{formatDollars(c.scale_dollars)}</td>
                <td>
                  <QualityTags ids={c.qualities} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

const STAGE_NAMES: Record<number, string> = {
  0: "Fetch enforcement sources",
  1: "Ingest enforcement cases",
  2: "Build vulnerability taxonomy",
  3: "Convergence scoring",
  4: "Policy scanning",
  5: "Exploitation prediction",
  6: "Detection patterns",
};

const HUMAN_GATE_STAGES = [2, 5];

function PipelineControls() {
  const pipeline_status = usePipelineStatus();
  const run_id = useRunId();
  const apiAvailable = useApiAvailable();
  const runPipeline = useRunPipeline();
  const approveStage = useApproveStage();
  const seedPipeline = useSeedPipeline();
  const refresh = useRefresh();
  const { busy, error, run, clearError } = useAsyncAction();

  if (!apiAvailable) return null;

  const hasPipeline = run_id && run_id !== "static";

  return (
    <>
    <ErrorBanner error={error} onDismiss={clearError} />
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Pipeline Controls</h3>
        <div className="pipeline-header-actions">
          {!hasPipeline && (
            <button className="btn btn-accent" onClick={() => run("seed", seedPipeline)} disabled={!!busy}>
              {busy === "seed" ? "Seeding\u2026" : "Seed Pipeline"}
            </button>
          )}
          <button className="btn btn-accent" onClick={() => run("run", runPipeline)} disabled={!!busy}>
            {busy === "run" ? "Running\u2026" : "Run Pipeline"}
          </button>
          <button className="btn" onClick={() => run("refresh", refresh)} disabled={!!busy}>
            {busy === "refresh" ? "Refreshing\u2026" : "Refresh"}
          </button>
        </div>
      </div>
      <div className="panel-body">
        <div className="pipeline-controls-stages">
          {[0, 1, 2, 3, 4, 5, 6].map((stage) => {
            const ps = pipeline_status.find((s) => s.stage === stage);
            const status: StageStatus = ps?.status ?? "idle";
            const needsApproval = HUMAN_GATE_STAGES.includes(stage) && status === "pending_review";
            const errorMsg = ps?.error_message;
            return (
              <div key={stage} className="pipeline-stage-row">
                <StageDot status={status} />
                <span className="stage-name">{STAGE_NAMES[stage]}</span>
                <span className={`stage-status-label ${status}`}>{status.replace("_", " ")}</span>
                {needsApproval && (
                  <button
                    className="btn btn-warning btn-sm"
                    onClick={() => run(`approve-${stage}`, () => approveStage(stage))}
                    disabled={!!busy}
                  >
                    {busy === `approve-${stage}` ? "Approving\u2026" : "Approve"}
                  </button>
                )}
                {status === "failed" && errorMsg && (
                  <div className="stage-error">{errorMsg}</div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
    </>
  );
}

export default function Dashboard() {
  const cases = useCases();
  const taxonomy = useTaxonomy();
  const policies = usePolicies();
  const detection_patterns = useDetectionPatterns();
  const threshold = useThreshold();
  const criticalPolicies = policies.filter((p) => p.risk_level === "critical").length;
  const totalFraudDollars = cases.reduce((sum, c) => sum + (c.scale_dollars || 0), 0);
  const criticalPatterns = detection_patterns.filter((p) => p.priority === "critical").length;

  return (
    <div>
      <ViewHeader
        title="Structural Vulnerability Analysis"
        description={<>HHS OIG fraud detection pipeline — convergence threshold: {"\u2265"}{threshold} qualities = high exploitation risk</>}
      />

      <PipelineControls />

      <DashboardMetrics
        cases={cases.length}
        taxonomy={taxonomy.length}
        policies={policies.length}
        detectionPatterns={detection_patterns.length}
        threshold={threshold}
        totalFraudDollars={totalFraudDollars}
        criticalPolicies={criticalPolicies}
        criticalPatterns={criticalPatterns}
      />

      <div className="split-view">
        <HighRiskTable policies={policies} threshold={threshold} />
        <TopCasesTable cases={cases} />
      </div>

      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Key Calibration Finding</h3>
        </div>
        <div className="panel-body calibration-text">
          <strong className="calibration-highlight">
            Every enforcement case exceeding $500M in intended losses scored {"\u2265"}3 vulnerability qualities.
          </strong>{" "}
          The most common qualities in large-scale schemes are Payment Precedes Verification and Expansion
          Outpaces Oversight, appearing in 75%+ of major cases. The pay-before-verify + self-attesting payment basis
          combination appeared together in every $1B+ scheme. Three policies currently score {"\u2265"}5:
          HCBS, PACE, and Hospital-at-Home — all share the characteristic of services delivered outside institutional
          settings with minimal verification infrastructure.
        </div>
      </div>
    </div>
  );
}
