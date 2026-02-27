import { useState, useEffect, useCallback } from "react";
import { usePipelineStore } from "../data/pipelineStore";
import { config } from "../config";
import { getToken } from "../auth";
import { Badge } from "../components/SharedUI";

const API_BASE = config.apiBaseUrl || "/api";

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

// ── Component ────────────────────────────────────────────────────────────

export default function ManagementView() {
  const runPipeline = usePipelineStore((s) => s.runPipeline);
  const seedPipeline = usePipelineStore((s) => s.seedPipeline);
  const refresh = usePipelineStore((s) => s.refresh);

  const [executions, setExecutions] = useState<Execution[]>([]);
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const authHeaders = async (): Promise<Record<string, string>> => {
    const token = await getToken();
    return token ? { Authorization: `Bearer ${token}` } : {};
  };

  const fetchExecutions = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/management/executions`, { headers: await authHeaders() });
      if (!res.ok) throw new Error(`${res.status}`);
      setExecutions(await res.json());
    } catch (e) {
      console.error("Failed to fetch executions:", e);
    }
  }, []);

  const fetchRuns = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/management/runs`, { headers: await authHeaders() });
      if (!res.ok) throw new Error(`${res.status}`);
      setRuns(await res.json());
    } catch (e) {
      console.error("Failed to fetch runs:", e);
    }
  }, []);

  const refreshAll = useCallback(async () => {
    setBusy("refresh");
    setError(null);
    await Promise.all([fetchExecutions(), fetchRuns()]);
    setBusy(null);
  }, [fetchExecutions, fetchRuns]);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  const stopExecution = async (arn: string, name: string) => {
    if (!window.confirm(`Stop execution "${name}"?`)) return;
    setBusy(arn);
    setError(null);
    try {
      const hdrs = await authHeaders();
      const res = await fetch(`${API_BASE}/management/executions/stop`, {
        method: "POST",
        headers: { "Content-Type": "application/json", ...hdrs },
        body: JSON.stringify({ execution_arn: arn }),
      });
      if (!res.ok) throw new Error(`${res.status}`);
      await refreshAll();
    } catch (e) {
      setError(`Failed to stop execution: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const deleteRun = async (runId: string) => {
    if (!window.confirm(`Delete run "${runId}" and all its data?`)) return;
    setBusy(runId);
    setError(null);
    try {
      const hdrs = await authHeaders();
      const res = await fetch(`${API_BASE}/management/runs/delete`, {
        method: "POST",
        headers: { "Content-Type": "application/json", ...hdrs },
        body: JSON.stringify({ run_id: runId }),
      });
      if (!res.ok) throw new Error(`${res.status}`);
      await Promise.all([refreshAll(), refresh()]);
    } catch (e) {
      setError(`Failed to delete run: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const handleNewRun = async () => {
    setBusy("new-run");
    setError(null);
    try {
      await runPipeline();
      await refreshAll();
    } catch (e) {
      setError(`Failed to start run: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const handleSeed = async () => {
    if (!window.confirm("Reset corpus and load seed data?")) return;
    setBusy("seed");
    setError(null);
    try {
      await seedPipeline();
      await refreshAll();
    } catch (e) {
      setError(`Failed to seed: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const runningCount = executions.filter((e) => e.status === "RUNNING").length;

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Pipeline Management</h2>
        <div className="view-desc">
          Manage Step Functions executions and pipeline runs
        </div>
      </div>

      {error && (
        <div className="panel stagger-in" style={{ borderLeft: "3px solid var(--critical)" }}>
          <div className="panel-body" style={{ color: "var(--critical)" }}>
            {error}
          </div>
        </div>
      )}

      <div className="metrics-row">
        <div className="metric-card stagger-in">
          <div className="metric-label">Running</div>
          <div className="metric-value">{runningCount}</div>
          <div className="metric-sub">executions</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Total Runs</div>
          <div className="metric-value">{runs.length}</div>
          <div className="metric-sub">in database</div>
        </div>
      </div>

      {/* ── Step Functions Executions ───────────────────────────────────── */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Step Functions Executions</h3>
          <button
            className="btn"
            onClick={refreshAll}
            disabled={busy === "refresh"}
          >
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
                      <Badge level={executionStatusLevel(ex.status)}>
                        {ex.status}
                      </Badge>
                    </td>
                    <td>{timeAgo(ex.start_date)}</td>
                    <td>
                      {ex.stop_date
                        ? `${Math.round((new Date(ex.stop_date).getTime() - new Date(ex.start_date).getTime()) / 1000)}s`
                        : "—"}
                    </td>
                    <td>
                      {ex.status === "RUNNING" && (
                        <button
                          className="btn btn-danger btn-sm"
                          onClick={() => stopExecution(ex.execution_arn, ex.name)}
                          disabled={busy === ex.execution_arn}
                        >
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

      {/* ── Pipeline Runs ──────────────────────────────────────────────── */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Pipeline Runs</h3>
          <div style={{ display: "flex", gap: "8px" }}>
            <button
              className="btn btn-accent"
              onClick={handleNewRun}
              disabled={busy === "new-run"}
            >
              {busy === "new-run" ? "Starting..." : "New Run"}
            </button>
            <button
              className="btn"
              onClick={handleSeed}
              disabled={busy === "seed"}
            >
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
                {runs.map((run) => {
                  const stageMap = new Map(
                    run.stages.map((s) => [s.stage, s.status]),
                  );
                  return (
                    <tr key={run.run_id}>
                      <td className="td-mono">{run.run_id}</td>
                      <td>{timeAgo(run.created_at)}</td>
                      <td>
                        <div className="stage-chips">
                          {STAGE_ORDER.map((n) => {
                            const status = stageMap.get(n);
                            return (
                              <span
                                key={n}
                                className={status ? stageStatusClass(status) : "stage-chip idle"}
                                title={`Stage ${n}: ${status || "not started"}`}
                              >
                                {n}
                              </span>
                            );
                          })}
                        </div>
                      </td>
                      <td className="td-muted">{run.notes || "—"}</td>
                      <td>
                        <button
                          className="btn btn-danger btn-sm"
                          onClick={() => deleteRun(run.run_id)}
                          disabled={busy === run.run_id}
                        >
                          {busy === run.run_id ? "..." : "Delete"}
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
    </div>
  );
}
