import { useState, useEffect, useCallback } from "react";
import {
  useSourceFeeds, useSourceCandidates,
  useFetchDiscovery, useRunDiscoveryFeeds, useReviewCandidate,
} from "../data/usePipelineSelectors";
import { useAsyncAction } from "../hooks";
import { ErrorBanner } from "../components/SharedUI";
import { Badge } from "../components/SharedUI";
import { ChevronDown, ChevronRight, ExternalLink } from "lucide-react";
import type { SourceCandidate } from "../types";

function statusBadge(status: string) {
  const level =
    status === "accepted" || status === "ingested"
      ? "low"
      : status === "rejected" || status === "error"
        ? "critical"
        : status === "scored"
          ? "medium"
          : "high";
  return <Badge level={level}>{status}</Badge>;
}

function richnessColor(score: number | null): string {
  if (score === null) return "var(--text-secondary)";
  if (score >= 0.7) return "var(--critical)";
  if (score >= 0.4) return "var(--high)";
  return "var(--text-secondary)";
}

export default function DiscoveryView() {
  const source_feeds = useSourceFeeds();
  const source_candidates = useSourceCandidates();
  const fetchDiscovery = useFetchDiscovery();
  const runDiscoveryFeeds = useRunDiscoveryFeeds();
  const reviewCandidate = useReviewCandidate();

  const { busy, error, run, clearError } = useAsyncAction();
  const [expandedFeed, setExpandedFeed] = useState<string | null>(null);
  const [filter, setFilter] = useState<string | null>(null);

  useEffect(() => {
    fetchDiscovery();
  }, [fetchDiscovery]);

  const handleRunFeeds = useCallback(
    () => run("feeds", runDiscoveryFeeds),
    [run, runDiscoveryFeeds],
  );

  const handleReview = useCallback(
    (candidateId: string, action: "accept" | "reject") =>
      run("review", () => reviewCandidate(candidateId, action)),
    [run, reviewCandidate],
  );

  const needsReview = source_candidates.filter((c) => c.status === "scored");
  const filtered = filter
    ? source_candidates.filter((c) => c.status === filter)
    : source_candidates;

  return (
    <div>
      <ErrorBanner error={error} onDismiss={clearError} />
      <div className="view-header stagger-in">
        <h2>Case Discovery</h2>
        <div className="view-desc">
          Automated monitoring of government enforcement feeds for new case sources.
        </div>
      </div>

      <div className="metrics-row">
        <div className="metric-card stagger-in">
          <div className="metric-label">Feeds</div>
          <div className="metric-value">{source_feeds.length}</div>
          <div className="metric-sub">configured</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Candidates</div>
          <div className="metric-value">{source_candidates.length}</div>
          <div className="metric-sub">discovered</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Review Queue</div>
          <div className="metric-value" style={{ color: needsReview.length > 0 ? "var(--high)" : undefined }}>
            {needsReview.length}
          </div>
          <div className="metric-sub">pending</div>
        </div>
        <div className="metric-card stagger-in">
          <div className="metric-label">Accepted</div>
          <div className="metric-value">
            {source_candidates.filter((c) => c.status === "accepted" || c.status === "ingested").length}
          </div>
          <div className="metric-sub">sources</div>
        </div>
      </div>

      {/* Feeds panel */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Source Feeds</h3>
          <button className="btn btn-accent" onClick={handleRunFeeds} disabled={!!busy}>
            {busy === "feeds" ? "Checking..." : "Check All Feeds"}
          </button>
        </div>
        <div className="panel-body dense">
          {source_feeds.length === 0 ? (
            <div className="empty-state">No feeds configured.</div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <th>Feed</th>
                  <th>Type</th>
                  <th>Last Checked</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {source_feeds.map((feed) => (
                  <tr
                    key={feed.feed_id}
                    className="clickable"
                    onClick={() => setExpandedFeed(expandedFeed === feed.feed_id ? null : feed.feed_id)}
                  >
                    <td className="td-name">
                      {expandedFeed === feed.feed_id ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                      {" "}{feed.name}
                    </td>
                    <td>{feed.content_type}</td>
                    <td>{feed.last_checked_at ? new Date(feed.last_checked_at).toLocaleDateString() : "Never"}</td>
                    <td>{feed.enabled ? <Badge level="low">Active</Badge> : <Badge level="medium">Disabled</Badge>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>

      {/* Candidates panel */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Discovered Candidates</h3>
        </div>

        <div className="filter-bar filter-bar-mb">
          <button className={`btn ${!filter ? "btn-accent" : ""}`} onClick={() => setFilter(null)}>
            All ({source_candidates.length})
          </button>
          <button className={`btn ${filter === "scored" ? "btn-accent" : ""}`} onClick={() => setFilter("scored")}>
            Review ({needsReview.length})
          </button>
          <button className={`btn ${filter === "accepted" ? "btn-accent" : ""}`} onClick={() => setFilter("accepted")}>
            Accepted
          </button>
          <button className={`btn ${filter === "rejected" ? "btn-accent" : ""}`} onClick={() => setFilter("rejected")}>
            Rejected
          </button>
        </div>

        <div className="panel-body dense">
          {filtered.length === 0 ? (
            <div className="empty-state">No candidates found. Run feed checks to discover new sources.</div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <th>Title</th>
                  <th>Richness</th>
                  <th>Est. Cases</th>
                  <th>Status</th>
                  <th className="hide-on-mobile">Actions</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((c) => (
                  <CandidateRow key={c.candidate_id} candidate={c} onReview={handleReview} />
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}

function CandidateRow({
  candidate,
  onReview,
}: {
  candidate: SourceCandidate;
  onReview: (id: string, action: "accept" | "reject") => void;
}) {
  const c = candidate;
  return (
    <tr>
      <td className="td-name">
        <a href={c.url} target="_blank" rel="noopener noreferrer">
          {c.title} <ExternalLink size={12} />
        </a>
      </td>
      <td>
        {c.richness_score !== null ? (
          <span style={{ color: richnessColor(c.richness_score), fontWeight: 600 }}>
            {c.richness_score.toFixed(2)}
          </span>
        ) : (
          "—"
        )}
      </td>
      <td>{c.estimated_cases ?? "—"}</td>
      <td>{statusBadge(c.status)}</td>
      <td className="hide-on-mobile">
        {c.status === "scored" && (
          <>
            <button className="btn" onClick={() => onReview(c.candidate_id, "accept")}>
              Accept
            </button>{" "}
            <button className="btn" onClick={() => onReview(c.candidate_id, "reject")}>
              Reject
            </button>
          </>
        )}
      </td>
    </tr>
  );
}
