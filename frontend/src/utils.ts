export function scoreLevel(score: number, threshold: number): string {
  if (score >= threshold + 2) return "critical";
  if (score >= threshold) return "high";
  if (score >= threshold - 1) return "medium";
  return "";
}

export function scoreColor(score: number, threshold: number): string {
  if (score >= threshold) return "var(--critical)";
  if (score >= threshold - 1) return "var(--high)";
  return "var(--text-secondary)";
}

export function formatDollars(n?: number | null): string {
  if (n == null) return "\u2014";
  if (n >= 1e9) return `$${(n / 1e9).toFixed(1)}B`;
  if (n >= 1e6) return `$${(n / 1e6).toFixed(0)}M`;
  if (n >= 1e3) return `$${(n / 1e3).toFixed(0)}K`;
  return `$${n}`;
}
