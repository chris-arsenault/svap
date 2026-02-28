# Drift Audit Report
Generated: 2026-02-27
Project: SVAP (Structural Vulnerability Analysis Pipeline)
Source files scanned: 52 (25 frontend TS/TSX, 27 backend Python)
Drift areas found: 8

## Executive Summary

SVAP is a small but architecturally clean codebase with a few significant drift patterns. The highest-impact issue is a dead prototype codebase (`svap-ui-project/`) that should be deleted. In the active code, the main drift is in the frontend data access layer (4 different patterns for accessing the Zustand store) and backend configuration duplication (identical default config dict in two Lambda entry points). The backend stage implementations are remarkably consistent, but stages 5 and 6 duplicate a parallel execution helper that should be extracted.

## Priority Matrix
| # | Area | Impact | Type | Variants | Files Affected | Notes |
|---|------|--------|------|----------|----------------|-------|
| 1 | Dead Prototype Codebase | HIGH | structural | 2 | 11 | `svap-ui-project/` is unreferenced dead code |
| 2 | Frontend Store Access Patterns | HIGH | structural | 4 | 10 | Views use 4 different patterns to access Zustand store |
| 3 | Frontend Error/Loading Handling | HIGH | behavioral | 3 | 12 | 10/12 views have no error UI; loading state is duplicated |
| 4 | Backend Config Duplication | MEDIUM | structural | 2 | 2 | Identical default config in `api.py` and `stage_runner.py` |
| 5 | Parallel Execution Duplication | MEDIUM | semantic | 2 | 2 | Nearly identical `_run_parallel_*` in stages 5 and 6 |
| 6 | Frontend Inline Styles | MEDIUM | structural | 2 | 7 | 27 inline `style={{}}` usages across 7 views |
| 7 | Backend API Route Error Handling | LOW | behavioral | 3 | 1 | Inconsistent error/response patterns across routes |
| 8 | Frontend Auth Token Access | LOW | structural | 2 | 2 | ManagementView duplicates auth header logic |

## Detailed Findings

### 1. Dead Prototype Codebase
**Variants found:** 2 | **Impact:** HIGH | **Files affected:** 11

**Variant A: "Active TypeScript frontend" (25 files)**
- How it works: `frontend/` uses React 18 + TypeScript + Vite + Zustand + react-router-dom with Cognito auth
- Representative files: `frontend/src/App.tsx`, `frontend/src/data/pipelineStore.ts`
- Strengths: Modern architecture, type-safe, 12 views, auth integration, path-based routing

**Variant B: "Dead JSX prototype" (11 files)**
- How it works: `svap-ui-project/svap-ui/` uses React + plain JavaScript + Context API + state-based routing
- Representative files: `svap-ui-project/svap-ui/src/App.jsx`, `svap-ui-project/svap-ui/src/data/usePipelineData.js`
- Code excerpt:
  ```jsx
  // svap-ui-project/svap-ui/src/App.jsx — state-based routing (no URL support)
  const VIEWS = {
    dashboard: Dashboard,
    cases: CaseSourcing,
    policies: PolicyExplorer,
    // ...
  };
  const [activeView, setActiveView] = useState('dashboard');
  const ViewComponent = VIEWS[activeView] || Dashboard;
  ```
- Weaknesses: No TypeScript, no auth, no URL routing, only 7 views, static seed data only

**Analysis:** `svap-ui-project/` was the initial prototype committed on 2026-02-25 and immediately superseded by `frontend/` in the same commit. It has received zero updates since. It is not referenced by any build target, Makefile rule, or Terraform config. It contains duplicate shared UI components (Badge, ScoreBar, etc.) that have since diverged in `frontend/`. Its continued presence creates confusion about which codebase is canonical and inflates file counts for tooling.

**Recommendation:** Delete `svap-ui-project/` entirely. There is no code in it that isn't already present (improved) in `frontend/`. Command: `rm -rf svap-ui-project/`.

---

### 2. Frontend Store Access Patterns
**Variants found:** 4 | **Impact:** HIGH | **Files affected:** 10

