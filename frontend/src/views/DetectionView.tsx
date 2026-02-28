import React, { useMemo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useDetectionPatterns } from "../data/usePipelineSelectors";
import { useExpandSet, expandableProps } from "../hooks";
import { Badge, ViewHeader } from "../components/SharedUI";
import type { DetectionPattern, RiskLevel } from "../types";

const PRIORITY_ORDER: RiskLevel[] = ["critical", "high", "medium", "low"];

function priorityRank(p: RiskLevel): number {
  return PRIORITY_ORDER.indexOf(p);
}

// ── Grouping types ───────────────────────────────────────────────────────

interface StepGroup {
  step_id: string;
  step_title: string;
  patterns: DetectionPattern[];
  highestPriority: RiskLevel;
}

interface TreeGroup {
  tree_id: string;
  policy_name: string;
  steps: StepGroup[];
  patternCount: number;
  priorityCounts: Record<RiskLevel, number>;
}

function buildTreeGroups(patterns: DetectionPattern[]): TreeGroup[] {
  const treeMap = new Map<
    string,
    { policy_name: string; stepMap: Map<string, { step_title: string; patterns: DetectionPattern[] }> }
  >();

  for (const pat of patterns) {
    let tree = treeMap.get(pat.tree_id);
    if (!tree) {
      tree = { policy_name: pat.policy_name, stepMap: new Map() };
      treeMap.set(pat.tree_id, tree);
    }
    let step = tree.stepMap.get(pat.step_id);
    if (!step) {
      step = { step_title: pat.step_title, patterns: [] };
      tree.stepMap.set(pat.step_id, step);
    }
    step.patterns.push(pat);
  }

  const groups: TreeGroup[] = [];
  for (const [tree_id, tree] of treeMap) {
    const steps: StepGroup[] = [];
    const priorityCounts: Record<RiskLevel, number> = { critical: 0, high: 0, medium: 0, low: 0 };
    let patternCount = 0;

    for (const [step_id, stepData] of tree.stepMap) {
      stepData.patterns.sort((a, b) => priorityRank(a.priority) - priorityRank(b.priority));
      const highestPriority = stepData.patterns[0]?.priority ?? "low";
      steps.push({ step_id, step_title: stepData.step_title, patterns: stepData.patterns, highestPriority });
      patternCount += stepData.patterns.length;
      for (const p of stepData.patterns) {
        priorityCounts[p.priority]++;
      }
    }

    steps.sort((a, b) => {
      const d = priorityRank(a.highestPriority) - priorityRank(b.highestPriority);
      return d !== 0 ? d : b.patterns.length - a.patterns.length;
    });

    groups.push({ tree_id, policy_name: tree.policy_name, steps, patternCount, priorityCounts });
  }

  groups.sort((a, b) => {
    const aTop = a.steps[0]?.highestPriority ?? "low";
    const bTop = b.steps[0]?.highestPriority ?? "low";
    const d = priorityRank(aTop) - priorityRank(bTop);
    return d !== 0 ? d : b.patternCount - a.patternCount;
  });

  return groups;
}

// ── Level 3: Pattern block ───────────────────────────────────────────────

