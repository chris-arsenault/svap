import React, { useState, useEffect, useCallback } from "react";
import {
  useSourceFeeds, useSourceCandidates,
  useFetchDiscovery, useRunDiscoveryFeeds, useReviewCandidate,
} from "../data/usePipelineSelectors";
import { useAsyncAction, useExpandSingle } from "../hooks";
import { ErrorBanner, Badge, ViewHeader, MetricCard } from "../components/SharedUI";
import { ChevronDown, ChevronRight, ExternalLink } from "lucide-react";
import type { RiskLevel, SourceCandidate, SourceFeed } from "../types";

const STATUS_LEVELS: Record<string, RiskLevel> = {
  accepted: "low",
  ingested: "low",
  rejected: "critical",
  error: "critical",
  scored: "medium",
};

function statusBadge(status: string) {
  const level: RiskLevel = STATUS_LEVELS[status] ?? "high";
  return <Badge level={level}>{status}</Badge>;
}

function richnessColor(score: number | null): string {
  if (score === null) return "var(--text-secondary)";
  if (score >= 0.7) return "var(--critical)";
  if (score >= 0.4) return "var(--high)";
  return "var(--text-secondary)";
}

function DiscoveryMetrics({ feedCount, candidateCount, reviewCount, acceptedCount }: {
  feedCount: number; candidateCount: number; reviewCount: number; acceptedCount: number;
}) {
  return (
    <div className="metrics-row">
      <MetricCard label="Feeds" value={feedCount} sub="configured" />
      <MetricCard label="Candidates" value={candidateCount} sub="discovered" />
      <MetricCard label="Review Queue" value={<span className={reviewCount > 0 ? "text-high" : ""}>{reviewCount}</span>} sub="pending" />
      <MetricCard label="Accepted" value={acceptedCount} sub="sources" />
    </div>
  );
}

export default function DiscoveryView() {
  const source_feeds = useSourceFeeds();
  const source_candidates = useSourceCandidates();
  const fetchDiscovery = useFetchDiscovery();
  const runDiscoveryFeeds = useRunDiscoveryFeeds();
  const reviewCandidate = useReviewCandidate();

  const { busy, error, run, clearError } = useAsyncAction();
  const { expandedId: expandedFeed, toggle: toggleFeed } = useExpandSingle();
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

  return (
    <div>
      <ErrorBanner error={error} onDismiss={clearError} />
      <ViewHeader title="Case Discovery" description="Automated monitoring of government enforcement feeds for new case sources." />

      <DiscoveryMetrics
        feedCount={source_feeds.length}
        candidateCount={source_candidates.length}
        reviewCount={needsReview.length}
        acceptedCount={source_candidates.filter((c) => c.status === "accepted" || c.status === "ingested").length}
      />

      {/* Feeds panel */}
      <div className="panel stagger-in">
        <div className="panel-header">
          <h3>Source Feeds</h3>
          <button className="btn btn-accent" onClick={handleRunFeeds} disabled={!!busy}>
            {busy === "feeds" ? "Checking..." : "Check All Feeds"}
          </button>
        </div>
        <div className="panel-body dense">
          <FeedsTable
            feeds={source_feeds}
            expandedFeed={expandedFeed}
            onToggleFeed={toggleFeed}
          />
        </div>
      </div>

      <CandidatesPanel
        candidates={source_candidates}
        needsReview={needsReview}
        filter={filter}
        setFilter={setFilter}
        onReview={handleReview}
      />
    </div>
  );
}

function FeedsTable({
  feeds,
  expandedFeed,
  onToggleFeed,
}: {
  feeds: SourceFeed[];
  expandedFeed: string | null;
  onToggleFeed: (id: string) => void;
}) {
  if (feeds.length === 0) {
    return <div className="empty-state">No feeds configured.</div>;
  }
  return (
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
        {feeds.map((feed) => (
          <tr
            key={feed.feed_id}
            className="clickable"
            onClick={() => onToggleFeed(feed.feed_id)}
            tabIndex={0}
            onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onToggleFeed(feed.feed_id); } }}
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
  );
}

function CandidatesPanel({
  candidates,
  needsReview,
  filter,
  setFilter,
  onReview,
}: {
  candidates: SourceCandidate[];
  needsReview: SourceCandidate[];
  filter: string | null;
  setFilter: (f: string | null) => void;
  onReview: (id: string, action: "accept" | "reject") => void;
}) {
  const filtered = filter ? candidates.filter((c) => c.status === filter) : candidates;
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Discovered Candidates</h3>
      </div>
      <div className="filter-bar filter-bar-mb">
        <button className={`btn ${!filter ? "btn-accent" : ""}`} onClick={() => setFilter(null)}>
          All ({candidates.length})
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
                <CandidateRow key={c.candidate_id} candidate={c} onReview={onReview} />
              ))}
            </tbody>
          </table>
        )}
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
          <span className="score-highlight" style={{ '--score-color': richnessColor(c.richness_score) } as React.CSSProperties}>
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