**Variant A: `usePipelineStore(useShallow(...))` for data (6 views)**
- How it works: Destructures multiple store slices with `useShallow` for render optimization
- Representative files: `frontend/src/views/Dashboard.tsx:237-245`, `frontend/src/views/CaseSourcing.tsx`
- Code excerpt:
  ```tsx
  // frontend/src/views/Dashboard.tsx:237-245
  const { cases, taxonomy, policies, detection_patterns, threshold } = usePipelineStore(
    useShallow((s) => ({
      cases: s.cases,
      taxonomy: s.taxonomy,
      policies: s.policies,
      detection_patterns: s.detection_patterns,
      threshold: s.threshold,
    })),
  );
  ```
- Strengths: Avoids unnecessary re-renders, explicit about which slices are consumed

**Variant B: Direct inline selector `usePipelineStore((s) => s.field)` (3 views)**
- How it works: Accesses a single store field with an inline selector
- Representative files: `frontend/src/views/DetectionView.tsx:83`, `frontend/src/views/PredictionView.tsx:82`
- Code excerpt:
  ```tsx
  // frontend/src/views/DetectionView.tsx:83
  const detection_patterns = usePipelineStore((s) => s.detection_patterns);
  ```
- Strengths: Concise for single-field access
- Weaknesses: Multiple single-field calls in same component (e.g., ManagementView:81-83) miss `useShallow` optimization

**Variant C: Selector hooks from `usePipelineSelectors.ts` (1 view)**
- How it works: Pre-defined selector hooks that wrap `usePipelineStore`
- Representative files: `frontend/src/views/SourcesView.tsx:4,30-31`
- Code excerpt:
  ```tsx
  // frontend/src/views/SourcesView.tsx:4,30-31
  import { useUploadSourceDocument, useDeleteSource, useCreateSource } from "../data/usePipelineSelectors";
  const uploadSourceDocument = useUploadSourceDocument();
  const deleteSource = useDeleteSource();
  ```
- Strengths: Clean import, single responsibility per hook
- Weaknesses: Only used in 1 view despite 14 selectors being defined

**Variant D: Mixed data+actions in `useShallow` (2 views)**
- How it works: Extracts both data slices and action functions in one `useShallow` call
- Representative files: `frontend/src/views/ResearchView.tsx:29-40`, `frontend/src/views/DiscoveryView.tsx:28-36`
- Code excerpt:
  ```tsx
  // frontend/src/views/ResearchView.tsx:29-40
  const { triage_results, research_sessions, policies,
    fetchTriageResults, fetchResearchSessions, runTriage,
    runDeepResearch, fetchFindings, fetchAssessments,
  } = usePipelineStore(useShallow((s) => ({ ... })));
  ```
- Weaknesses: Actions are stable references — wrapping them in `useShallow` is unnecessary and adds overhead

**Analysis:** The project invested in building a proper selector layer (`usePipelineSelectors.ts` with 14 hooks) but only 1 of 12 views actually uses it. The remaining views use 3 different ad-hoc patterns. This creates cognitive overhead — new views have no clear template for how to access store data. The `useShallow` approach is correct for multi-field data access but should not wrap action functions (which are already stable).

**Recommendation:** Converge on two patterns: (1) `useShallow` for multi-field data access, (2) selector hooks for action access. Update `usePipelineSelectors.ts` to also export data selectors grouped by view concern (e.g., `useDashboardData()`, `useTaxonomyData()`). Migrate all 12 views to use these consistently. Target interface:
```tsx
// Data: grouped selector hook with useShallow internally
const { cases, taxonomy, threshold } = useDashboardData();
// Actions: individual selector hooks
const runPipeline = useRunPipeline();
const approveStage = useApproveStage();
```

---

### 3. Frontend Error/Loading Handling
**Variants found:** 3 | **Impact:** HIGH | **Files affected:** 12

**Variant A: "No error UI" (10 views)**
- How it works: Async operations catch errors but only log to `console.error()` with no user feedback
- Representative files: `frontend/src/views/SourcesView.tsx:44`, `frontend/src/views/DiscoveryView.tsx:50-53`, `frontend/src/views/ResearchView.tsx:64,73`
- Code excerpt:
  ```tsx
  // frontend/src/views/SourcesView.tsx:44 — error swallowed to console
  } catch (err) {
    console.error("Upload failed:", err);
  }
  ```

