import { useState, useEffect, useCallback } from "react";
import { useRunPipeline, useSeedPipeline, useRefresh } from "../data/usePipelineSelectors";
import { apiGet, apiPost } from "../data/api";
import { useAsyncAction } from "../hooks";
import { ErrorBanner, Badge, ViewHeader, MetricCard } from "../components/SharedUI";

// ── Types ────────────────────────────────────────────────────────────────

interface Execution {
  name: string;
  execution_arn: string;
  status: string;
  start_date: string;
  stop_date: string | null;
}

interface StageStatus {
  stage: number;
  status: string;
}

interface PipelineRun {
  run_id: string;
  created_at: string;
  notes: string;
  stages: StageStatus[];
}

// ── Helpers ──────────────────────────────────────────────────────────────

function timeAgo(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime();
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function executionStatusLevel(status: string) {
  switch (status) {
    case "RUNNING":
      return "medium" as const;
    case "SUCCEEDED":
      return "low" as const;
    case "FAILED":
      return "critical" as const;
    case "ABORTED":
      return "high" as const;
    default:
      return "neutral" as const;
  }
}

function stageStatusClass(status: string) {
  switch (status) {
    case "completed":
    case "approved":
      return "stage-chip completed";
    case "running":
      return "stage-chip running";
    case "failed":
      return "stage-chip failed";
    case "pending_review":
      return "stage-chip pending_review";
    default:
      return "stage-chip";
  }
}

const STAGE_ORDER = [0, 1, 2, 3, 4, 5, 6];

function ExecutionsPanel({
  executions,
  busy,
  onRefresh,
  onStop,
}: {
  executions: Execution[];
  busy: string | null;
  onRefresh: () => void;
  onStop: (arn: string, name: string) => void;
}) {
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Step Functions Executions</h3>
        <button className="btn" onClick={onRefresh} disabled={busy === "refresh"}>
          {busy === "refresh" ? "Refreshing..." : "Refresh"}
        </button>
      </div>
      <div className="panel-body dense">
        {executions.length === 0 ? (
          <div className="empty-state">No executions found</div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Execution</th>
                <th>Status</th>
                <th>Started</th>
                <th>Duration</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {executions.map((ex) => (
                <tr key={ex.execution_arn}>
                  <td className="td-mono">{ex.name}</td>
                  <td>
                    <Badge level={executionStatusLevel(ex.status)}>{ex.status}</Badge>
                  </td>
                  <td>{timeAgo(ex.start_date)}</td>
                  <td>
                    {ex.stop_date
                      ? `${Math.round((new Date(ex.stop_date).getTime() - new Date(ex.start_date).getTime()) / 1000)}s`
                      : "—"}
                  </td>
                  <td>
                    {ex.status === "RUNNING" && (
                      <button className="btn btn-danger btn-sm" onClick={() => onStop(ex.execution_arn, ex.name)} disabled={busy === ex.execution_arn}>
                        {busy === ex.execution_arn ? "Stopping..." : "Stop"}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function RunsPanel({
  runs,
  busy,
  onNewRun,
  onSeed,
  onDelete,
}: {
  runs: PipelineRun[];
  busy: string | null;
  onNewRun: () => void;
  onSeed: () => void;
  onDelete: (runId: string) => void;
}) {
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Pipeline Runs</h3>
        <div className="flex-row">
          <button className="btn btn-accent" onClick={onNewRun} disabled={busy === "new-run"}>
            {busy === "new-run" ? "Starting..." : "New Run"}
          </button>
          <button className="btn" onClick={onSeed} disabled={busy === "seed"}>
            {busy === "seed" ? "Seeding..." : "Seed"}
          </button>
        </div>
      </div>
      <div className="panel-body dense">
        {runs.length === 0 ? (
          <div className="empty-state">No pipeline runs found</div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Run ID</th>
                <th>Created</th>
                <th>Stages</th>
                <th>Notes</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {runs.map((r) => {
                const stageMap = new Map(r.stages.map((s) => [s.stage, s.status]));
                return (
                  <tr key={r.run_id}>
                    <td className="td-mono">{r.run_id}</td>
                    <td>{timeAgo(r.created_at)}</td>
                    <td>
                      <div className="stage-chips">
                        {STAGE_ORDER.map((n) => {
                          const status = stageMap.get(n);
                          return (
                            <span key={n} className={status ? stageStatusClass(status) : "stage-chip idle"} title={`Stage ${n}: ${status || "not started"}`}>
                              {n}
                            </span>
                          );
                        })}
                      </div>
                    </td>
                    <td className="td-muted">{r.notes || "—"}</td>
                    <td>
                      <button className="btn btn-danger btn-sm" onClick={() => onDelete(r.run_id)} disabled={busy === r.run_id}>
                        {busy === r.run_id ? "..." : "Delete"}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function ManagementMetrics({ runningCount, totalRuns }: { runningCount: number; totalRuns: number }) {
  return (
    <div className="metrics-row">
      <MetricCard label="Running" value={runningCount} sub="executions" />
      <MetricCard label="Total Runs" value={totalRuns} sub="in database" />
    </div>
  );
}

// ── Component ────────────────────────────────────────────────────────────

export default function ManagementView() {
  const runPipeline = useRunPipeline();
  const seedPipeline = useSeedPipeline();
  const refresh = useRefresh();

  const [executions, setExecutions] = useState<Execution[]>([]);
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const { busy, error, run, clearError } = useAsyncAction();

  const fetchExecutions = useCallback(async () => {
    const res = await apiGet("/management/executions");
    if (!res.ok) throw new Error(`${res.status}`);
    setExecutions(await res.json());
  }, []);

  const fetchRuns = useCallback(async () => {
    const res = await apiGet("/management/runs");
    if (!res.ok) throw new Error(`${res.status}`);
    setRuns(await res.json());
  }, []);

  const refreshAll = useCallback(async () => {
    await run("refresh", async () => {
      await Promise.all([fetchExecutions(), fetchRuns()]);
    });
  }, [fetchExecutions, fetchRuns, run]);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  const stopExecution = useCallback(async (arn: string, name: string) => {
    if (!window.confirm(`Stop execution "${name}"?`)) return;
    await run(arn, async () => {
      await apiPost("/management/executions/stop", { execution_arn: arn });
      await Promise.all([fetchExecutions(), fetchRuns()]);
    });
  }, [run, fetchExecutions, fetchRuns]);

  const deleteRun = useCallback(async (runId: string) => {
    if (!window.confirm(`Delete run "${runId}" and all its data?`)) return;
    await run(runId, async () => {
      await apiPost("/management/runs/delete", { run_id: runId });
      await Promise.all([fetchExecutions(), fetchRuns(), refresh()]);
    });
  }, [run, fetchExecutions, fetchRuns, refresh]);

  const handleNewRun = useCallback(async () => {
    await run("new-run", async () => {
      await runPipeline();
      await Promise.all([fetchExecutions(), fetchRuns()]);
    });
  }, [run, runPipeline, fetchExecutions, fetchRuns]);

  const handleSeed = useCallback(async () => {
    if (!window.confirm("Reset corpus and load seed data?")) return;
    await run("seed", async () => {
      await seedPipeline();
      await Promise.all([fetchExecutions(), fetchRuns()]);
    });
  }, [run, seedPipeline, fetchExecutions, fetchRuns]);

  const runningCount = executions.filter((e) => e.status === "RUNNING").length;

  return (
    <div>
      <ViewHeader title="Pipeline Management" description="Manage Step Functions executions and pipeline runs" />
      <ErrorBanner error={error} onDismiss={clearError} />
      <ManagementMetrics runningCount={runningCount} totalRuns={runs.length} />
      <ExecutionsPanel executions={executions} busy={busy} onRefresh={refreshAll} onStop={stopExecution} />
      <RunsPanel runs={runs} busy={busy} onNewRun={handleNewRun} onSeed={handleSeed} onDelete={deleteRun} />
    </div>
  );
}
