import React, { useState } from "react";
import { usePipeline } from "../data/usePipelineData";
import { ScoreBar, QualityTags, formatDollars, RiskBadge, StageDot } from "../components/SharedUI";
import type { Case, Policy, ViewId, ViewProps, StageStatus } from "../types";

function MetricsRow({
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
      <div className="metric-card stagger-in">
        <div className="metric-label">Enforcement Cases</div>
        <div className="metric-value">{cases}</div>
        <div className="metric-sub">{formatDollars(totalFraudDollars)} total intended losses</div>
      </div>
      <div className="metric-card stagger-in">
        <div className="metric-label">Vulnerability Qualities</div>
        <div className="metric-value">{taxonomy}</div>
        <div className="metric-sub">Threshold: {"\u2265"}{threshold} = high risk</div>
      </div>
      <div className="metric-card stagger-in">
        <div className="metric-label">Policies Scanned</div>
        <div className="metric-value">{policies}</div>
        <div className="metric-sub metric-sub-critical">{criticalPolicies} critical risk</div>
      </div>
      <div className="metric-card stagger-in">
        <div className="metric-label">Detection Patterns</div>
        <div className="metric-value">{detectionPatterns}</div>
        <div className="metric-sub metric-sub-critical">{criticalPatterns} critical priority</div>
      </div>
    </div>
  );
}

function HighRiskTable({
  policies,
  threshold,
  onNavigate,
}: {
  policies: Policy[];
  threshold: number;
  onNavigate: (view: ViewId) => void;
}) {
  const highRisk = policies
    .filter((p) => p.convergence_score >= threshold)
    .sort((a, b) => b.convergence_score - a.convergence_score);

  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Highest-Risk Policies</h3>
        <button className="btn" onClick={() => onNavigate("predictions")}>
          View predictions {"\u2192"}
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

function TopCasesTable({ cases, onNavigate }: { cases: Case[]; onNavigate: (view: ViewId) => void }) {
  const topCases = [...cases].sort((a, b) => (b.scale_dollars || 0) - (a.scale_dollars || 0)).slice(0, 5);

  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Largest Enforcement Cases</h3>
        <button className="btn" onClick={() => onNavigate("cases")}>
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
  1: "Ingest enforcement cases",
  2: "Build vulnerability taxonomy",
  3: "Scan policy catalog",
  4: "Generate predictions",
  5: "Design detection patterns",
  6: "Compile final report",
};

const HUMAN_GATE_STAGES = [2, 5];

function PipelineControls() {
  const { pipeline_status, run_id, apiAvailable, runPipeline, approveStage, seedPipeline, refresh } = usePipeline();
  const [busy, setBusy] = useState<string | null>(null);

  const wrap = (label: string, fn: () => Promise<unknown>) => async () => {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(null);
    }
  };

  if (!apiAvailable) return null;

  const hasPipeline = run_id && run_id !== "static";

  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Pipeline Controls</h3>
        <div className="pipeline-header-actions">
          {!hasPipeline && (
            <button className="btn btn-accent" onClick={wrap("seed", seedPipeline)} disabled={!!busy}>
              {busy === "seed" ? "Seeding\u2026" : "Seed Pipeline"}
            </button>
          )}
          <button className="btn btn-accent" onClick={wrap("run", runPipeline)} disabled={!!busy}>
            {busy === "run" ? "Running\u2026" : "Run Pipeline"}
          </button>
          <button className="btn" onClick={wrap("refresh", refresh)} disabled={!!busy}>
            {busy === "refresh" ? "Refreshing\u2026" : "Refresh"}
          </button>
        </div>
      </div>
      <div className="panel-body">
        <div className="pipeline-controls-stages">
          {[1, 2, 3, 4, 5, 6].map((stage) => {
            const ps = pipeline_status.find((s) => s.stage === stage);
            const status: StageStatus = ps?.status ?? "idle";
            const needsApproval = HUMAN_GATE_STAGES.includes(stage) && status === "pending_review";
            return (
              <div key={stage} className="pipeline-stage-row">
                <StageDot status={status} />
                <span className="stage-name">{STAGE_NAMES[stage]}</span>
                <span className={`stage-status-label ${status}`}>{status.replace("_", " ")}</span>
                {needsApproval && (
                  <button
                    className="btn btn-warning btn-sm"
                    onClick={wrap(`approve-${stage}`, () => approveStage(stage))}
                    disabled={!!busy}
                  >
                    {busy === `approve-${stage}` ? "Approving\u2026" : "Approve"}
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

export default function Dashboard({ onNavigate }: ViewProps) {
  const { cases, taxonomy, policies, detection_patterns, threshold } = usePipeline();
  const criticalPolicies = policies.filter((p) => p.risk_level === "critical").length;
  const totalFraudDollars = cases.reduce((sum, c) => sum + (c.scale_dollars || 0), 0);
  const criticalPatterns = detection_patterns.filter((p) => p.priority === "critical").length;

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Structural Vulnerability Analysis</h2>
        <div className="view-desc">
          HHS OIG fraud detection pipeline — convergence threshold: {"\u2265"}{threshold} qualities = high exploitation
          risk
        </div>
      </div>

      <PipelineControls />

      <MetricsRow
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
        <HighRiskTable policies={policies} threshold={threshold} onNavigate={onNavigate} />
        <TopCasesTable cases={cases} onNavigate={onNavigate} />
      </div>

      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Key Calibration Finding</h3>
        </div>
        <div className="panel-body calibration-text">
          <strong className="calibration-highlight">
            Every enforcement case exceeding $500M in intended losses scored {"\u2265"}3 vulnerability qualities.
          </strong>{" "}
          The most common qualities in large-scale schemes are V1 (Payment Precedes Verification) and V6 (Expansion
          Outpaces Oversight), appearing in 75%+ of major cases. The V1+V2 combination (pay-before-verify +
          self-attesting payment basis) appeared together in every $1B+ scheme. Three policies currently score {"\u2265"}5:
          HCBS, PACE, and Hospital-at-Home — all share the characteristic of services delivered outside institutional
          settings with minimal verification infrastructure.
        </div>
      </div>
    </div>
  );
}