**Variant B: "Inline error display" (1 view: ManagementView)**
- How it works: Stores error in local `useState`, renders a styled error panel
- Representative files: `frontend/src/views/ManagementView.tsx:204-210`
- Code excerpt:
  ```tsx
  // frontend/src/views/ManagementView.tsx:204-210
  {error && (
    <div className="panel stagger-in" style={{ borderLeft: "3px solid var(--critical)" }}>
      <div className="panel-body" style={{ color: "var(--critical)" }}>
        {error}
      </div>
    </div>
  )}
  ```

**Variant C: "Global API gate" (1 component: App.tsx)**
- How it works: `ApiGate` wrapper shows global loading/error from store selectors
- Representative files: `frontend/src/App.tsx:153-176`
- Code excerpt:
  ```tsx
  // frontend/src/App.tsx:153-176
  function ApiGate({ children }: { children: React.ReactNode }) {
    const loading = useLoading();
    const error = useError();
    const refresh = useRefresh();
    // ... renders global loading or error overlay
  }
  ```

**Analysis:** The global `ApiGate` handles initial load errors, but once the app is running, per-view async operations (upload, seed, approve, fetch) silently swallow errors. Users get no feedback when operations fail. Meanwhile, each view re-invents local `[busy, setBusy]` state instead of using the store's `loading`/`error` selectors that already exist. The ManagementView is the only view with inline error display, but it uses ad-hoc inline styles rather than a shared error component.

**Recommendation:** Create a shared `<ErrorBanner message={error} />` component and a `useAsyncAction()` hook that wraps try/catch with local busy+error state:
```tsx
function useAsyncAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (fn: () => Promise<void>) => {
    setBusy(true); setError(null);
    try { await fn(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }, []);
  return { busy, error, run, clearError: () => setError(null) };
}
```

---

### 4. Backend Config Duplication
**Variants found:** 2 | **Impact:** MEDIUM | **Files affected:** 2

**Variant A: Module-level dict in `api.py`**
- Representative files: `backend/src/svap/api.py:52-72`
- Code excerpt:
  ```python
  # backend/src/svap/api.py:52-72
  _DEFAULT_CONFIG = {
      "bedrock": {
          "region": "us-east-1",
          "model_id": "us.anthropic.claude-sonnet-4-6",
          "max_tokens": 4096,
          "temperature": 0.2,
          "retry_attempts": 3,
          "retry_delay_seconds": 5,
      },
      "rag": { "chunk_size": 1500, "chunk_overlap": 200, ... },
      "pipeline": { "human_gates": [2, 5], "max_concurrency": 5, ... },
  }
  ```

**Variant B: Factory function in `stage_runner.py`**
- Representative files: `backend/src/svap/stage_runner.py:134-156`
- Code excerpt:
  ```python
  # backend/src/svap/stage_runner.py:134-156
  def _default_config() -> dict:
      """Minimal default config when no config file is available."""
      return {
          "bedrock": { ... },  # identical values
          "rag": { ... },
          "pipeline": { ... },
      }
  ```

**Analysis:** Both contain identical values today but are maintained independently. If the model ID changes (it already changed to `us.anthropic.claude-sonnet-4-6`), both must be updated. The `api.py` version is a module-level dict (mutable, shared), while `stage_runner.py` returns a fresh dict from a function (safer). Neither references the other despite being in the same package.

**Recommendation:** Extract to a shared function in a new `svap/defaults.py` or add to existing `svap/__init__.py`:
```python
# svap/defaults.py
def default_config() -> dict:
    return { "bedrock": {...}, "rag": {...}, "pipeline": {...} }
```
Both `api.py` and `stage_runner.py` import from this single source.

---

### 5. Parallel Execution Duplication
**Variants found:** 2 | **Impact:** MEDIUM | **Files affected:** 2

**Variant A: `_run_parallel_predictions` in stage5**
- Representative files: `backend/src/svap/stages/stage5_prediction.py:104-131`
- Code excerpt:
  ```python
  # backend/src/svap/stages/stage5_prediction.py:104-131
  def _run_parallel_predictions(storage, client, run_id, jobs, max_concurrency):
      total_predictions = 0
      failed_policies = []
      with ThreadPoolExecutor(max_workers=max_concurrency) as executor:
          future_to_policy = {
              executor.submit(_invoke_llm, client, prompt): (policy_id, profile, h)
              for policy_id, profile, h, prompt in jobs
          }
          for future in as_completed(future_to_policy):
              policy_id, profile, h = future_to_policy[future]
              try:
                  result = future.result()
                  count = _store_predictions(storage, run_id, policy_id, profile, result)
                  total_predictions += count
              except Exception as exc:
                  failed_policies.append(policy_id)
                  print(f"  ERROR for {policy_id}: {exc}")
      return total_predictions, failed_policies
  ```

