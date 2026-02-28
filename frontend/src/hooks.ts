import { useState, useCallback } from "react";

/**
 * Shared hook for async operations with busy/error tracking.
 *
 * Usage:
 *   const { busy, error, run, clearError } = useAsyncAction();
 *   <button disabled={!!busy} onClick={() => run("saving", async () => { ... })}>
 *   <ErrorBanner error={error} onDismiss={clearError} />
 */
export function useAsyncAction() {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (label: string, fn: () => Promise<unknown>) => {
    setBusy(label);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }, []);

  const clearError = useCallback(() => setError(null), []);

  return { busy, error, run, clearError };
}

/**
 * Toggle a value in a Set immutably. Shared by all multi-expand views.
 */
export function toggleSet(prev: Set<string>, id: string): Set<string> {
  const next = new Set(prev);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/**
 * Shared hook for multi-expand state (Set<string>-based toggle).
 * Used by tree views with multiple simultaneously expanded nodes.
 */
export function useExpandSet() {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const toggle = useCallback(
    (id: string) => setExpanded((prev) => toggleSet(prev, id)),
    [],
  );
  const reset = useCallback(() => setExpanded(new Set()), []);
  return { expanded, toggle, set: setExpanded, reset };
}

/**
 * Shared hook for single-expand state (only one item expanded at a time).
 * Used by card grids, detail panels, and single-expand tree views.
 */
export function useExpandSingle() {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const toggle = useCallback(
    (id: string) => setExpandedId((prev) => (prev === id ? null : id)),
    [],
  );
  return { expandedId, toggle };
}

/**
 * Returns props for a keyboard-accessible expandable element.
 * Handles onClick, onKeyDown (Enter/Space), role, and tabIndex.
 */
export function expandableProps(onToggle: () => void) {
  return {
    role: "button" as const,
    tabIndex: 0,
    onClick: onToggle,
    onKeyDown: (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onToggle();
      }
    },
  };
}