function PatternBlock({
  pat,
  isExpanded,
  onToggle,
}: {
  pat: DetectionPattern;
  isExpanded: boolean;
  onToggle: (id: string) => void;
}) {
  return (
    <div
      className={`detection-pattern${isExpanded ? " expanded" : ""}`}
      {...expandableProps(() => onToggle(pat.pattern_id))}
    >
      <div className="detection-pattern-header">
        <Badge level={pat.priority}>{pat.priority}</Badge>
        <div className="detection-pattern-signal">{pat.anomaly_signal}</div>
      </div>
      {!isExpanded && (
        <div className="detection-pattern-source">{pat.data_source}</div>
      )}
      {isExpanded && (
        <div className="detection-pattern-detail">
          <div className="detection-detail-field">
            <div className="detection-detail-label">Data Source</div>
            <div className="detection-detail-value accent">{pat.data_source}</div>
          </div>
          {pat.detection_latency && (
            <div className="detection-detail-field">
              <div className="detection-detail-label">Detection Latency</div>
              <div className="detection-detail-value">{pat.detection_latency}</div>
            </div>
          )}
          {pat.baseline && (
            <div className="detection-detail-field">
              <div className="detection-detail-label">Baseline</div>
              <div className="detection-detail-value">{pat.baseline}</div>
            </div>
          )}
          {pat.false_positive_risk && (
            <div className="detection-detail-field">
              <div className="detection-detail-label">False Positive Risk</div>
              <div className="detection-detail-value risk">{pat.false_positive_risk}</div>
            </div>
          )}
          {pat.implementation_notes && (
            <div className="detection-detail-field">
              <div className="detection-detail-label">Implementation Notes</div>
              <div className="detection-detail-value">{pat.implementation_notes}</div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Level 2: Step row ────────────────────────────────────────────────────

function StepRow({
  step,
  isExpanded,
  onToggle,
  expandedPatterns,
  onTogglePattern,
}: {
  step: StepGroup;
  isExpanded: boolean;
  onToggle: (id: string) => void;
  expandedPatterns: Set<string>;
  onTogglePattern: (id: string) => void;
}) {
  return (
    <div className="detection-step">
      <div
        className="detection-step-content"
        {...expandableProps(() => onToggle(step.step_id))}
      >
        <div className="detection-step-header">
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <span className="detection-step-title">{step.step_title}</span>
          <Badge level={step.highestPriority}>{step.highestPriority}</Badge>
          <span className="detection-step-count">{step.patterns.length}</span>
        </div>
      </div>
      {isExpanded && (
        <div className="detection-patterns">
          {step.patterns.map((pat) => (
            <PatternBlock
              key={pat.pattern_id}
              pat={pat}
              isExpanded={expandedPatterns.has(pat.pattern_id)}
              onToggle={onTogglePattern}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ── Level 1: Tree panel ──────────────────────────────────────────────────

function TreePanel({
  group,
  isExpanded,
  onToggle,
  expandedSteps,
  onToggleStep,
  expandedPatterns,
  onTogglePattern,
}: {
  group: TreeGroup;
  isExpanded: boolean;
  onToggle: (id: string) => void;
  expandedSteps: Set<string>;
  onToggleStep: (id: string) => void;
  expandedPatterns: Set<string>;
  onTogglePattern: (id: string) => void;
}) {
  const { priorityCounts } = group;

  return (
    <div className="panel stagger-in">
      <div
        className="panel-header clickable"
        {...expandableProps(() => onToggle(group.tree_id))}
      >
        <div className="detection-header-left">
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <h3 className="detection-policy-name">{group.policy_name}</h3>
        </div>
        <div className="detection-header-right">
          <div className="detection-priority-pills">
            {PRIORITY_ORDER.map((p) =>
              priorityCounts[p] > 0 ? (
                <span key={p} className={`detection-priority-pill ${p}`}>
                  {priorityCounts[p]} {p}
                </span>
              ) : null,
            )}
          </div>
          <span className="tree-header-stat">
            {group.patternCount} pattern{group.patternCount !== 1 ? "s" : ""}
          </span>
          <span className="tree-header-stat">
            {group.steps.length} step{group.steps.length !== 1 ? "s" : ""}
          </span>
        </div>
      </div>

      {isExpanded && (
        <div className="panel-body panel-body-bordered">
          <div className="detection-steps">
            {group.steps.map((step) => (
              <StepRow
                key={step.step_id}
                step={step}
                isExpanded={expandedSteps.has(step.step_id)}
                onToggle={onToggleStep}
                expandedPatterns={expandedPatterns}
                onTogglePattern={onTogglePattern}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Root ──────────────────────────────────────────────────────────────────

export default function DetectionView() {
  const detection_patterns = useDetectionPatterns();
  const [filterPriority, setFilterPriority] = React.useState<RiskLevel | null>(null);
  const { expanded: expandedTrees, toggle: toggleTree, set: setExpandedTrees } = useExpandSet();
  const { expanded: expandedSteps, toggle: toggleStep, set: setExpandedSteps } = useExpandSet();
  const { expanded: expandedPatterns, toggle: togglePattern, reset: resetPatterns } = useExpandSet();

  const filtered = filterPriority
    ? detection_patterns.filter((p) => p.priority === filterPriority)
    : detection_patterns;

  const treeGroups = useMemo(() => buildTreeGroups(filtered), [filtered]);

  // Unfiltered counts for filter pills
  const counts: Partial<Record<RiskLevel, number>> = {};
  detection_patterns.forEach((p) => {
    counts[p.priority] = (counts[p.priority] || 0) + 1;
  });

  const expandAll = React.useCallback(() => {
    setExpandedTrees(new Set(treeGroups.map((g) => g.tree_id)));
    setExpandedSteps(new Set(treeGroups.flatMap((g) => g.steps.map((s) => s.step_id))));
  }, [treeGroups, setExpandedTrees, setExpandedSteps]);

  const collapseAll = React.useCallback(() => {
    setExpandedTrees(new Set());
    setExpandedSteps(new Set());
    resetPatterns();
  }, [setExpandedTrees, setExpandedSteps, resetPatterns]);

  return (
    <div>
      <ViewHeader
        title="Detection Patterns"
        description={<>{detection_patterns.length} anomaly signals across {treeGroups.length} polic{treeGroups.length === 1 ? "y" : "ies"}</>}
      />

      <div className="filter-bar filter-bar-mb stagger-in">
        <button
          className={`btn ${!filterPriority ? "btn-accent" : ""}`}
          onClick={() => setFilterPriority(null)}
        >
          All ({detection_patterns.length})
        </button>
        {PRIORITY_ORDER.map((p) =>
          counts[p] ? (
            <button
              key={p}
              className={`btn ${filterPriority === p ? "btn-accent" : ""}`}
              onClick={() => setFilterPriority(filterPriority === p ? null : p)}
            >
              {p} ({counts[p]})
            </button>
          ) : null,
        )}
        <div className="detection-controls">
          <button className="btn-ghost" onClick={expandAll}>Expand all</button>
          <button className="btn-ghost" onClick={collapseAll}>Collapse all</button>
        </div>
      </div>

      {treeGroups.map((group) => (
        <TreePanel
          key={group.tree_id}
          group={group}
          isExpanded={expandedTrees.has(group.tree_id)}
          onToggle={toggleTree}
          expandedSteps={expandedSteps}
          onToggleStep={toggleStep}
          expandedPatterns={expandedPatterns}
          onTogglePattern={togglePattern}
        />
      ))}
    </div>
  );
}