**Variant B: `_run_parallel_detection` in stage6**
- Representative files: `backend/src/svap/stages/stage6_detection.py:98-125`
- Code excerpt:
  ```python
  # backend/src/svap/stages/stage6_detection.py:98-125
  def _run_parallel_detection(storage, client, run_id, jobs, max_concurrency):
      total_patterns = 0
      failed_predictions = []
      with ThreadPoolExecutor(max_workers=max_concurrency) as executor:
          future_to_pred = {
              executor.submit(_invoke_llm, client, prompt): (pred, h)
              for pred, h, prompt in jobs
          }
          for future in as_completed(future_to_pred):
              pred, h = future_to_pred[future]
              try:
                  result = future.result()
                  count = _store_patterns(storage, run_id, pred, result)
                  total_patterns += count
              except Exception as exc:
                  failed_predictions.append(pred["prediction_id"])
                  print(f"  ERROR for {pred['prediction_id']}: {exc}")
      return total_patterns, failed_predictions
  ```

**Analysis:** These two functions are structurally identical — both create a ThreadPoolExecutor, submit LLM invocations, collect results via `as_completed`, count successes, and track failures. The only differences are: (1) the shape of job tuples, (2) the store function called (`_store_predictions` vs `_store_patterns`), and (3) the variable names. This is classic copy-paste drift that happens when a new stage is built by copying a previous one.

**Recommendation:** Extract a generic `run_parallel_llm(client, jobs, store_fn, max_concurrency)` utility:
```python
# svap/parallel.py
def run_parallel_llm(client, jobs, store_fn, max_concurrency, label="item"):
    """Execute LLM calls in parallel. jobs: [(key, prompt, *context)]. store_fn(result, *context) -> count."""
    total = 0
    failed = []
    with ThreadPoolExecutor(max_workers=max_concurrency) as executor:
        futures = {executor.submit(_invoke_llm, client, prompt): (key, *ctx) for key, prompt, *ctx in jobs}
        for future in as_completed(futures):
            key, *ctx = futures[future]
            try:
                total += store_fn(future.result(), *ctx)
            except Exception as exc:
                failed.append(key)
                print(f"  ERROR for {key}: {exc}")
    return total, failed
```

---

### 6. Frontend Inline Styles
**Variants found:** 2 | **Impact:** MEDIUM | **Files affected:** 7

**Variant A: CSS classes (canonical pattern)**
- How it works: Views use class names defined in `frontend/src/index.css`
- Representative files: `frontend/src/views/Dashboard.tsx`, `frontend/src/views/CaseSourcing.tsx`
- Code excerpt:
  ```tsx
  // frontend/src/views/Dashboard.tsx — uses CSS classes
  <section className="view-section">
    <div className="panel stagger-in">
      <div className="panel-header"><h2>Pipeline</h2></div>
      <div className="panel-body">
  ```

**Variant B: Inline `style={{}}` objects (27 instances in 7 views)**
- How it works: Ad-hoc styles passed as React style props
- Representative files: `frontend/src/views/ResearchView.tsx` (8 instances), `frontend/src/views/DimensionRegistryView.tsx` (6 instances)
- Code excerpt:
  ```tsx
  // frontend/src/views/ResearchView.tsx:162-168
  <span style={{
    color: t.triage_score >= 0.7 ? "var(--critical)" : t.triage_score >= 0.4 ? "var(--high)" : undefined,
    fontWeight: 600,
  }}>

  // frontend/src/views/DimensionRegistryView.tsx:83-86
  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
  ```
- Weaknesses: Creates new objects on every render, inconsistent with CSS-class approach used elsewhere

**Analysis:** The project uses CSS custom properties (e.g., `var(--critical)`, `var(--border)`) and has a well-structured `index.css` with utility classes. However, 7 views bypass this system with inline styles for layout (flex/gap), spacing (margins/padding), and conditional colors. Some inline styles inject CSS custom properties via `style={{ "--q-color": quality.color } as React.CSSProperties}` which is a legitimate technique for dynamic theming. The layout and spacing styles, however, could all be CSS classes.

