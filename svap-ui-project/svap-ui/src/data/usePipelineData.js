/**
 * usePipelineData — single data hook for the entire SVAP UI
 *
 * Strategy:
 *   1. On mount, tries to fetch /api/dashboard from the FastAPI server
 *   2. If the API is available, uses live pipeline data from SQLite
 *   3. If the API is unavailable (no backend running), falls back to
 *      the static seed data files — so the UI always works standalone
 *
 * The dashboard endpoint returns everything in one call (cases, taxonomy,
 * policies, predictions, patterns, convergence, calibration, HHS reference
 * data). Individual views don't need separate fetches.
 *
 * To refresh after a pipeline stage runs:
 *   call refresh() from any component
 */

import { useState, useEffect, useCallback, createContext, useContext } from 'react';

// Static fallbacks
import { SEED_CASES } from '../data/seedCases';
import { SEED_TAXONOMY } from '../data/seedTaxonomy';
import { SEED_POLICIES, CONVERGENCE_THRESHOLD } from '../data/seedPolicies';
import { SEED_PREDICTIONS, SEED_DETECTION_PATTERNS } from '../data/predictions';
import { ENFORCEMENT_SOURCES, HHS_DATA_SOURCES } from '../data/sources';
import { POLICY_CATALOG, SCANNED_PROGRAMS } from '../data/policyCatalog';

const API_BASE = '/api';

// Build the static fallback shape to match what the API returns
function buildStaticData() {
  // Build convergence from case qualities
  const caseConvergence = SEED_CASES.map(c => ({
    name: c.case_name,
    score: c.qualities.length,
    scale: c.scale_dollars,
    qualities: c.qualities,
  }));

  const policyConvergence = SEED_POLICIES.map(p => ({
    name: p.name,
    score: p.convergence_score,
    qualities: p.qualities,
  }));

  return {
    run_id: 'seed',
    source: 'static',
    pipeline_status: [
      { stage: 1, status: 'completed' },
      { stage: 2, status: 'completed' },
      { stage: 3, status: 'completed' },
      { stage: 4, status: 'completed' },
      { stage: 5, status: 'completed' },
      { stage: 6, status: 'completed' },
    ],
    counts: {
      cases: SEED_CASES.length,
      taxonomy_qualities: SEED_TAXONOMY.length,
      policies: SEED_POLICIES.length,
      predictions: SEED_PREDICTIONS.length,
      detection_patterns: SEED_DETECTION_PATTERNS.length,
    },
    calibration: { threshold: CONVERGENCE_THRESHOLD },
    cases: SEED_CASES,
    taxonomy: SEED_TAXONOMY,
    policies: SEED_POLICIES,
    predictions: SEED_PREDICTIONS,
    detection_patterns: SEED_DETECTION_PATTERNS,
    case_convergence: caseConvergence,
    policy_convergence: policyConvergence,
    // HHS reference data (always static — comes from hhs_extension.py constants)
    policy_catalog: POLICY_CATALOG,
    enforcement_sources: ENFORCEMENT_SOURCES,
    data_sources: HHS_DATA_SOURCES,
    scanned_programs: SCANNED_PROGRAMS,
  };
}


export function usePipelineData() {
  const [data, setData] = useState(() => buildStaticData());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [apiAvailable, setApiAvailable] = useState(false);

  const fetchDashboard = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const res = await fetch(`${API_BASE}/dashboard`, {
        signal: AbortSignal.timeout(3000), // 3s timeout — fail fast to static
      });

      if (!res.ok) throw new Error(`API returned ${res.status}`);

      const apiData = await res.json();
      setData({ ...apiData, source: 'api' });
      setApiAvailable(true);
    } catch (err) {
      // API unavailable — use static data (already set as default)
      console.info('SVAP API unavailable, using static seed data.', err.message);
      setData(buildStaticData());
      setApiAvailable(false);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchDashboard();
  }, [fetchDashboard]);

  // Pipeline operations (only work when API is available)
  const runStage = useCallback(async (stage) => {
    if (!apiAvailable) throw new Error('API not available');
    const res = await fetch(`${API_BASE}/pipeline/run`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ stage }),
    });
    if (!res.ok) throw new Error(`Run stage failed: ${res.status}`);
    return res.json();
  }, [apiAvailable]);

  const approveStage = useCallback(async (stage) => {
    if (!apiAvailable) throw new Error('API not available');
    const res = await fetch(`${API_BASE}/pipeline/approve`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ stage }),
    });
    if (!res.ok) throw new Error(`Approve failed: ${res.status}`);
    const result = await res.json();
    await fetchDashboard(); // Refresh after approval
    return result;
  }, [apiAvailable, fetchDashboard]);

  const seedPipeline = useCallback(async () => {
    if (!apiAvailable) throw new Error('API not available');
    const res = await fetch(`${API_BASE}/pipeline/seed`, { method: 'POST' });
    if (!res.ok) throw new Error(`Seed failed: ${res.status}`);
    const result = await res.json();
    await fetchDashboard();
    return result;
  }, [apiAvailable, fetchDashboard]);

  return {
    // Data
    ...data,
    threshold: data.calibration?.threshold ?? CONVERGENCE_THRESHOLD,

    // State
    loading,
    error,
    apiAvailable,
    source: data.source,

    // Actions
    refresh: fetchDashboard,
    runStage,
    approveStage,
    seedPipeline,
  };
}


// ── Context provider (optional — use if you want to avoid prop drilling) ──

const PipelineContext = createContext(null);

export function PipelineProvider({ children }) {
  const pipeline = usePipelineData();
  return (
    <PipelineContext.Provider value={pipeline}>
      {children}
    </PipelineContext.Provider>
  );
}

export function usePipeline() {
  const ctx = useContext(PipelineContext);
  if (!ctx) throw new Error('usePipeline must be used within PipelineProvider');
  return ctx;
}
