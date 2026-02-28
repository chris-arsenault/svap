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