**Recommendation:** Add utility classes to `index.css` for the common inline patterns:
```css
.flex-row { display: flex; align-items: center; gap: 0.5rem; }
.flex-wrap { display: flex; flex-wrap: wrap; gap: 0.25rem; }
.score-critical { color: var(--critical); font-weight: 600; }
.score-high { color: var(--high); font-weight: 600; }
```
Keep CSS variable injection via inline styles (it's the correct pattern for dynamic theming).

---

### 7. Backend API Route Error Handling
**Variants found:** 3 | **Impact:** LOW | **Files affected:** 1

**Variant A: Active run required (raises 404)**
- Code excerpt:
  ```python
  # backend/src/svap/api.py — routes using get_active_run_id()
  def _status(event):
      storage = get_storage()
      run_id = get_active_run_id(storage)  # Raises ApiError(404) if none
      return {"run_id": run_id, "stages": storage.get_pipeline_status(run_id)}
  ```

**Variant B: Graceful fallback (returns empty data)**
- Code excerpt:
  ```python
  # backend/src/svap/api.py — routes using get_latest_run or ""
  def _dashboard(event):
      storage = get_storage()
      run_id = storage.get_latest_run() or ""
      return get_dashboard_data(storage, run_id)
  ```

**Variant C: Path parameter access without guard**
- Code excerpt:
  ```python
  # backend/src/svap/api.py — direct dict access
  def _get_case(event):
      case_id = event["pathParameters"]["case_id"]  # Could KeyError
  ```

**Analysis:** Routes have 3 different strategies for handling missing data: strict 404 (correct for specific-resource endpoints), graceful fallback (correct for dashboard), and unguarded access (could produce a raw 500 instead of a proper 400). The last variant is the actual bug — path parameter access should use `.get()` with a guard.

**Recommendation:** Wrap path parameter access:
```python
def _path_param(event, name):
    val = (event.get("pathParameters") or {}).get(name)
    if not val:
        raise ApiError(400, f"Missing path parameter: {name}")
    return val
```

---

### 8. Frontend Auth Token Access
**Variants found:** 2 | **Impact:** LOW | **Files affected:** 2

**Variant A: Store-centralized auth (11 views)**
- How it works: API calls go through `pipelineStore.ts` `apiGet()`/`apiPost()` which handle auth headers
- Representative files: `frontend/src/data/pipelineStore.ts:38-49`

**Variant B: Direct `getToken()` (ManagementView only)**
- How it works: ManagementView builds its own `authHeaders()` helper and calls `fetch()` directly
- Representative files: `frontend/src/views/ManagementView.tsx:90-93`
- Code excerpt:
  ```tsx
  // frontend/src/views/ManagementView.tsx:90-93
  const authHeaders = async (): Promise<Record<string, string>> => {
    const token = await getToken();
    return token ? { Authorization: `Bearer ${token}` } : {};
  };
  ```

**Analysis:** ManagementView accesses endpoints (`/api/management/executions`, `/api/management/runs`) that aren't wired through the Zustand store's actions. Rather than adding these to the store, it built its own fetch+auth layer. This is pragmatic but creates a second auth code path that could diverge (e.g., if token refresh logic changes).

**Recommendation:** Either add management API calls to `pipelineStore.ts`, or extract `apiGet`/`apiPost` from the store into a standalone `api.ts` utility that both the store and ManagementView can import.

---

## Behavioral Findings

### B1. View Loading State Management
**Domain:** Loading & Error State Patterns (Domain 4)
**Variants found:** 3 | **Impact:** HIGH | **Files affected:** 12

**Behavior Matrix:**

| View | Local busy state | Store loading | Error display | Error handling |
|------|:---|:---|:---|:---|
| Dashboard | `useState<string\|null>` | via ApiGate only | none | swallowed |
| SourcesView | `useState(false)` | — | none | `console.error` |
| CaseSourcing | none | — | none | none |
| TaxonomyView | none | — | none | none |
| ConvergenceMatrix | none | — | none | none |
| PolicyExplorer | none | — | none | none |
| PredictionView | none | — | none | none |
| DetectionView | none | — | none | none |
| ManagementView | `useState<string\|null>` + `useState<string\|null>` (error) | — | inline panel | try/catch per action |
| DiscoveryView | `useState(false)` | — | none | `catch` swallowed |
| ResearchView | `useState(false)` | — | none | `catch` swallowed |
| DimensionRegistryView | `useState(false)` | — | none | none |

**Analysis:** This is genuine drift, not intentional variation. The store already exports `useLoading()` and `useError()` selectors, but only `ApiGate` in App.tsx uses them. Views independently re-create busy state tracking with different types (`boolean` vs `string | null`). The `string | null` pattern (Dashboard, ManagementView) enables tracking *which* action is busy, while `boolean` (SourcesView, DiscoveryView, ResearchView) is a simpler flag. Neither pattern surfaces errors to the user (except ManagementView). This means users get silent failures for uploads, pipeline runs, and approvals.

---

### B2. Stage Signature Consistency
**Domain:** Multi-Step Workflow Consistency (Domain 3)
**Variants found:** 2 | **Impact:** LOW | **Files affected:** 12

**Behavior Matrix:**

| Stage | Signature | Has `run_id` |
|-------|-----------|:---:|
| stage0_source_fetch | `run(storage, client, run_id, config)` | yes |
| stage0a_discovery | `run(storage, client, config)` | **no** |
| stage1_case_assembly | `run(storage, client, run_id, config)` | yes |
| stage2_taxonomy | `run(storage, client, run_id, config)` | yes |
| stage3_scoring | `run(storage, client, run_id, config)` | yes |
| stage4_scanning | `run(storage, client, run_id, config)` | yes |
| stage4a_triage | `run(storage, client, run_id, config)` | yes |
| stage4b_research | `run(storage, client, run_id, config, policy_ids=None)` | yes (+extra) |
| stage4c_assessment | `run(storage, client, run_id, config)` | yes |
| stage5_prediction | `run(storage, client, run_id, config)` | yes |
| stage6_detection | `run(storage, client, run_id, config)` | yes |

**Analysis:** This is intentional variation, not drift. `stage0a_discovery` operates independently of pipeline runs (writes to global tables), so it correctly omits `run_id`. `stage4b_research` accepts optional `policy_ids` for targeted research. These signature differences reflect genuine architectural distinctions documented in CLAUDE.md.

---

## Semantic Findings

### S1. Parallel LLM Execution Pattern
**Functional role:** "Execute batch of LLM calls concurrently, collect results, track failures"

Two independent implementations serve the exact same purpose with different variable names:
- `_run_parallel_predictions()` in `stage5_prediction.py:104-131`
- `_run_parallel_detection()` in `stage6_detection.py:98-125`

Both create a `ThreadPoolExecutor`, submit via `_invoke_llm`, iterate with `as_completed`, count successes, track failures by ID, and return `(total, failed)`. The structural similarity is near-total — only the job tuple shape and store function differ.

**Consolidation complexity:** LOW — a generic wrapper parameterized by store function would replace both.

### S2. Store Selector Abstraction
**Functional role:** "Provide granular access to Zustand store slices"

The project has `usePipelineSelectors.ts` with 14 pre-defined selector hooks, but 11/12 views bypass them entirely. This isn't two implementations of the same concept — it's an abstraction that was built but not adopted. The selectors represent the intended canonical pattern; the direct store access in views is organic drift away from it.

**Consolidation complexity:** LOW — mechanical migration of imports in each view file.

---

## Quick Wins
1. **Delete `svap-ui-project/`** — zero-effort, removes 11 dead files and eliminates confusion
2. **Extract `default_config()` to shared module** — 15-minute change, eliminates sync risk
3. **Wrap path parameters in `api.py`** — 5-minute change, prevents raw 500 errors

## Questions for the Team
1. **ManagementView's direct fetch pattern** — Is there a reason its API calls aren't in the Zustand store? Should management endpoints be added to `pipelineStore.ts`?
2. **Inline CSS variables** — The `style={{ "--q-color": quality.color } as React.CSSProperties}` pattern is used for dynamic theming. Is this intentional, or should these be data attributes + CSS selectors?
3. **Stage 4 sub-stages** — stage4a/4b/4c are invoked as sub-stages. Should they have their own `log_stage_start/complete` lifecycle, or should stage4 manage them internally?
